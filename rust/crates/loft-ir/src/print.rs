//! The canonical print (plan §8.2, lane 1.2's shadow check): the text src/record/print.ts writes for the same roots,
//! byte for byte. Its grammar is in that file's header. In short: the program's roots walked post-order with every
//! call inlined, one line per distinct text, numbered by first use; loop binders print as `bind TY LEVEL SLOT`, LEVEL
//! counting the loops they are inside, so two equal loops are one line. After the roots line, one `kernel` line per
//! table in the order the walk first fetched it, its fill walked after the roots (binders as `fill TY SLOT`), then one
//! line per scene-level decl (cameras, media, draws, passes, checks, solves, each in decl order), walked before the
//! fills; a graph function a decl names prints as its body over `arg TY K` binders.
//!
//! The walk recurses once per graph level (operand, call body, loop body); callers on deep graphs run it on a thread
//! with a large stack.
use crate::error::{Error, Fault, Internal, Span, WireFault};
use crate::graph::{
    Blend, Cells, Clear, Cover, Fill, Find, Formula, Graph, Ground, HoldKind, Keep, Kernel, Layer, Lens, Medium, Op,
    OutShape, Outline, Part, Proxy, Read, Recur, Size, Strategy, Target, Times, When,
};
use std::collections::HashMap;
use std::fmt::Write;

/// The canonical print of `g`'s program roots. Err names the node it cannot print: a binder read outside its loop or
/// function (`Fault::Scope`), or a `Grad` (the wave-1 recorder writes derivatives expanded).
pub fn print(g: &Graph) -> Result<String, Error> {
    let mut p = Printer {
        g,
        lines: Vec::new(),
        levels: Vec::new(),
        ids: HashMap::new(),
        global: HashMap::new(),
        scopes: vec![Scope { level: 0, bound: HashMap::new(), frame: 0, memo: HashMap::new(), parent: None }],
        frames: vec![HashMap::new()],
        loop_lines: HashMap::new(),
        tables: Vec::new(),
        noted: Vec::new(),
    };
    let roots = g.roots.iter().map(|r| p.walk(*r, 0, false)).collect::<Result<Vec<u32>, Error>>()?;
    let decls = p.decls()?;
    // Each fetched table's fill (`tables` grows while fills are walked).
    let mut kernels = Vec::new();
    let mut j = 0;
    while let Some(&k) = p.tables.get(j) {
        kernels.push(p.fill(k)?);
        j += 1;
    }
    let mut out = String::new();
    for l in &p.lines {
        out.push_str(l);
        out.push('\n');
    }
    out.push_str("roots");
    out.push(' ');
    out.push_str(&refs(&roots));
    for k in kernels.iter().chain(&decls) {
        out.push('\n');
        out.push_str(k);
    }
    Ok(out)
}

/// One walk position: its loop level, the loop binders it reads (node -> line), its call frame and its memo. A loop
/// scope reads through to its parent's memo; a call scope starts a new frame and does not (its body's binders mean
/// that call's arguments).
struct Scope {
    level: u32,
    bound: HashMap<u32, u32>,
    frame: usize,
    memo: HashMap<u32, u32>,
    parent: Option<usize>,
}

/// A call's arguments: binder node -> (argument node, caller scope, whether the caller is inside a call).
type Frame = HashMap<u32, (u32, usize, bool)>;

struct Printer<'g> {
    g: &'g Graph,
    lines: Vec<String>,
    /// Per line, the loop levels its value reads: bit L is a binder of a loop at level L (JS `1 << L`, so mod 32).
    levels: Vec<u32>,
    ids: HashMap<String, u32>,
    /// Lines of values outside every call that read no binder, for the whole walk.
    global: HashMap<u32, u32>,
    scopes: Vec<Scope>,
    frames: Vec<Frame>,
    /// (loop, level, frame) -> its line: re-walking would give the same line, so its results share one walk.
    loop_lines: HashMap<(u32, u32, usize), u32>,
    /// Table kernels in the order the walk first fetched them, and compute kernels as raw reads first named them.
    tables: Vec<u32>,
    /// Raw functions whose reads are listed (each once, its deps' first).
    noted: Vec<u32>,
}

fn refs(ks: &[u32]) -> String {
    let mut s = String::new();
    for (j, k) in ks.iter().enumerate() {
        let _ = write!(s, "{}#{k}", if j == 0 { "" } else { " " });
    }
    s
}

/// FNV-1a, 32 bit, each word xor-ed whole (print.ts `fnv`).
fn fnv(xs: impl Iterator<Item = u32>) -> String {
    let h = xs.fold(0x811c_9dc5u32, |h, x| (h ^ x).wrapping_mul(0x0100_0193));
    format!("{h:08x}")
}

/// print.ts `codes`: `Array.from(s, c => c.charCodeAt(0))` iterates code points and keeps each one's first UTF-16
/// unit, so a character outside the BMP contributes only its high surrogate. Replicated, not corrected: the hash must
/// equal the TS print's.
fn code_units(s: &str) -> impl Iterator<Item = u32> + '_ {
    s.chars().map(|c| {
        let c = u32::from(c);
        if c > 0xffff { 0xd800 + ((c - 0x1_0000) >> 10) } else { c }
    })
}

fn hex(bits: &[u64]) -> String {
    let mut s = String::new();
    for (j, b) in bits.iter().enumerate() {
        let _ = write!(s, "{}{b:x}", if j == 0 { "" } else { " " });
    }
    s
}

fn unprintable(node: u32, what: Fault) -> Error {
    Error::Internal(Internal::Wire(WireFault { span: Span::Nodes, at: u64::from(node), what }))
}

impl Printer<'_> {
    fn line(&mut self, text: String, levels: u32) -> u32 {
        if let Some(k) = self.ids.get(&text) {
            return *k;
        }
        let k = self.lines.len() as u32;
        self.ids.insert(text.clone(), k);
        self.lines.push(text);
        self.levels.push(levels);
        k
    }
    fn op(&mut self, head: String, kids: &[u32]) -> u32 {
        let levels = kids.iter().fold(0, |m, k| m | self.levels[*k as usize]);
        let text = format!("{head} {}", refs(kids));
        self.line(text.trim_end().into(), levels)
    }
    fn reads(&self, ks: &[u32]) -> u32 {
        ks.iter().fold(0, |m, k| m | self.levels[*k as usize])
    }

    fn walk(&mut self, id: u32, s: usize, in_call: bool) -> Result<u32, Error> {
        if let Some(k) = self.scopes[s].bound.get(&id) {
            return Ok(*k);
        }
        // A call's argument is walked where its binder is first read, not before the body: the TS graph, which
        // inlined the call, meets it there.
        if let Some((arg, caller, caller_in_call)) = self.frames[self.scopes[s].frame].get(&id).copied() {
            return self.walk(arg, caller, caller_in_call);
        }
        if let Some(k) = self.find(s, id) {
            return Ok(k);
        }
        let k = self.node(id, s, in_call)?;
        if self.levels[k as usize] == 0 && !in_call {
            self.global.insert(id, k);
        } else {
            self.scopes[s].memo.insert(id, k);
        }
        Ok(k)
    }
    fn find(&self, s: usize, id: u32) -> Option<u32> {
        if let Some(k) = self.global.get(&id) {
            return Some(*k);
        }
        let mut at = Some(s);
        while let Some(x) = at {
            if let Some(k) = self.scopes[x].memo.get(&id) {
                return Some(*k);
            }
            at = self.scopes[x].parent;
        }
        None
    }

    fn node(&mut self, id: u32, s: usize, in_call: bool) -> Result<u32, Error> {
        let g = self.g;
        let n = &g.nodes[id as usize];
        let ty = n.ty.name();
        let kids = |p: &mut Self| g.args_of(n).iter().map(|a| p.walk(*a, s, in_call)).collect::<Result<Vec<_>, _>>();
        Ok(match n.op {
            Op::Prim(prim) => {
                let ks = kids(self)?;
                self.op(format!("{} {ty}", prim.name()), &ks)
            }
            Op::Lit(i) => {
                let text = format!("lit {ty} {}", hex(&g.lits[i as usize..i as usize + usize::from(n.ty.lits())]));
                self.line(text, 0)
            }
            Op::Time => self.line("time".into(), 0),
            Op::Param(k) => {
                let p = &g.params[k as usize];
                let bits = [p.def, p.min, p.max].map(|l| g.lits[l as usize]);
                self.line(format!("param {} {}", g.sym(p.name), hex(&bits)), 0)
            }
            Op::Input(k) => {
                let d = &g.inputs[k as usize];
                let def = &g.lits[d.def as usize..d.def as usize + usize::from(n.ty.lits())];
                self.line(format!("input {ty} {} {}", g.sym(d.name), hex(def)), 0)
            }
            Op::Var(name) => self.line(format!("var {ty} {}", g.sym(name)), 0),
            Op::Bind(_) => return Err(unprintable(id, Fault::Scope)),
            Op::Fetch { kernel, lane } => {
                let res = match &g.kernels[kernel as usize] {
                    Kernel::Table { .. } | Kernel::Compute(_) => self.table_name(kernel),
                    Kernel::Rows { label, at, len } => {
                        let rows = &g.rows[*at as usize..(at + len) as usize];
                        format!("rows:{}:{}", g.sym(*label), fnv(rows.iter().copied()))
                    }
                };
                let ks = kids(self)?;
                // Listed after its index is walked, as print.ts does: the index may fetch another table first.
                if matches!(g.kernels[kernel as usize], Kernel::Table { .. }) && !self.tables.contains(&kernel) {
                    self.tables.push(kernel);
                }
                self.op(format!("fetch {res} {lane}"), &ks)
            }
            Op::Raw(k) => {
                let r = &g.raws[k as usize];
                let mut head = format!("raw {} {ty} {}", g.sym(r.name), fnv(code_units(g.sym(r.text))));
                if !r.reads.is_empty() {
                    let reads: Vec<String> = r.reads.iter().map(|x| self.read_name(*x)).collect();
                    let _ = write!(head, " reads [{}]", reads.join(" "));
                }
                let ks = kids(self)?;
                // After its arguments, as print.ts does: an argument may name another table first.
                self.note(k);
                self.op(head, &ks)
            }
            Op::Swizzle(sel) => {
                let pick: String =
                    (0..sel.len).map(|k| b"xyzw"[usize::from((sel.comps >> (2 * k)) & 3)] as char).collect();
                let ks = kids(self)?;
                self.op(format!("get {ty} {pick}"), &ks)
            }
            Op::Vec => {
                let ks = kids(self)?;
                self.op(format!("vec {ty}"), &ks)
            }
            Op::Loop { decl, k } => {
                let l = self.walk_loop(decl, s, in_call)?;
                self.op(format!("iter {ty} {k}"), &[l])
            }
            Op::Call(f) => {
                let f = &g.fns[f as usize];
                let args = g.args_of(n);
                self.frames.push(f.binders.iter().zip(args).map(|(b, a)| (*b, (*a, s, in_call))).collect());
                let level = self.scopes[s].level;
                let frame = self.frames.len() - 1;
                self.scopes.push(Scope { level, bound: HashMap::new(), frame, memo: HashMap::new(), parent: None });
                let body = self.scopes.len() - 1;
                self.walk(f.body, body, true)?
            }
            Op::Grad => return Err(unprintable(id, Fault::Op { code: 8 })),
        })
    }

    fn table_name(&self, k: u32) -> String {
        match &self.g.kernels[k as usize] {
            Kernel::Table { name, lanes, entries, role, .. } => {
                format!("table:{}:{lanes}:{entries}:{}", self.g.sym(*name), role.name())
            }
            Kernel::Rows { label, .. } => format!("rows:{}", self.g.sym(*label)),
            Kernel::Compute(c) => format!("compute:{}", self.g.sym(c.name)),
        }
    }
    fn read_name(&self, r: Read) -> String {
        let g = self.g;
        match r {
            Read::Table(k) => self.table_name(k),
            Read::Output { kernel, output } => match &g.kernels[kernel as usize] {
                Kernel::Compute(c) => format!("compute:{}:{}", g.sym(c.name), g.sym(c.writes[output as usize].key)),
                Kernel::Table { .. } | Kernel::Rows { .. } => unreachable!("decode checked a read output's kernel"),
            },
            Read::Pass(p) => match g.passes[p as usize].target {
                Target::Texture { name, .. } => format!("pass:{}", g.sym(name)),
                Target::View => unreachable!("decode checked a read pass's target"),
            },
        }
    }
    /// List the kernels raw function `r` reads (its deps' first, each raw once), as print.ts `note` does.
    fn note(&mut self, r: u32) {
        if self.noted.contains(&r) {
            return;
        }
        self.noted.push(r);
        let raw = &self.g.raws[r as usize];
        for d in raw.deps.iter() {
            self.note(*d);
        }
        for x in raw.reads.iter() {
            let k = match *x {
                Read::Table(k) | Read::Output { kernel: k, .. } => k,
                Read::Pass(_) => continue,
            };
            if !self.tables.contains(&k) {
                self.tables.push(k);
            }
        }
    }

    /// A scope for a fill: one level in, its binders bound to `fill TY SLOT` lines (print.ts `Canon.fill`).
    fn fill_scope(&mut self, binders: &[(u32, String)]) -> usize {
        self.scoped("fill", binders)
    }

    /// Graph function `f` as its body's line, over binders printed `arg TY K` one level in.
    fn fn_ref(&mut self, f: u32) -> Result<String, Error> {
        let binders: Vec<(u32, String)> =
            self.g.fns[f as usize].binders.iter().enumerate().map(|(k, b)| (*b, k.to_string())).collect();
        let s = self.scoped("arg", &binders);
        Ok(format!("#{}", self.walk(self.g.fns[f as usize].body, s, false)?))
    }
    /// A scope one level in whose binders print as `WORD TY SLOT`.
    fn scoped(&mut self, word: &str, binders: &[(u32, String)]) -> usize {
        let mut bound = HashMap::new();
        for (b, slot) in binders {
            let k = self.line(format!("{word} {} {slot}", self.g.nodes[*b as usize].ty.name()), 1);
            bound.insert(*b, k);
        }
        self.scopes.push(Scope { level: 1, bound, frame: 0, memo: HashMap::new(), parent: None });
        self.scopes.len() - 1
    }
    fn lits(&self, at: u32, n: usize) -> String {
        hex(&self.g.lits[at as usize..at as usize + n])
    }
    fn root_refs(&mut self, xs: &[u32], s: usize) -> Result<String, Error> {
        let ks = xs.iter().map(|x| self.walk(*x, s, false)).collect::<Result<Vec<_>, _>>()?;
        Ok(refs(&ks))
    }

    /// The scene-level decls' lines, in print.ts's order.
    fn decls(&mut self) -> Result<Vec<String>, Error> {
        let g = self.g;
        let mut out = Vec::new();
        for c in g.cameras.iter() {
            let line = match c.lens {
                Lens::Look { eye, target, up, fovy } => {
                    format!("look [{}]", self.root_refs(&[eye, target, up, fovy], 0)?)
                }
                Lens::Matrix { cols } => format!("matrix [{}]", self.root_refs(&cols, 0)?),
                Lens::Table { kernel } => format!("table {}", self.table_name(kernel)),
            };
            out.push(format!("camera {} {line}", g.sym(c.name)));
        }
        for m in g.media.iter() {
            out.push(match *m {
                Medium::March { density, within, steps, shadow_steps } => format!(
                    "medium march {} within {} steps {steps} {shadow_steps}",
                    self.fn_ref(density)?,
                    self.lits(within, 6)
                ),
                Medium::Gaussians { kernel } => format!("medium gaussians {}", self.table_name(kernel)),
            });
        }
        for d in g.draws.iter() {
            // The paint first, then the cover's functions: print.ts's order.
            let paint = self.fn_ref(d.paint)?;
            let cover = match &d.cover {
                Cover::Grid { u, v, place, count } => format!("grid {u} {v} {} {count}", self.fn_ref(*place)?),
                Cover::Mesh { kernel, count } => format!("mesh {} {count}", self.table_name(*kernel)),
                Cover::Quads { place, count } => format!("quads {} {count}", self.fn_ref(*place)?),
                Cover::Ray { proxy, find } => {
                    let proxy = match proxy {
                        Proxy::Box { lits } => format!("box {}", self.lits(*lits, 6)),
                        Proxy::Instances { kernel, local } => {
                            format!("instances {} {}", self.table_name(*kernel), self.lits(*local, 6))
                        }
                    };
                    let find = match find {
                        Find::Hit(f) => format!("hit {}", self.fn_ref(*f)?),
                        Find::Medium(m) => format!("medium {m}"),
                    };
                    format!("ray {proxy} {find}")
                }
                Cover::Screen => "screen".into(),
            };
            let layer = match d.layer {
                Layer::Subject => "subject",
                Layer::Surroundings => "surroundings",
            };
            let blend = match d.raster.blend {
                Blend::Opaque => "opaque",
                Blend::Alpha => "alpha",
                Blend::Add => "add",
            };
            let (cull, depth) = (u8::from(d.raster.cull), u8::from(d.raster.depth_write));
            out.push(format!("draw {} {layer} {cull} {depth} {blend} paint {paint} {cover}", g.sym(d.name)));
        }
        for p in g.passes.iter() {
            let clear = match p.clear {
                Clear::Load => "load".to_string(),
                Clear::Colour(at) => self.lits(at, 4),
            };
            let ground = match p.ground {
                Ground::None => "none".to_string(),
                Ground::Traced { hit, paint } => format!("{} {}", self.fn_ref(hit)?, self.fn_ref(paint)?),
            };
            let outline = match p.outline {
                Outline::None => "none".to_string(),
                Outline::Ink(f) => self.fn_ref(f)?,
            };
            let target = match p.target {
                Target::View => "view".to_string(),
                Target::Texture { name, size, keep, scene } => {
                    let size = match size {
                        Size::View(scale) => format!("view {}", self.lits(scale, 1)),
                        Size::Fixed(w, h) => format!("fixed {w} {h}"),
                    };
                    let keep = match keep {
                        Keep::Colour => "colour",
                        Keep::Depth => "depth",
                        Keep::Light => "light",
                    };
                    format!("texture {} {size} {keep} {}", g.sym(name), u8::from(scene))
                }
            };
            out.push(format!(
                "pass camera {} draws {} {} clear {clear} ground {ground} outline {outline} target {target}",
                p.camera, p.draws[0], p.draws[1]
            ));
        }
        for c in g.checks.iter() {
            let how = match c.how {
                Strategy::Prove { boxes } => format!("prove {boxes}"),
                Strategy::Sample { n } => format!("sample {n}"),
            };
            let body = self.formula(&c.body)?;
            out.push(format!("check {} {how} {body}", g.sym(c.label)));
        }
        for sv in g.solves.iter() {
            let mut binders = Vec::new();
            for (k, u) in sv.unknowns.iter().enumerate() {
                binders.extend(u.jets.iter().enumerate().map(|(d, j)| (*j, format!("{k} {d}"))));
            }
            let mut unknowns = Vec::new();
            for u in sv.unknowns.iter() {
                let from = self.walk(u.from, 0, false)?;
                unknowns.push(format!("{} {} #{from}", g.sym(u.name), u.numbers));
            }
            let mut near = Vec::new();
            for n in sv.near.iter() {
                near.push(format!("{} #{}", n.unknown, self.walk(n.to, 0, false)?));
            }
            let s = self.scoped("jet", &binders);
            let mut holds = Vec::new();
            for h in sv.holds.iter() {
                let kind = match h.kind {
                    HoldKind::Equal { margin, within } => {
                        format!("equal #{} {}", self.walk(margin, s, false)?, self.lits(within, 1))
                    }
                    HoldKind::Bound { margin } => format!("bound #{}", self.walk(margin, s, false)?),
                };
                let times = match h.times {
                    Times::At { t } => format!("at {}", self.lits(t, 1)),
                    Times::During { lits } => format!("during {}", self.lits(lits, 2)),
                    Times::Span => "span".into(),
                };
                holds.push(format!("{kind} {times} {} {}", g.sym(h.said), g.sym(h.site)));
            }
            out.push(format!(
                "solve {} {} unknowns [{}] near [{}] holds [{}]",
                g.sym(sv.label),
                self.lits(sv.numbers, 6),
                unknowns.join(", "),
                near.join(", "),
                holds.join(", ")
            ));
        }
        Ok(out)
    }

    fn formula(&mut self, f: &Formula) -> Result<String, Error> {
        Ok(match f {
            Formula::Atom { margin, test } => format!("(atom {})", self.root_refs(&[*margin, *test], 0)?),
            Formula::Always(q) | Formula::Eventually(q) => {
                let word = if matches!(f, Formula::Always(_)) { "always" } else { "eventually" };
                let mut leaves = Vec::new();
                for ((l, lo), hi) in q.leaves.iter().zip(&q.lo).zip(&q.hi) {
                    leaves.push(self.root_refs(&[*l, *lo, *hi], 0)?);
                }
                format!("({word} [{}] {})", leaves.join(", "), self.formula(&q.body)?)
            }
            Formula::And(parts) | Formula::Or(parts) => {
                let word = if matches!(f, Formula::And(_)) { "and" } else { "or" };
                let ps = parts.iter().map(|p| self.formula(p)).collect::<Result<Vec<_>, _>>()?;
                format!("({word} {})", ps.join(" "))
            }
            Formula::Not(p) => format!("(not {})", self.formula(p)?),
        })
    }

    /// Table `k`'s line: its fill walked over its binders.
    fn fill(&mut self, k: u32) -> Result<String, Error> {
        let g = self.g;
        let head = format!("kernel {}", self.table_name(k));
        let (fill, lanes, entries) = match &g.kernels[k as usize] {
            Kernel::Table { fill, lanes, entries, .. } => (fill, lanes, entries),
            Kernel::Compute(c) => {
                let when = match c.when {
                    When::Build => "build".to_string(),
                    When::Load => "load".to_string(),
                    When::Frame => "frame".to_string(),
                    When::Steps { start, count, rate, keep } => {
                        format!("steps {} {count} {} {keep}", g.sym(start), self.lits(rate, 1))
                    }
                };
                let libs: Vec<String> = c
                    .libs
                    .iter()
                    .map(|r| {
                        let r = &g.raws[*r as usize];
                        format!("{}:{}", g.sym(r.name), fnv(code_units(g.sym(r.text))))
                    })
                    .collect();
                let writes: Vec<String> = c
                    .writes
                    .iter()
                    .map(|w| {
                        let shape = match w.shape {
                            OutShape::Buffer { entries, lanes } => format!("buffer {entries} {lanes}"),
                            OutShape::Texture { dims, size, format } => {
                                format!("texture {dims} {} {} {} {}", size[0], size[1], size[2], g.sym(format))
                            }
                        };
                        format!("{} {} {shape}", g.sym(w.key), g.sym(w.symbol))
                    })
                    .collect();
                // The libs' reads first, then the values: print.ts's order.
                for r in c.libs.iter() {
                    self.note(*r);
                }
                let values = c.values.iter().map(|x| self.walk(*x, 0, false)).collect::<Result<Vec<_>, _>>()?;
                let [x, y, z] = c.dispatch;
                return Ok(format!(
                    "{head} {when} entry {} dispatch {x} {y} {z} text {} libs [{}] writes [{}] values [{}]",
                    g.sym(c.entry),
                    fnv(code_units(g.sym(c.text))),
                    libs.join(" "),
                    writes.join(", "),
                    refs(&values)
                ));
            }
            Kernel::Rows { .. } => unreachable!("only tables and compute kernels are listed"),
        };
        Ok(match fill {
            Fill::Map(runs) => {
                let mut text = format!("{head} map");
                for r in runs.iter() {
                    let s = self.fill_scope(&[(r.index, "i".into())]);
                    let ls = r.lanes.iter().map(|x| self.walk(*x, s, false)).collect::<Result<Vec<_>, _>>()?;
                    let _ = write!(text, " {} [{}]", r.count, refs(&ls));
                }
                text
            }
            Fill::Scan(sc) => {
                let init = sc.init.iter().map(|x| self.walk(*x, 0, false)).collect::<Result<Vec<_>, _>>()?;
                let mut binders: Vec<(u32, String)> =
                    sc.state.iter().enumerate().map(|(j, b)| (*b, format!("s{j}"))).collect();
                binders.push((sc.index, "i".into()));
                let s = self.fill_scope(&binders);
                let next = sc.next.iter().map(|x| self.walk(*x, s, false)).collect::<Result<Vec<_>, _>>()?;
                let ls = sc.lanes.iter().map(|x| self.walk(*x, s, false)).collect::<Result<Vec<_>, _>>()?;
                format!("{head} scan {} {} [{}] -> [{}] [{}]", sc.first, sc.steps, refs(&init), refs(&next), refs(&ls))
            }
            Fill::Data { at } => {
                let len = (entries * lanes * 4) as usize;
                format!("{head} data {}", fnv(g.rows[*at as usize..*at as usize + len].iter().copied()))
            }
            Fill::Recurrence(Recur::Part { part, kept }) => {
                let part = match part {
                    Part::Prev => "prev",
                    Part::Even => "even",
                    Part::Odd => "odd",
                };
                if !self.tables.contains(kept) {
                    self.tables.push(*kept);
                }
                format!("{head} recurrence {part} of {}", self.table_name(*kept))
            }
            Fill::Recurrence(Recur::Kept(x)) => {
                let cells = match x.cells {
                    Cells::List(n) => format!("list {n}"),
                    Cells::Grid(w, h) => format!("grid {w} {h}"),
                };
                let s = self.fill_scope(&[(x.index, "i".into()), (x.clock, "t".into())]);
                let start = x.start.iter().map(|r| self.walk(*r, s, false)).collect::<Result<Vec<_>, _>>()?;
                let step = x.step.iter().map(|r| self.walk(*r, s, false)).collect::<Result<Vec<_>, _>>()?;
                format!(
                    "{head} recurrence {cells} rate {} from {} until {} first {} steps {} every {} [{}] -> [{}]",
                    self.lits(x.rate, 1),
                    self.lits(x.from, 1),
                    self.lits(x.until, 1),
                    x.first,
                    x.steps,
                    x.every,
                    refs(&start),
                    refs(&step)
                )
            }
        })
    }

    fn walk_loop(&mut self, decl: u32, s: usize, in_call: bool) -> Result<u32, Error> {
        let (level, frame) = (self.scopes[s].level, self.scopes[s].frame);
        if let Some(k) = self.loop_lines.get(&(decl, level, frame)) {
            return Ok(*k);
        }
        let g = self.g;
        let l = &g.loops[decl as usize];
        let count = self.walk(l.count, s, in_call)?;
        let init = l.init.iter().map(|x| self.walk(*x, s, in_call)).collect::<Result<Vec<_>, _>>()?;
        let bound = self.scopes[s].bound.clone();
        self.scopes.push(Scope { level: level + 1, bound, frame, memo: HashMap::new(), parent: Some(s) });
        let body = self.scopes.len() - 1;
        let bit = 1u32.wrapping_shl(level);
        for (j, b) in l.state.iter().chain([&l.index]).enumerate() {
            let slot = if j == l.state.len() { "i".to_string() } else { format!("s{j}") };
            let k = self.line(format!("bind {} {level} {slot}", g.nodes[*b as usize].ty.name()), bit);
            self.scopes[body].bound.insert(*b, k);
        }
        let next = l.next.iter().map(|x| self.walk(*x, body, in_call)).collect::<Result<Vec<_>, _>>()?;
        let stop = l.stop.map(|x| self.walk(x, body, in_call)).transpose()?;
        let outer = self.reads(&[count]) | self.reads(&init) | (self.reads(&next) | self.reads(stop.as_slice())) & !bit;
        let until = stop.map_or("-".to_string(), |k| format!("#{k}"));
        let text = format!("loop #{count} [{}] -> [{}] until {until}", refs(&init), refs(&next));
        let k = self.line(text, outer);
        self.loop_lines.insert((decl, level, frame), k);
        Ok(k)
    }
}
