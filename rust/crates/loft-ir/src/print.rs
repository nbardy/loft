//! The canonical print (plan §8.2, lane 1.2's shadow check): the text src/record/print.ts writes for the same roots,
//! byte for byte. Its grammar is in that file's header. In short: the program's roots walked post-order with every
//! call inlined, one line per distinct text, numbered by first use; loop binders print as `bind TY LEVEL SLOT`, LEVEL
//! counting the loops they are inside, so two equal loops are one line.
//!
//! The walk recurses once per graph level (operand, call body, loop body); callers on deep graphs run it on a thread
//! with a large stack.
use crate::error::{Error, Fault, Internal, Span, WireFault};
use crate::graph::{Graph, Kernel, Op};
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
    };
    let roots = g.roots.iter().map(|r| p.walk(*r, 0, false)).collect::<Result<Vec<u32>, Error>>()?;
    let mut out = String::new();
    for l in &p.lines {
        out.push_str(l);
        out.push('\n');
    }
    out.push_str("roots");
    out.push(' ');
    out.push_str(&refs(&roots));
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
                let res = match g.kernels[kernel as usize] {
                    Kernel::Table { name, lanes, entries, role } => {
                        format!("table:{}:{lanes}:{entries}:{}", g.sym(name), role.name())
                    }
                    Kernel::Rows { label, at, len } => {
                        let rows = &g.rows[at as usize..(at + len) as usize];
                        format!("rows:{}:{}", g.sym(label), fnv(rows.iter().copied()))
                    }
                };
                let ks = kids(self)?;
                self.op(format!("fetch {res} {lane}"), &ks)
            }
            Op::Raw(r) => {
                let r = &g.raws[r as usize];
                let head = format!("raw {} {ty} {}", g.sym(r.name), fnv(code_units(g.sym(r.text))));
                let ks = kids(self)?;
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
