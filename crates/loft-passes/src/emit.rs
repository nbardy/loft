//! WGSL emit (plan §1 layer 2): one module per kind, never per instance, and three shared modules (cull, classify,
//! present). A kind's text depends only on its declaration and the library fns it reaches; its id, row base and row
//! count are data in `KindParams`. The author's structure is kept: their fns stay fns, their loops stay loops.

use crate::Prog;
use loft_ir::ast::*;
use loft_ir::program::abi;
use std::collections::BTreeSet;
use std::fmt::Write;

const PRELUDE: &str = r"struct Frame {
  view_proj: mat4x4<f32>,
  inv_view_proj: mat4x4<f32>,
  eye: vec4<f32>,
  sun: vec4<f32>,
  size: vec4<u32>,
  cold: vec4<u32>,
}
@group(0) @binding(0) var<uniform> frame: Frame;
const LOFT_NONCE: u32 = {{NONCE}};
const LOFT_EMPTY: u32 = 0xffffffffu;

struct LoftXform { r: mat3x3<f32>, t: vec3<f32> }
fn loft_xf_pt(x: LoftXform, p: vec3<f32>) -> vec3<f32> { return x.r * p + x.t; }
fn loft_xf_vec(x: LoftXform, v: vec3<f32>) -> vec3<f32> { return x.r * v; }
fn loft_xf_inv(x: LoftXform) -> LoftXform { let rt = transpose(x.r); return LoftXform(rt, -(rt * x.t)); }
fn loft_rot_y(p: vec3<f32>, a: f32) -> vec3<f32> {
  let s = sin(a);
  let c = cos(a);
  return vec3<f32>(c * p.x + s * p.z, p.y, c * p.z - s * p.x);
}
// The camera ray through a pixel position (pixel units, centre at +0.5).
fn loft_ray(px: vec2<f32>) -> vec3<f32> {
  let ndc = vec2<f32>(px.x / f32(frame.size.x) * 2.0 - 1.0, 1.0 - px.y / f32(frame.size.y) * 2.0);
  let far = frame.inv_view_proj * vec4<f32>(ndc, 1.0, 1.0);
  return normalize(far.xyz / far.w - frame.eye.xyz);
}
";

/// Lowers a type for WGSL: spaces and units erase to their storage.
fn wty(t: &Ty) -> String {
    match t {
        Ty::Void => String::new(),
        Ty::Bool => "bool".into(),
        Ty::U32 => "u32".into(),
        Ty::I32 | Ty::AbstractInt => "i32".into(),
        Ty::F32 => "f32".into(),
        Ty::VecN(n) => format!("vec{n}<f32>"),
        Ty::Geo(..) => "vec3<f32>".into(),
        Ty::Xform(..) => "LoftXform".into(),
        Ty::Row(_) => "Row".into(),
        Ty::Map(..) => unreachable!("maps are not values"),
    }
}

fn f32_lit(v: f64) -> String {
    let s = format!("{:?}", v as f32);
    if s.contains('.') || s.contains('e') || s.contains("inf") || s.contains("NaN") { s } else { format!("{s}.0") }
}

/// A map statement's emitted locals, for `.cell`, `.to_parent` and `.from_parent`.
#[derive(Clone)]
enum MapStage {
    Mirror,
    Translate(String),
    RotY(String),
}

pub struct Emitter<'p, 'a> {
    p: &'p Prog<'a>,
    types: &'p [Ty],
    out: String,
    maps: Vec<(String, Vec<MapStage>)>,
}

/// The fns, user and library, reachable from a kind's queries and the shade glue's light fn.
fn reachable<'a>(p: &Prog<'a>, roots: &[&'a FnDecl], extra: &[&str]) -> (Vec<&'a FnDecl>, BTreeSet<String>) {
    fn walk_expr(e: &Expr, calls: &mut Vec<String>, names: &mut Vec<String>) {
        match &e.kind {
            ExprKind::Call { callee, args } => {
                calls.push(callee.name.clone());
                args.iter().for_each(|a| walk_expr(&a.value, calls, names));
            }
            ExprKind::Ctor { args, .. } => args.iter().for_each(|a| walk_expr(&a.value, calls, names)),
            ExprKind::Name(n) => names.push(n.clone()),
            ExprKind::Member(b, _) | ExprKind::Unary(_, b) => walk_expr(b, calls, names),
            ExprKind::Index(a, b) | ExprKind::Binary(_, a, b) => {
                walk_expr(a, calls, names);
                walk_expr(b, calls, names);
            }
            ExprKind::Num { .. } | ExprKind::Bool(_) => {}
        }
    }
    fn walk_block(b: &Block, calls: &mut Vec<String>, names: &mut Vec<String>) {
        for s in &b.stmts {
            match s {
                Stmt::Let { value, .. } => walk_expr(value, calls, names),
                Stmt::Assign { target, value, .. } => {
                    walk_expr(target, calls, names);
                    walk_expr(value, calls, names);
                }
                Stmt::Map { head, stages, .. } => {
                    walk_expr(head, calls, names);
                    stages.iter().flat_map(|(_, a)| a).for_each(|a| walk_expr(a, calls, names));
                }
                Stmt::Return(v, _) => v.iter().for_each(|e| walk_expr(e, calls, names)),
                Stmt::If { cond, then, otherwise } => {
                    walk_expr(cond, calls, names);
                    walk_block(then, calls, names);
                    otherwise.iter().for_each(|o| walk_block(o, calls, names));
                }
                Stmt::For { lo, hi, body, .. } => {
                    walk_expr(lo, calls, names);
                    walk_expr(hi, calls, names);
                    walk_block(body, calls, names);
                }
                Stmt::Emit { fields, .. } => fields.iter().for_each(|(_, e)| walk_expr(e, calls, names)),
                Stmt::Expr(e) => walk_expr(e, calls, names),
            }
        }
    }
    let mut seen: Vec<&FnDecl> = vec![];
    let mut consts = BTreeSet::new();
    let mut todo: Vec<&FnDecl> = roots.to_vec();
    for f in extra.iter().filter_map(|n| p.fns.get(n).copied()) {
        seen.push(f);
        todo.push(f);
    }
    while let Some(f) = todo.pop() {
        let (mut calls, mut names) = (vec![], vec![]);
        walk_block(&f.body, &mut calls, &mut names);
        for c in calls {
            if let Some(g) = p.fns.get(c.as_str())
                && !seen.iter().any(|s| std::ptr::eq(*s, *g))
                && !roots.iter().any(|s| std::ptr::eq(*s, *g))
            {
                seen.push(g);
                todo.push(g);
            }
        }
        for n in names {
            if p.consts.get(n.as_str()).is_some_and(|c| !c.comptime) {
                consts.insert(n);
            }
        }
    }
    // Callees before callers: WGSL allows any order, but readers expect it.
    seen.reverse();
    (seen, consts)
}

impl<'p, 'a> Emitter<'p, 'a> {
    pub fn new(p: &'p Prog<'a>, types: &'p [Ty]) -> Self {
        Emitter { p, types, out: String::new(), maps: vec![] }
    }

    fn ty_of(&self, e: &Expr) -> &Ty {
        &self.types[e.id as usize]
    }

    fn resolve(&self, t: &TyExpr) -> Ty {
        // Signature types never mention a map space, so erasure needs no scope.
        match t {
            TyExpr::Name(i) => match i.name.as_str() {
                "u32" => Ty::U32,
                "i32" => Ty::I32,
                "bool" => Ty::Bool,
                "vec2f" => Ty::VecN(2),
                "vec3f" => Ty::VecN(3),
                "vec4f" => Ty::VecN(4),
                _ => Ty::F32,
            },
            TyExpr::Generic(i, _) => Ty::VecN(i.name.as_bytes()[3] - b'0'),
            TyExpr::Geo(g, _) => Ty::Geo(*g, Space::Var(String::new())),
            TyExpr::Xform(..) => Ty::Xform(Space::Var(String::new()), Space::Var(String::new())),
            TyExpr::Field(_) => Ty::F32,
        }
    }

    fn func(&mut self, f: &FnDecl, name: &str) {
        let params: Vec<String> = f
            .params
            .iter()
            .map(|prm| match &prm.ty {
                Some(t) => format!("{}: {}", prm.name.name, wty(&self.resolve(t))),
                None => "self_: Row".into(),
            })
            .collect();
        let ret = f.ret.as_ref().map(|t| format!(" -> {}", wty(&self.resolve(t)))).unwrap_or_default();
        let _ = writeln!(self.out, "fn {name}({}){ret} {{", params.join(", "));
        self.maps.clear();
        self.block_body(&f.body, 1);
        self.out += "}\n\n";
    }

    fn block_body(&mut self, b: &Block, depth: usize) {
        let mark = self.maps.len();
        for s in &b.stmts {
            self.stmt(s, depth);
        }
        self.maps.truncate(mark);
    }

    fn line(&mut self, depth: usize, s: &str) {
        let _ = writeln!(self.out, "{}{s}", "  ".repeat(depth));
    }

    fn stmt(&mut self, s: &Stmt, d: usize) {
        match s {
            Stmt::Let { mutable, name, value, .. } => {
                let v = self.expr(value);
                let t = wty(self.ty_of(value));
                let kw = if *mutable { "var" } else { "let" };
                self.line(d, &format!("{kw} {}: {t} = {v};", name.name));
            }
            Stmt::Assign { target, op, value } => {
                let (t, v) = (self.expr(target), self.expr(value));
                let op = op.map_or("=".to_string(), |o| format!("{}=", o.wgsl()));
                self.line(d, &format!("{t} {op} {v};"));
            }
            Stmt::Map { name, head, stages } => {
                let n = &name.name;
                let (point, mut all): (String, Vec<(&str, Vec<String>)>) = match &head.kind {
                    ExprKind::Call { callee, args } if crate::check::MAP_OPS.contains(&callee.name.as_str()) => {
                        (self.expr(&args[0].value), vec![(callee.name.as_str(), vec![])])
                    }
                    _ => (self.expr(head), vec![]),
                };
                for (op, args) in stages {
                    let a = args.iter().map(|x| self.expr(x)).collect();
                    all.push((op.name.as_str(), a));
                }
                self.line(d, &format!("let {n}_s0: vec3<f32> = {point};"));
                let mut recorded = vec![];
                for (i, (op, args)) in all.iter().enumerate() {
                    let (prev, cur) = (format!("{n}_s{i}"), format!("{n}_s{}", i + 1));
                    match *op {
                        "mirror_x" => {
                            self.line(d, &format!("let {n}_cell: f32 = select(-1.0, 1.0, {prev}.x >= 0.0);"));
                            self.line(d, &format!("let {cur}: vec3<f32> = vec3<f32>(abs({prev}.x), {prev}.yz);"));
                            recorded.push(MapStage::Mirror);
                        }
                        "translate" => {
                            self.line(d, &format!("let {n}_t{i}: vec3<f32> = {};", args[0]));
                            self.line(d, &format!("let {cur}: vec3<f32> = {prev} + {n}_t{i};"));
                            recorded.push(MapStage::Translate(format!("{n}_t{i}")));
                        }
                        "rot_y" => {
                            self.line(d, &format!("let {n}_a{i}: f32 = {};", args[0]));
                            self.line(d, &format!("let {cur}: vec3<f32> = loft_rot_y({prev}, {n}_a{i});"));
                            recorded.push(MapStage::RotY(format!("{n}_a{i}")));
                        }
                        other => unreachable!("checked map stage {other}"),
                    }
                }
                self.line(d, &format!("let {n}: vec3<f32> = {n}_s{};", all.len()));
                self.maps.push((n.clone(), recorded));
            }
            Stmt::Return(v, _) => {
                let v = v.as_ref().map(|e| format!(" {}", self.expr(e))).unwrap_or_default();
                self.line(d, &format!("return{v};"));
            }
            Stmt::If { cond, then, otherwise } => {
                let c = self.expr(cond);
                self.line(d, &format!("if {c} {{"));
                self.block_body(then, d + 1);
                if let Some(o) = otherwise {
                    self.line(d, "} else {");
                    self.block_body(o, d + 1);
                }
                self.line(d, "}");
            }
            Stmt::For { var, lo, hi, body, .. } => {
                let t = wty(self.ty_of(lo)).replace("i32", match self.ty_of(hi) {
                    Ty::U32 => "u32",
                    _ => "i32",
                });
                let (a, b) = (self.expr(lo), self.expr(hi));
                let v = &var.name;
                self.line(d, &format!("for (var {v}: {t} = {a}; {v} < {b}; {v}++) {{"));
                self.block_body(body, d + 1);
                self.line(d, "}");
            }
            Stmt::Emit { .. } => unreachable!("checked: stage 0 only, never emitted"),
            Stmt::Expr(e) => {
                let v = self.expr(e);
                if matches!(e.kind, ExprKind::Call { .. }) && *self.ty_of(e) == Ty::Void {
                    self.line(d, &format!("{v};"));
                } else {
                    self.line(d, &format!("_ = {v};"));
                }
            }
        }
    }

    fn map_apply(&self, name: &str, inverse: bool, x: String, point: bool) -> String {
        let stages = &self.maps.iter().rev().find(|(n, _)| n == name).expect("checked: map in scope").1;
        let mut v = x;
        let order: Vec<&MapStage> = if inverse { stages.iter().rev().collect() } else { stages.iter().collect() };
        for st in order {
            v = match st {
                MapStage::Mirror => format!("({v} * vec3<f32>({name}_cell, 1.0, 1.0))"),
                MapStage::Translate(t) if point => format!("({v} {} {t})", if inverse { "-" } else { "+" }),
                MapStage::Translate(_) => v,
                MapStage::RotY(a) => format!("loft_rot_y({v}, {}{a})", if inverse { "-" } else { "" }),
            };
        }
        v
    }

    pub fn expr(&mut self, e: &Expr) -> String {
        match &e.kind {
            ExprKind::Num { value, suffix, text, .. } => match self.ty_of(e) {
                Ty::U32 => format!("{}u", *value as u32),
                Ty::I32 => format!("{}i", *value as i32),
                Ty::AbstractInt => text.clone(),
                _ => f32_lit(if *suffix == NumSuffix::Deg { value.to_radians() } else { *value }),
            },
            ExprKind::Bool(b) => b.to_string(),
            ExprKind::Name(n) if n == "self" => "self_".into(),
            ExprKind::Name(n) => n.clone(),
            ExprKind::Member(base, m) => {
                if let ExprKind::Name(n) = &base.kind
                    && m.name == "cell"
                    && self.maps.iter().any(|(k, _)| k == n)
                {
                    return format!("{n}_cell");
                }
                format!("{}.{}", self.expr(base), m.name)
            }
            ExprKind::Index(a, i) => format!("{}[{}]", self.expr(a), self.expr(i)),
            ExprKind::Unary(op, x) => {
                let o = match op {
                    UnOp::Neg => "-",
                    UnOp::Not => "!",
                };
                format!("({o}{})", self.expr(x))
            }
            ExprKind::Binary(op, a, b) => {
                let point = matches!(self.ty_of(b), Ty::Geo(Geo::Pt, _));
                match (self.ty_of(a).clone(), &a.kind) {
                    (Ty::Map(..), ExprKind::Member(base, m)) => {
                        let ExprKind::Name(n) = &base.kind else { unreachable!("checked map ref") };
                        let x = self.expr(b);
                        self.map_apply(n, m.name == "to_parent", x, point)
                    }
                    (Ty::Xform(..), _) => {
                        let f = if point { "loft_xf_pt" } else { "loft_xf_vec" };
                        format!("{f}({}, {})", self.expr(a), self.expr(b))
                    }
                    _ => format!("({} {} {})", self.expr(a), op.wgsl(), self.expr(b)),
                }
            }
            ExprKind::Ctor { args, .. } => {
                let a: Vec<String> = args.iter().map(|x| self.expr(&x.value)).collect();
                match self.ty_of(e) {
                    Ty::Geo(Geo::Dir | Geo::Norm, _) => format!("normalize(vec3<f32>({}))", a.join(", ")),
                    Ty::Xform(..) => format!("loft_xform_yaw({}, {})", a[0], a[1]),
                    t => format!("{}({})", wty(t), a.join(", ")),
                }
            }
            ExprKind::Call { callee, args } => {
                let a: Vec<String> = args.iter().map(|x| self.expr(&x.value)).collect();
                let name = match callee.name.as_str() {
                    "inv" => "loft_xf_inv".to_string(),
                    "vec2" | "vec2f" => "vec2<f32>".into(),
                    "vec3" | "vec3f" => "vec3<f32>".into(),
                    "vec4" | "vec4f" => "vec4<f32>".into(),
                    other => other.into(),
                };
                format!("{name}({})", a.join(", "))
            }
        }
    }

    fn header(&mut self, nonce: &str) {
        self.out += &PRELUDE.replace("{{NONCE}}", nonce);
        self.out += "fn loft_xform_yaw(o: vec3<f32>, yaw: f32) -> LoftXform {\n  let s = sin(yaw);\n  let c = cos(yaw);\n  return LoftXform(mat3x3<f32>(vec3<f32>(c, 0.0, -s), vec3<f32>(0.0, 1.0, 0.0), vec3<f32>(s, 0.0, c)), o);\n}\n\n";
    }

    fn library(&mut self, fns: &[&FnDecl], consts: &BTreeSet<String>) {
        for c in consts {
            let decl = self.p.consts[c.as_str()];
            let v = self.expr(&decl.value);
            let t = wty(&self.types[decl.value.id as usize]);
            let _ = writeln!(self.out, "const {c}: {t} = {v};");
        }
        for f in fns {
            self.func(f, &f.name.name);
        }
    }
}

/// The text of one kind's module. `light` is the library fn the shade pass calls (named in the schedule).
pub fn kind_module(p: &Prog, types: &[Ty], k: &KindDecl, light: &str) -> String {
    let mut em = Emitter::new(p, types);
    em.header(abi::NONCE_PLACEHOLDER);
    let q = |which| k.queries.iter().find(|(w, _)| *w == which).map(|(_, f)| f);
    let mut roots: Vec<&FnDecl> = k.queries.iter().map(|(_, f)| f).collect();
    roots.extend(k.fns.iter());
    let (lib, consts) = reachable(p, &roots, &[light]);
    // The row: the author's fields, then the compiler's hidden columns.
    let mut fields = vec![];
    let mut loads = vec![];
    let mut o = 0u32;
    for rf in &k.row {
        let t = em.resolve(&rf.ty);
        let w = |i: u32| format!("rows[o + {}u]", o + i);
        let load = match &t {
            Ty::F32 => w(0),
            Ty::U32 => format!("bitcast<u32>({})", w(0)),
            Ty::I32 => format!("bitcast<i32>({})", w(0)),
            Ty::Bool => format!("({} != 0.0)", w(0)),
            Ty::VecN(n) => format!("vec{n}<f32>({})", (0..u32::from(*n)).map(w).collect::<Vec<_>>().join(", ")),
            Ty::Geo(..) => format!("vec3<f32>({}, {}, {})", w(0), w(1), w(2)),
            Ty::Xform(..) => format!(
                "LoftXform(mat3x3<f32>({}), vec3<f32>({}, {}, {}))",
                (0..9).map(w).collect::<Vec<_>>().join(", "),
                w(9),
                w(10),
                w(11)
            ),
            other => unreachable!("checked row field type {other}"),
        };
        fields.push(format!("  {}: {},", rf.name.name, wty(&t)));
        loads.push(load);
        o += t.words().expect("checked storable");
    }
    let stride = o + abi::HIDDEN_WORDS;
    fields.push("  loft_lo: vec3<f32>,\n  loft_hi: vec3<f32>,\n  loft_lip: f32,".into());
    loads.push(format!("vec3<f32>(rows[o + {o}u], rows[o + {}u], rows[o + {}u])", o + 1, o + 2));
    loads.push(format!("vec3<f32>(rows[o + {}u], rows[o + {}u], rows[o + {}u])", o + 3, o + 4, o + 5));
    loads.push(format!("rows[o + {}u]", o + 6));
    let _ = write!(
        em.out,
        "// kind {name}: one module for every instance; rows are data.\nstruct Row {{\n{fields}\n}}\n\
         struct KindParams {{ kind_id: u32, base: u32, n_rows: u32, pad: u32 }}\n\
         @group(1) @binding(0) var<storage, read> rows: array<f32>;\n\
         @group(1) @binding(1) var<storage, read> visible: array<u32>;\n\
         @group(1) @binding(2) var<uniform> kp: KindParams;\n\
         const STRIDE: u32 = {stride}u;\n\
         fn load_row(i: u32) -> Row {{\n  let o = i * STRIDE;\n  return Row(\n    {loads}\n  );\n}}\n\n",
        name = k.name.name,
        fields = fields.join("\n"),
        loads = loads.join(",\n    "),
    );
    em.library(&lib, &consts);
    for f in &k.fns {
        em.func(f, &f.name.name);
    }
    for (which, f) in &k.queries {
        let n = match which {
            QueryKind::Visible => "q_visible",
            QueryKind::Trace => "q_trace",
            QueryKind::Material => "q_material",
        };
        em.func(f, n);
    }
    let hit = match q(QueryKind::Visible) {
        Some(_) => "  // Closed form: the author's `visible` query.\n  let t = q_visible(row, ro, rd);\n  if t < 0.0 { discard; }\n".to_string(),
        None => "  // Sphere trace with the row's derived Lipschitz bound (hidden column `loft_lip`).\n  \
                 var t = max(span.x, 0.0);\n  let step = 1.0 / row.loft_lip;\n  var hit = false;\n  \
                 for (var i = 0u; i < 192u; i++) {\n    let d = q_trace(row, ro + rd * t);\n    \
                 if d < 1e-4 + 2e-4 * t { hit = true; break; }\n    t += d * step;\n    if t > span.y { break; }\n  }\n  \
                 if !hit { discard; }\n"
            .to_string(),
    };
    let visible_twin = if q(QueryKind::Visible).is_some() { "q_visible(row, a.xyz, rd)" } else { "0.0" };
    let _ = write!(
        em.out,
        r"// --- generated entry points ---
const LOFT_CUBE: array<u32, 36> = array<u32, 36>(1u, 3u, 7u, 1u, 7u, 5u, 0u, 6u, 2u, 0u, 4u, 6u, 2u, 6u, 7u, 2u, 7u, 3u,
  0u, 1u, 5u, 0u, 5u, 4u, 4u, 5u, 7u, 4u, 7u, 6u, 0u, 2u, 3u, 0u, 3u, 1u);
struct VsOut {{ @builtin(position) pos: vec4<f32>, @location(0) wp: vec3<f32>, @location(1) @interpolate(flat) row: u32 }}
struct FsOut {{ @location(0) vis: vec2<u32>, @builtin(frag_depth) depth: f32 }}

// The proxy: the row's derived AABB, back faces (the camera may be inside it).
@vertex fn vs(@builtin(vertex_index) vi: u32, @builtin(instance_index) ii: u32) -> VsOut {{
  let r = visible[kp.base + ii];
  let row = load_row(r);
  let c = LOFT_CUBE[vi];
  let corner = vec3<f32>(f32(c & 1u), f32((c >> 1u) & 1u), f32((c >> 2u) & 1u));
  let wp = mix(row.loft_lo, row.loft_hi, corner);
  return VsOut(frame.view_proj * vec4<f32>(wp, 1.0), wp, r);
}}

fn loft_box(ro: vec3<f32>, rd: vec3<f32>, lo: vec3<f32>, hi: vec3<f32>) -> vec2<f32> {{
  let inv = 1.0 / rd;
  let a = (lo - ro) * inv;
  let b = (hi - ro) * inv;
  let n = min(a, b);
  let f = max(a, b);
  return vec2<f32>(max(max(n.x, n.y), n.z), min(min(f.x, f.y), f.z));
}}

@fragment fn fs(v: VsOut) -> FsOut {{
  if frame.cold.x == LOFT_NONCE {{ discard; }}
  let row = load_row(v.row);
  let ro = frame.eye.xyz;
  let rd = normalize(v.wp - ro);
  let span = loft_box(ro, rd, row.loft_lo, row.loft_hi);
{hit}  let clip = frame.view_proj * vec4<f32>(ro + rd * t, 1.0);
  return FsOut(vec2<u32>((kp.kind_id << 24u) | v.row, bitcast<u32>(t)), clip.z / clip.w);
}}

@group(2) @binding(0) var vis_tex: texture_2d<u32>;
@group(2) @binding(1) var hdr: texture_storage_2d<rgba16float, write>;
@group(2) @binding(2) var<storage, read> tiles: array<u32>;

// One workgroup per 8x8 tile that classify listed for this kind.
@compute @workgroup_size(8, 8) fn shade(@builtin(workgroup_id) wg: vec3<u32>, @builtin(local_invocation_id) lid: vec3<u32>) {{
  if frame.cold.x == LOFT_NONCE {{ return; }}
  let tile = tiles[kp.kind_id * frame.size.w + wg.x];
  let px = vec2<u32>(tile & 0xffffu, tile >> 16u) * 8u + lid.xy;
  if px.x >= frame.size.x || px.y >= frame.size.y {{ return; }}
  let v = textureLoad(vis_tex, px, 0).xy;
  if v.x == LOFT_EMPTY || (v.x >> 24u) != kp.kind_id {{ return; }}
  let row = load_row(v.x & 0xffffffu);
  let t = bitcast<f32>(v.y);
  let rd = loft_ray(vec2<f32>(px) + 0.5);
  let p = frame.eye.xyz + rd * t;
  // Normal from the field (tetrahedron differences), occlusion from the field along it.
  let e = 5e-4 * max(1.0, 0.25 * t);
  let k = vec2<f32>(1.0, -1.0);
  let n = normalize(k.xyy * q_trace(row, p + k.xyy * e) + k.yyx * q_trace(row, p + k.yyx * e)
    + k.yxy * q_trace(row, p + k.yxy * e) + k.xxx * q_trace(row, p + k.xxx * e));
  var occ = 0.0;
  var w = 1.0;
  for (var i = 0u; i < 5u; i++) {{
    let h = 0.02 + 0.06 * f32(i);
    occ += (h - q_trace(row, p + n * h)) * w;
    w *= 0.8;
  }}
  let ao = clamp(1.0 - 2.0 * occ, 0.0, 1.0);
  let albedo = q_material(row, p, n);
  textureStore(hdr, px, vec4<f32>({light}(albedo, n, -rd, normalize(frame.sun.xyz), ao), 1.0));
}}

@group(3) @binding(0) var<storage, read> twin_in: array<vec4<f32>>;
@group(3) @binding(1) var<storage, read_write> twin_out: array<vec4<f32>>;

// The twin gate: the same queries the frame runs, at CPU-chosen samples.
@compute @workgroup_size(64) fn twin(@builtin(global_invocation_id) g: vec3<u32>) {{
  let i = g.x;
  if i >= arrayLength(&twin_out) / 2u {{ return; }}
  let a = twin_in[2u * i];
  let b = twin_in[2u * i + 1u];
  let row = load_row(bitcast<u32>(a.w));
  let rd = normalize(b.xyz);
  twin_out[2u * i] = vec4<f32>(q_trace(row, a.xyz), {visible_twin}, 0.0, 0.0);
  twin_out[2u * i + 1u] = vec4<f32>(q_material(row, a.xyz, rd), 0.0);
}}
",
    );
    em.out
}

pub fn cull_module() -> String {
    let mut s = PRELUDE.replace("{{NONCE}}", abi::NONCE_PLACEHOLDER);
    s += r"
struct Inst { lo: vec3<f32>, kind: u32, hi: vec3<f32>, row: u32 }
@group(1) @binding(0) var<storage, read> insts: array<Inst>;
@group(1) @binding(1) var<storage, read_write> draw_args: array<atomic<u32>>;
@group(1) @binding(2) var<storage, read_write> visible: array<u32>;
@group(1) @binding(3) var<storage, read> bases: array<u32>;

// Every row of every kind: frustum-test its derived AABB, append visible rows, count one draw per kind.
@compute @workgroup_size(64) fn cull(@builtin(global_invocation_id) g: vec3<u32>) {
  if frame.cold.x == LOFT_NONCE { return; }
  let i = g.x;
  if i >= arrayLength(&insts) { return; }
  let b = insts[i];
  var outside = 0x3fu;
  for (var c = 0u; c < 8u; c++) {
    let corner = vec3<f32>(f32(c & 1u), f32((c >> 1u) & 1u), f32((c >> 2u) & 1u));
    let q = frame.view_proj * vec4<f32>(mix(b.lo, b.hi, corner), 1.0);
    var bits = 0u;
    if q.x < -q.w { bits |= 1u; }
    if q.x > q.w { bits |= 2u; }
    if q.y < -q.w { bits |= 4u; }
    if q.y > q.w { bits |= 8u; }
    if q.z < 0.0 { bits |= 16u; }
    if q.z > q.w { bits |= 32u; }
    outside &= bits;
  }
  if outside != 0u { return; }
  let slot = atomicAdd(&draw_args[b.kind * 4u + 1u], 1u);
  visible[bases[b.kind] + slot] = b.row;
}
";
    s
}

pub fn classify_module() -> String {
    let mut s = PRELUDE.replace("{{NONCE}}", abi::NONCE_PLACEHOLDER);
    s += r"
@group(1) @binding(0) var vis_tex: texture_2d<u32>;
@group(1) @binding(1) var<storage, read_write> shade_args: array<atomic<u32>>;
@group(1) @binding(2) var<storage, read_write> tiles: array<u32>;
var<workgroup> mask: atomic<u32>;

// One workgroup per 8x8 tile: which kinds cover it; append the tile to each kind's list.
@compute @workgroup_size(8, 8) fn classify(@builtin(workgroup_id) wg: vec3<u32>, @builtin(local_invocation_index) li: u32,
    @builtin(global_invocation_id) g: vec3<u32>) {
  if frame.cold.x == LOFT_NONCE { return; }
  if li == 0u { atomicStore(&mask, 0u); }
  workgroupBarrier();
  if g.x < frame.size.x && g.y < frame.size.y {
    let v = textureLoad(vis_tex, g.xy, 0).x;
    if v != LOFT_EMPTY { atomicOr(&mask, 1u << (v >> 24u)); }
  }
  workgroupBarrier();
  if li == 0u {
    var m = atomicLoad(&mask);
    loop {
      if m == 0u { break; }
      let k = firstTrailingBit(m);
      let slot = atomicAdd(&shade_args[k * 3u], 1u);
      tiles[k * frame.size.w + slot] = wg.x | (wg.y << 16u);
      m &= m - 1u;
    }
  }
}
";
    s
}

/// `sky` is the library fn named by the schedule's present pass.
pub fn present_module(p: &Prog, types: &[Ty], sky: &str) -> String {
    let mut em = Emitter::new(p, types);
    em.header(abi::NONCE_PLACEHOLDER);
    let (lib, consts) = reachable(p, &[], &[sky]);
    em.library(&lib, &consts);
    em.out += &format!(
        r"@group(1) @binding(0) var hdr_tex: texture_2d<f32>;
@group(1) @binding(1) var vis_tex: texture_2d<u32>;

@vertex fn vs_present(@builtin(vertex_index) vi: u32) -> @builtin(position) vec4<f32> {{
  let uv = vec2<f32>(f32((vi << 1u) & 2u), f32(vi & 2u));
  return vec4<f32>(uv * 2.0 - 1.0, 0.0, 1.0);
}}

// ACES filmic fit (Narkowicz 2015); the target is sRGB, so the hardware encodes.
fn loft_aces(x: vec3<f32>) -> vec3<f32> {{
  return clamp((x * (2.51 * x + 0.03)) / (x * (2.43 * x + 0.59) + 0.14), vec3<f32>(0.0), vec3<f32>(1.0));
}}

@fragment fn fs_present(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<f32> {{
  if frame.cold.x == LOFT_NONCE {{ discard; }}
  let px = vec2<u32>(pos.xy);
  var c = textureLoad(hdr_tex, px, 0).rgb;
  if textureLoad(vis_tex, px, 0).x == LOFT_EMPTY {{ c = {sky}(loft_ray(pos.xy), normalize(frame.sun.xyz)); }}
  return vec4<f32>(loft_aces(c), 1.0);
}}
"
    );
    em.out
}
