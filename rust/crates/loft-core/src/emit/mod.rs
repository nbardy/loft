//! Emit (plan §1.10, §3.3, §1.9 precision, lane 2.2): graph functions to one WGSL module, validated by Naga with no GPU.
//!
//! - **Names by first use.** Statements are named in the order a walk from each result first needs them; a graph
//!   function is named `f` + the hash of its own text, so the same function has the same name in every module and
//!   every job, whichever module recorded it first. Node ids never reach the output.
//! - **One text for both precisions.** Generated floats are `lf`; the host prepends `alias lf = f32;` or
//!   `enable f16; alias lf = f16;` at load (`validate` does the same). Storage stays f32 and is converted on read.
//!   Raw author functions keep their own types: their arguments are converted to f32 and their result back to `lf`.
//! - **Site map.** Every line of the module records the node it writes (or the raw line it copies), so a Naga refusal
//!   comes back as `Scene { Wgsl }` at the author's file and line.
mod body;

use crate::prim::{self, Spell};
use body::{Body, unsupported};
use loft_ir::graph::{Graph, Prim, Tag, Ty};
use loft_ir::{Error, error::SceneFault};
use std::collections::HashMap;

pub struct Line {
    text: String,
    node: Option<u32>,
}

/// What a module line came from: generated for a node (None: scaffolding), or line `k` of a raw node's source.
#[derive(Clone, Copy)]
enum Src {
    Gen(Option<u32>),
    Raw(u32, u32),
}

/// One WGSL module (without the `lf` alias line) and the kernel resources it reads, in binding order
/// (`@group(1) @binding(k)` reads `reads[k]`).
pub struct Emitted {
    pub text: String,
    pub reads: Vec<u32>,
    lines: Vec<Src>,
}

/// The module exporting each Fn decl `exports[i].1` as a WGSL function named `exports[i].0`. `at` is layout's answer
/// per decl (lane 2.1): a Param's component slot and an Input's vec4 slot in the frame uniform (`[slot, 0]`), a Kernel's
/// resource index and lanes per row (`[res, lanes]`). The module is validated as f32 before it is returned.
pub fn module(g: &Graph, exports: &[(&str, u32)], at: &[[u32; 2]]) -> Result<Emitted, Error> {
    let raws = raw_names(g)?;
    let mut e = Module { g, at, raws: &raws, fns: HashMap::new(), order: vec![], out: vec![], reads: vec![], raw_used: vec![] };
    let mut tail = vec![];
    for &(name, d) in exports {
        let (params, result) = signature(g, d)?;
        let f = e.function(d)?;
        let ps: Vec<String> = params.iter().enumerate().map(|(k, &b)| format!("a{k}: {}", ty_name(g.nodes[b as usize].ty, "lf"))).collect();
        let args: Vec<String> = (0..params.len()).map(|k| format!("a{k}")).collect();
        let ret = ty_name(g.nodes[result as usize].ty, "lf");
        tail.push(format!("fn {name}({}) -> {ret} {{ return {f}({}); }}", ps.join(", "), args.join(", ")));
    }
    let (params, inputs) = (0..g.decls.len()).fold((0, 0), |(p, i), d| match g.decls[d].tag {
        Tag::Param => (p.max(at[d][0] / 4 + 1), i),
        Tag::Input => (p, i.max(at[d][0] + 1)),
        Tag::Fn | Tag::Loop | Tag::Kernel | Tag::Draw | Tag::Pass | Tag::Medium | Tag::Check | Tag::Solve | Tag::Camera => (p, i),
    });
    let mut lines: Vec<(String, Src)> = vec![];
    let mut put = |t: String| lines.push((t, Src::Gen(None)));
    put(format!("struct Frame {{ time: f32, params: array<vec4<f32>, {}>, inputs: array<vec4<f32>, {}> }}", params.max(1), inputs.max(1)));
    put("@group(0) @binding(0) var<uniform> frame: Frame;".into());
    for (k, &kern) in e.reads.iter().enumerate() {
        put(format!("@group(1) @binding({k}) var<storage, read> r{}: array<vec4<f32>>;", at[kern as usize][0]));
    }
    for &n in &e.raw_used {
        let Some(loft_ir::graph::Node { op: loft_ir::graph::Op::Raw(sym), .. }) = g.nodes.get(n as usize) else { unreachable!("raw_used holds Raw nodes") };
        lines.extend(g.sym(*sym).lines().enumerate().map(|(k, t)| (t.to_string(), Src::Raw(n, k as u32))));
    }
    for &d in &e.order {
        lines.extend(e.fns[&d].1.iter().map(|l| (l.text.clone(), Src::Gen(l.node))));
    }
    lines.extend(tail.into_iter().map(|t| (t, Src::Gen(None))));
    let text = lines.iter().map(|(t, _)| t.as_str()).collect::<Vec<_>>().join("\n");
    let m = Emitted { text, reads: e.reads, lines: lines.into_iter().map(|(_, s)| s).collect() };
    validate(g, &m, false)?;
    Ok(m)
}

/// Naga's validation of `m` under one precision (the `lf` alias the host would prepend). A refusal is `Scene { Wgsl }`
/// at the site of the node that wrote the line, or at the author's line inside a raw function.
pub fn validate(g: &Graph, m: &Emitted, f16: bool) -> Result<(), Error> {
    let prelude = if f16 { "enable f16;\nalias lf = f16;\n" } else { "alias lf = f32;\n" };
    let src = format!("{prelude}{}", m.text);
    let skip = prelude.lines().count() as u32;
    let refuse = |line: Option<u32>, msg: String| {
        let line = line.unwrap_or(0).saturating_sub(skip);
        let (site, line) = match line.checked_sub(1).and_then(|l| m.lines.get(l as usize)) {
            Some(Src::Gen(n)) => (n.and_then(|n| g.site(n).cloned()), line),
            Some(Src::Raw(n, k)) => (raw_site(g, *n, *k), k + 1),
            None => (None, line),
        };
        Error::Scene { site, fault: SceneFault::Wgsl { line, msg: msg.into() } }
    };
    let module = naga::front::wgsl::parse_str(&src).map_err(|e| refuse(e.location(&src).map(|l| l.line_number), e.message().into()))?;
    let caps = naga::valid::Capabilities::default() | naga::valid::Capabilities::SHADER_FLOAT16;
    naga::valid::Validator::new(naga::valid::ValidationFlags::all(), caps)
        .validate(&module)
        .map(|_| ())
        .map_err(|e| refuse(e.location(&src).map(|l| l.line_number), e.as_inner().to_string()))
}

/// The author's line `k` of a raw function: its node's site moved down `k` lines (the source starts at the site).
fn raw_site(g: &Graph, n: u32, k: u32) -> Option<loft_ir::graph::Site> {
    g.site(n).map(|s| loft_ir::graph::Site { line: s.line + k, ..s.clone() })
}

/// Each raw node's function name: the last function its source declares, after Naga parses the source alone (so a
/// typo in a raw function is refused at the author's line even when nothing calls it).
fn raw_names(g: &Graph) -> Result<HashMap<u32, String>, Error> {
    let mut by_sym: HashMap<u32, String> = HashMap::new();
    let mut out = HashMap::new();
    for (n, node) in g.nodes.iter().enumerate() {
        let loft_ir::graph::Op::Raw(sym) = node.op else { continue };
        let n = n as u32;
        if let Some(name) = by_sym.get(&sym) {
            out.insert(n, name.clone());
            continue;
        }
        let src = format!("alias lf = f32;\n{}", g.sym(sym));
        let refuse = |line: Option<u32>, msg: &str| {
            let k = line.unwrap_or(2).saturating_sub(2);
            Error::Scene { site: raw_site(g, n, k), fault: SceneFault::Wgsl { line: k + 1, msg: msg.into() } }
        };
        let m = naga::front::wgsl::parse_str(&src).map_err(|e| refuse(e.location(&src).map(|l| l.line_number), e.message()))?;
        let name = m.functions.iter().last().and_then(|(_, f)| f.name.clone()).ok_or_else(|| refuse(None, "a raw function source declares no function"))?;
        by_sym.insert(sym, name.clone());
        out.insert(n, name);
    }
    Ok(out)
}

/// A Fn decl's parameters (binder nodes) and its single result. A multi-result function has no consumer in WGSL yet.
fn signature(g: &Graph, d: u32) -> Result<(&[u32], u32), Error> {
    let decl = &g.decls[d as usize];
    let roots = g.roots_of(decl);
    match (decl.tag, roots.split_at_checked(decl.arity as usize)) {
        (Tag::Fn, Some((params, [result]))) => Ok((params, *result)),
        (Tag::Fn, Some((_, [r, ..]))) => Err(unsupported(g, *r, "a graph function with several results in WGSL")),
        (_, _) => Err(Error::Usage(format!("emit: decl {d} is not a graph function with a result").into())),
    }
}

struct Module<'g> {
    g: &'g Graph,
    at: &'g [[u32; 2]],
    raws: &'g HashMap<u32, String>,
    /// Per Fn decl: its name and its lines (empty for a second decl with the same text, which shares the first's).
    fns: HashMap<u32, (String, Vec<Line>)>,
    order: Vec<u32>,
    out: Vec<String>,
    reads: Vec<u32>,
    raw_used: Vec<u32>,
}

impl Module<'_> {
    /// Writes decl `d` (callees first) and returns its name.
    fn function(&mut self, d: u32) -> Result<String, Error> {
        if let Some((name, _)) = self.fns.get(&d) {
            return Ok(name.clone());
        }
        let g = self.g;
        let (params, result) = signature(g, d)?;
        let mut b = Body::new(g, self.at, self.raws, params);
        for c in b.place(result) {
            let name = self.function(c)?;
            b.fn_names.insert(c, name);
        }
        let r = b.need(result)?;
        let (lines, needs) = b.lines();
        let ps: Vec<String> = params.iter().enumerate().map(|(k, &p)| format!("a{k}: {}", ty_name(g.nodes[p as usize].ty, "lf"))).collect();
        let ret = ty_name(g.nodes[result as usize].ty, "lf");
        let sig = format!("({}) -> {ret} {{", ps.join(", "));
        let mut h = Fnv::default();
        h.eat(&sig);
        lines.iter().for_each(|l| h.eat(&l.text));
        h.eat(&r);
        let name = format!("f{:016x}", h.0);
        for k in needs.reads {
            if !self.reads.contains(&k) {
                self.reads.push(k);
            }
        }
        for n in needs.raws {
            let first = self.raw_used.iter().all(|&m| self.raws[&m] != self.raws[&n]);
            if first {
                self.raw_used.push(n);
            }
        }
        if self.out.contains(&name) {
            self.fns.insert(d, (name.clone(), vec![]));
            return Ok(name);
        }
        let mut text = vec![Line { text: format!("fn {name}{sig}"), node: None }];
        text.extend(lines.into_iter().map(|l| Line { text: format!("  {}", l.text), node: l.node }));
        text.push(Line { text: format!("  return {r};"), node: Some(result) });
        text.push(Line { text: "}".into(), node: None });
        self.out.push(name.clone());
        self.order.push(d);
        self.fns.insert(d, (name.clone(), text));
        Ok(name)
    }
}

/// FNV-1a 64 over the function's text (with line breaks), which is all a function's name depends on.
#[derive(Default)]
struct Fnv(u64);
impl Fnv {
    fn eat(&mut self, s: &str) {
        let mut h = if self.0 == 0 { 0xcbf2_9ce4_8422_2325 } else { self.0 };
        for b in s.bytes().chain(*b"\n") {
            h = (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3);
        }
        self.0 = h;
    }
}

pub(crate) fn ty_name(t: Ty, float: &str) -> String {
    match t {
        Ty::F32 => float.into(),
        Ty::V2 => format!("vec2<{float}>"),
        Ty::V3 => format!("vec3<{float}>"),
        Ty::V4 => format!("vec4<{float}>"),
        Ty::Bool => "bool".into(),
        Ty::Rec(_) => unreachable!("a record value is refused at its function's signature"),
    }
}

/// A literal: `width(ty)` authored f64s rounded once to f32 (a bool is 0 or 1).
pub(crate) fn lit(g: &Graph, i: u32, ty: Ty) -> String {
    let at = |k: u32| f64::from_bits(g.lits[(i + k) as usize]);
    match ty.width() {
        None => (if at(0) == 0.0 { "false" } else { "true" }).into(),
        Some(1) => format!("lf({:?})", at(0) as f32),
        Some(w) => format!("{}({})", ty_name(ty, "lf"), (0..u32::from(w)).map(|k| format!("{:?}", at(k) as f32)).collect::<Vec<_>>().join(", ")),
    }
}

/// An operator's WGSL from its operands' text and types. Spellings come from `prim::PRIMS`; `vec`, `pick` and the
/// scalar forms WGSL lacks (`normalize`, `dot` of scalars) are written here.
pub(crate) fn spell(p: Prim, xs: &[String], tys: &[Ty], ty: Ty) -> String {
    let splat = |k: usize| if tys[k] == ty || tys[k] != Ty::F32 { xs[k].clone() } else { format!("{}({})", ty_name(ty, "lf"), xs[k]) };
    let c = match core(p) {
        Some(c) => c,
        None if p == Prim::Vec => return format!("{}({})", ty_name(ty, "lf"), xs.join(", ")),
        // `pick` is a chain of selects on `i == k`, as src/expr.ts defines it: out of range reads the first value.
        None => {
            return (2..xs.len()).fold(splat(1), |acc, k| format!("select({acc}, {}, {} == lf({}.0))", splat(k), xs[0], k - 1));
        }
    };
    let def = prim::def(c);
    match (&def.wgsl, c, tys.first()) {
        (_, prim::Prim::Normalize, Some(Ty::F32)) => format!("sign({})", xs[0]),
        (_, prim::Prim::Dot, Some(Ty::F32)) => format!("({} * {})", xs[0], xs[1]),
        (Spell::Infix(op), _, _) => format!("({} {op} {})", xs[0], xs[1]),
        (Spell::Prefix(op), _, _) => format!("({op}{})", xs[0]),
        (Spell::Mod, _, _) => format!("({a} - {b} * floor({a} / {b}))", a = xs[0], b = xs[1]),
        (Spell::Call(f), _, _) if def.splat => format!("{f}({})", (0..xs.len()).map(splat).collect::<Vec<_>>().join(", ")),
        (Spell::Call(f), _, _) => format!("{f}({})", xs.join(", ")),
        (Spell::Select, _, _) => unreachable!("select is written by the body (branch-local if)"),
    }
}

/// loft-ir's operator as loft-core's table row. Two enums for one operator set is a wave-1 seam (lane 1.1 and 1.3 each
/// wrote one); when they become one `Prim` this match goes away.
#[rustfmt::skip]
fn core(p: Prim) -> Option<prim::Prim> {
    use prim::Prim as C;
    Some(match p {
        Prim::Add => C::Add, Prim::Sub => C::Sub, Prim::Mul => C::Mul, Prim::Div => C::Div, Prim::Min => C::Min,
        Prim::Max => C::Max, Prim::Pow => C::Pow, Prim::Mod => C::Mod, Prim::Rem => C::Rem, Prim::Atan2 => C::Atan2,
        Prim::Step => C::Step, Prim::Neg => C::Neg, Prim::Abs => C::Abs, Prim::Sign => C::Sign, Prim::Floor => C::Floor,
        Prim::Ceil => C::Ceil, Prim::Trunc => C::Trunc, Prim::RoundEven => C::RoundEven, Prim::Fract => C::Fract,
        Prim::Sqrt => C::Sqrt, Prim::InverseSqrt => C::InverseSqrt, Prim::Exp => C::Exp, Prim::Exp2 => C::Exp2,
        Prim::Log => C::Log, Prim::Log2 => C::Log2, Prim::Sin => C::Sin, Prim::Cos => C::Cos, Prim::Tan => C::Tan,
        Prim::Asin => C::Asin, Prim::Acos => C::Acos, Prim::Atan => C::Atan, Prim::Sinh => C::Sinh, Prim::Cosh => C::Cosh,
        Prim::Tanh => C::Tanh, Prim::Saturate => C::Saturate, Prim::Normalize => C::Normalize, Prim::Mix => C::Mix,
        Prim::Clamp => C::Clamp, Prim::Smoothstep => C::Smoothstep, Prim::Length => C::Length, Prim::Distance => C::Distance,
        Prim::Dot => C::Dot, Prim::Cross => C::Cross, Prim::Lt => C::Lt, Prim::Gt => C::Gt, Prim::Le => C::Le,
        Prim::Ge => C::Ge, Prim::Eq => C::Eq, Prim::Ne => C::Ne, Prim::And => C::And, Prim::Or => C::Or, Prim::Not => C::Not,
        Prim::Select => C::Select,
        Prim::Vec | Prim::Pick => return None,
    })
}
