//! One graph function's WGSL body (plan §1.10 emit, §3.3, §7 branch-local `if`).
//!
//! Two passes over the nodes the result reaches. **Place** visits users before operands (descending ids: operands
//! precede users) and puts each statement in the deepest block holding all its uses: the root, an arm of a `select`
//! (so a value only one arm reads is computed under a real `if`), or a loop body. A node that reads none of a loop's
//! binders is lifted out of that loop. **Write** walks from the result in operand order and names each statement by
//! first use (`v0`, `v1`, ...), so the text depends on the graph's structure and never on node ids, creation order or
//! discarded nodes. Both passes are structural, so the same function gives the same text in any job.
use super::{Line, Slots, lit, spell, ty_name};
use loft_ir::graph::{Graph, Node, Op, Prim, Ty};
use loft_ir::{Error, error::SceneFault};
use std::collections::{HashMap, HashSet};
use std::fmt::Write;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Root,
    Arm,
    Body(u32),
}
struct Block {
    parent: u32,
    depth: u32,
    kind: Kind,
}

/// Where a use sits. A loop's count and starts are read where the loop statement goes, its step and stop in its body;
/// both are known only once every result of the loop is placed, so they resolve when the operand is placed.
#[derive(Clone, Copy)]
enum Use {
    At(u32),
    Stmt(u32),
    Body(u32),
}

/// What the module needs from a body: functions it calls and raw functions.
#[derive(Default)]
pub struct Needs {
    pub calls: Vec<u32>,
    /// (calling node, raw function) per raw call.
    pub raws: Vec<(u32, u32)>,
}

pub struct Body<'g> {
    g: &'g Graph,
    at: &'g Slots,
    raw_names: &'g [String],
    /// The called functions' names, which the module fills in after `place` reports the calls.
    pub fn_names: HashMap<u32, String>,
    blocks: Vec<Block>,
    place: HashMap<u32, u32>,
    arms: HashMap<(u32, u8), u32>,
    bodies: HashMap<u32, u32>,
    loop_at: HashMap<u32, u32>,
    names: HashMap<u32, String>,
    loops: HashMap<u32, String>,
    bufs: HashMap<u32, Vec<Line>>,
    next: u32,
    pub needs: Needs,
}

pub fn unsupported(g: &Graph, n: u32, what: &str) -> Error {
    Error::Scene { site: g.site(n).cloned(), fault: SceneFault::Unsupported { what: what.into() } }
}

impl<'g> Body<'g> {
    /// Places and writes the body of `result` with parameters `params` (binder nodes, named `a0`, `a1`, ...).
    pub fn new(g: &'g Graph, at: &'g Slots, raw_names: &'g [String], params: &[u32]) -> Self {
        let names = params.iter().enumerate().map(|(k, &b)| (b, format!("a{k}"))).collect();
        let root = Block { parent: 0, depth: 0, kind: Kind::Root };
        #[rustfmt::skip]
        let b = Body { g, at, raw_names, fn_names: HashMap::new(), blocks: vec![root], place: HashMap::new(), arms: HashMap::new(),
            bodies: HashMap::new(), loop_at: HashMap::new(), names, loops: HashMap::new(), bufs: HashMap::new(), next: 0, needs: Needs::default() };
        b
    }

    fn node(&self, n: u32) -> &'g Node {
        &self.g.nodes[n as usize]
    }

    fn child(&mut self, parent: u32, kind: Kind) -> u32 {
        let depth = self.blocks[parent as usize].depth + 1;
        self.blocks.push(Block { parent, depth, kind });
        (self.blocks.len() - 1) as u32
    }

    fn lca(&self, mut a: u32, mut b: u32) -> u32 {
        while a != b {
            let (da, db) = (self.blocks[a as usize].depth, self.blocks[b as usize].depth);
            if da >= db {
                a = self.blocks[a as usize].parent;
            }
            if db >= da {
                b = self.blocks[b as usize].parent;
            }
        }
        a
    }

    /// Out of every enclosing loop whose binders `deps` does not read, stopping at the first loop it does read.
    fn lift(&self, mut b: u32, deps: &[u32]) -> u32 {
        let mut x = b;
        loop {
            match self.blocks[x as usize].kind {
                Kind::Root => return b,
                Kind::Body(l) if deps.contains(&l) => return b,
                Kind::Body(_) => b = self.blocks[x as usize].parent,
                Kind::Arm => {}
            }
            x = self.blocks[x as usize].parent;
        }
    }

    fn body_block(&mut self, l: u32) -> u32 {
        match self.bodies.get(&l) {
            Some(&b) => b,
            None => {
                let b = self.child(self.loop_at[&l], Kind::Body(l));
                self.bodies.insert(l, b);
                b
            }
        }
    }

    /// The place pass. Leaves and literals are written inline and are not placed. Returns the functions called.
    pub fn place(&mut self, result: u32) -> Vec<u32> {
        let g = self.g;
        let mut live = vec![result];
        let mut seen: HashSet<u32> = HashSet::from([result]);
        let mut binder_of: HashMap<u32, u32> = HashMap::new();
        let mut i = 0;
        while i < live.len() {
            let n = live[i];
            i += 1;
            let mut more: Vec<u32> = g.args_of(self.node(n)).to_vec();
            if let Op::Loop { decl, .. } = self.node(n).op {
                let l = &g.loops[decl as usize];
                l.state.iter().chain([&l.index]).for_each(|&b| _ = binder_of.insert(b, decl));
                more.extend(l.nodes());
            }
            for m in more {
                if seen.insert(m) {
                    live.push(m);
                }
            }
        }
        live.sort_unstable();
        // Which loops' binders each node reads (ascending: operands first).
        let mut deps: HashMap<u32, Vec<u32>> = HashMap::new();
        for &n in &live {
            let mut d: Vec<u32> = match self.node(n).op {
                Op::Bind(_) => binder_of.get(&n).map_or(vec![], |&l| vec![l]),
                Op::Loop { decl, .. } => g.loops[decl as usize]
                    .nodes()
                    .flat_map(|r| deps[&r].iter().copied())
                    .filter(|&l| l != decl)
                    .collect(),
                Op::Lit(_)
                | Op::Time
                | Op::Param(_)
                | Op::Input(_)
                | Op::Var(_)
                | Op::Vec
                | Op::Prim(_)
                | Op::Swizzle(_)
                | Op::Call(_)
                | Op::Fetch { .. }
                | Op::Grad
                | Op::Raw(_) => g.args_of(self.node(n)).iter().flat_map(|a| deps[a].iter().copied()).collect(),
            };
            d.sort_unstable();
            d.dedup();
            deps.insert(n, d);
        }
        let mut uses: HashMap<u32, Vec<Use>> = HashMap::from([(result, vec![Use::At(0)])]);
        let mut rooted: HashSet<u32> = HashSet::new();
        for &n in live.iter().rev() {
            let node = self.node(n);
            if matches!(node.op, Op::Lit(_) | Op::Time | Op::Param(_) | Op::Input(_) | Op::Bind(_) | Op::Var(_)) {
                continue;
            }
            let mut b: Option<u32> = None;
            for u in uses.remove(&n).unwrap_or_default() {
                let at = match u {
                    Use::At(x) => x,
                    Use::Stmt(l) => self.loop_at[&l],
                    Use::Body(l) => self.body_block(l),
                };
                b = Some(b.map_or(at, |b| self.lca(b, at)));
            }
            let b = self.lift(b.unwrap_or(0), &deps[&n]);
            self.place.insert(n, b);
            let args = g.args_of(node);
            let mut push = |m: u32, u: Use| uses.entry(m).or_default().push(u);
            match node.op {
                Op::Prim(Prim::Select) => {
                    push(args[0], Use::At(b));
                    for (k, &arm) in args[1..].iter().enumerate() {
                        let blk = self.child(b, Kind::Arm);
                        self.arms.insert((n, k as u8), blk);
                        push(arm, Use::At(blk));
                    }
                }
                Op::Loop { decl, .. } => {
                    let at = self.loop_at.get(&decl).map_or(b, |&x| self.lca(x, b));
                    self.loop_at.insert(decl, at);
                    if rooted.insert(decl) {
                        let l = &g.loops[decl as usize];
                        [l.count].iter().chain(&l.init).for_each(|&r| push(r, Use::Stmt(decl)));
                        l.next.iter().chain(&l.stop).for_each(|&r| push(r, Use::Body(decl)));
                    }
                }
                Op::Prim(_)
                | Op::Vec
                | Op::Swizzle(_)
                | Op::Call(_)
                | Op::Fetch { .. }
                | Op::Grad
                | Op::Raw(_)
                | Op::Lit(_)
                | Op::Time
                | Op::Param(_)
                | Op::Input(_)
                | Op::Bind(_)
                | Op::Var(_) => args.iter().for_each(|&m| push(m, Use::At(b))),
            }
        }
        live.iter()
            .filter_map(|&n| match self.node(n).op {
                Op::Call(d) => Some(d),
                Op::Lit(_)
                | Op::Time
                | Op::Param(_)
                | Op::Input(_)
                | Op::Bind(_)
                | Op::Var(_)
                | Op::Vec
                | Op::Prim(_)
                | Op::Swizzle(_)
                | Op::Fetch { .. }
                | Op::Loop { .. }
                | Op::Grad
                | Op::Raw(_) => None,
            })
            .collect()
    }

    fn emit(&mut self, block: u32, text: String, node: Option<u32>) {
        self.bufs.entry(block).or_default().push(Line { text, node });
    }

    fn fresh(&mut self, p: &str) -> String {
        self.next += 1;
        format!("{p}{}", self.next - 1)
    }

    /// The WGSL expression naming `n`'s value, writing its statement (and its operands') on first use.
    pub fn need(&mut self, n: u32) -> Result<String, Error> {
        if let Some(s) = self.names.get(&n) {
            return Ok(s.clone());
        }
        let (g, node) = (self.g, self.node(n));
        let args = g.args_of(node);
        let ty = node.ty;
        let inline = |s: String| Ok(s);
        let expr = match node.op {
            Op::Lit(i) => return inline(lit(g, i, ty)),
            Op::Time => return inline("lf(frame.time.x)".into()),
            Op::Param(d) => {
                let s = self.at.params[d as usize];
                return inline(format!("lf(frame.params[{}].{})", s / 4, ["x", "y", "z", "w"][s as usize % 4]));
            }
            Op::Input(d) => {
                let s = self.at.inputs[d as usize];
                let w = ty.width().unwrap_or(1) as usize;
                return inline(format!("{}(frame.inputs[{s}].{})", ty_name(ty, "lf"), &"xyzw"[..w]));
            }
            Op::Bind(_) => return Err(unsupported(g, n, "a binder read outside its function or loop")),
            Op::Var(v) => {
                let what = format!("the free variable `{}` (bound by the job's consumer) in WGSL", g.sym(v));
                return Err(unsupported(g, n, &what));
            }
            Op::Loop { decl, k } => {
                self.need_loop(decl)?;
                return Ok(format!("s{}_{k}", self.loops[&decl]));
            }
            Op::Grad => {
                return Err(unsupported(g, n, "a gradient request (rewritten during interning, which has not run)"));
            }
            Op::Prim(Prim::Select) => return self.select(n, args, ty),
            Op::Vec => {
                let xs = args.iter().map(|&a| self.need(a)).collect::<Result<Vec<_>, _>>()?;
                format!("{}({})", ty_name(ty, "lf"), xs.join(", "))
            }
            Op::Prim(p) => {
                let xs = args.iter().map(|&a| self.need(a)).collect::<Result<Vec<_>, _>>()?;
                let tys: Vec<Ty> = args.iter().map(|&a| self.node(a).ty).collect();
                spell(p, &xs, &tys, ty)
            }
            Op::Swizzle(sel) => {
                let x = self.need(args[0])?;
                match (self.node(args[0]).ty, sel.len) {
                    (Ty::F32, 1) => x,
                    (Ty::F32, _) => format!("{}({x})", ty_name(ty, "lf")),
                    (Ty::V2 | Ty::V3 | Ty::V4 | Ty::Bool, _) => {
                        let c: String =
                            (0..sel.len).map(|k| ["x", "y", "z", "w"][(sel.comps >> (2 * k)) as usize & 3]).collect();
                        format!("{x}.{c}")
                    }
                }
            }
            Op::Call(d) => {
                let xs = args.iter().map(|&a| self.need(a)).collect::<Result<Vec<_>, _>>()?;
                self.needs.calls.push(d);
                format!("{}({})", self.fn_names[&d], xs.join(", "))
            }
            Op::Raw(r) => {
                let xs = args.iter().map(|&a| Ok(format!("{}({})", ty_name(self.node(a).ty, "f32"), self.need(a)?)));
                let xs = xs.collect::<Result<Vec<_>, Error>>()?;
                self.needs.raws.push((n, r));
                format!("{}({}({}))", ty_name(ty, "lf"), self.raw_names[r as usize], xs.join(", "))
            }
            Op::Fetch { kernel, lane } => {
                let row = self.need(args[0])?;
                let r = self.at.kernels[kernel as usize];
                // Clamped by construction (plan §4.3): the row index never leaves its resource, so never its pack.
                let (p, off, last, lanes) = (r.pack, r.offset, r.rows.max(1) - 1, r.lanes);
                format!("vec4<lf>(loft_pack{p}[{off}u + min(u32({row}), {last}u) * {lanes}u + {lane}u])")
            }
        };
        let name = self.fresh("v");
        self.emit(self.place[&n], format!("let {name} = {expr};"), Some(n));
        self.names.insert(n, name.clone());
        Ok(name)
    }

    /// `select(c, a, b)`: an arm whose value only that arm reads is computed under a real `if` (plan §7).
    fn select(&mut self, n: u32, args: &[u32], ty: Ty) -> Result<String, Error> {
        let c = self.need(args[0])?;
        let mut arms = Vec::with_capacity(2);
        let mut local = false;
        for (k, &arm) in args[1..].iter().enumerate() {
            let blk = self.arms[&(n, k as u8)];
            let inside = self.within(self.place.get(&arm).copied(), blk);
            let x = self.need(arm)?;
            let x = if self.node(arm).ty == ty { x } else { format!("{}({x})", ty_name(ty, "lf")) };
            local |= inside;
            arms.push((self.bufs.remove(&blk).unwrap_or_default(), x));
        }
        let at = self.place[&n];
        let name = self.fresh("v");
        match local {
            false => self.emit(at, format!("let {name} = select({}, {}, {c});", arms[1].1, arms[0].1), Some(n)),
            true => {
                self.emit(at, format!("var {name}: {};", ty_name(ty, "lf")), Some(n));
                self.emit(at, format!("if ({c}) {{"), Some(n));
                let [(t, tx), (f, fx)]: [_; 2] =
                    arms.try_into().unwrap_or_else(|_| unreachable!("select has two arms"));
                self.wrap(at, t, format!("{name} = {tx};"), n);
                self.emit(at, "} else {".into(), Some(n));
                self.wrap(at, f, format!("{name} = {fx};"), n);
                self.emit(at, "}".into(), Some(n));
            }
        }
        self.names.insert(n, name.clone());
        Ok(name)
    }

    fn within(&self, b: Option<u32>, of: u32) -> bool {
        let mut x = match b {
            Some(x) => x,
            None => return false,
        };
        loop {
            match (x == of, self.blocks[x as usize].kind) {
                (true, _) => return true,
                (false, Kind::Root) => return false,
                (false, Kind::Arm | Kind::Body(_)) => x = self.blocks[x as usize].parent,
            }
        }
    }

    fn wrap(&mut self, at: u32, lines: Vec<Line>, last: String, n: u32) {
        for l in lines {
            self.emit(at, format!("  {}", l.text), l.node);
        }
        self.emit(at, format!("  {last}"), Some(n));
    }

    /// The loop statement: state in `var`s, the body in a `for` capped by the count, every next value read before any
    /// is assigned (a step may swap two states), then the stop test, as the TS emitter wrote it.
    fn need_loop(&mut self, l: u32) -> Result<(), Error> {
        if self.loops.contains_key(&l) {
            return Ok(());
        }
        let lp = &self.g.loops[l as usize];
        let (count, init, binders, index, next, stop) = (lp.count, &lp.init, &lp.state, lp.index, &lp.next, lp.stop);
        let at = self.loop_at[&l];
        let id = self.fresh("L");
        let starts = init.iter().map(|&i| self.need(i)).collect::<Result<Vec<_>, _>>()?;
        for (k, s) in starts.iter().enumerate() {
            self.emit(at, format!("var s{id}_{k}: {} = {s};", ty_name(self.node(init[k]).ty, "lf")), None);
            self.names.insert(binders[k], format!("s{id}_{k}"));
        }
        let cap = self.need(count)?;
        self.names.insert(index, format!("lf(i{id})"));
        self.loops.insert(l, id.clone());
        let body = self.body_block(l);
        let nexts = next.iter().map(|&x| self.need(x)).collect::<Result<Vec<_>, _>>()?;
        let stop = stop.map(|s| self.need(s)).transpose()?;
        let mut tail = String::new();
        for (k, x) in nexts.iter().enumerate() {
            _ = write!(tail, "let n{id}_{k} = {x}; ");
        }
        for k in 0..nexts.len() {
            _ = write!(tail, "s{id}_{k} = n{id}_{k}; ");
        }
        if let Some(s) = stop {
            _ = write!(tail, "if ({s}) {{ break; }}");
        }
        let lines = self.bufs.remove(&body).unwrap_or_default();
        self.emit(at, format!("for (var i{id} = 0u; i{id} < u32({cap}); i{id}++) {{"), None);
        for ln in lines {
            self.emit(at, format!("  {}", ln.text), ln.node);
        }
        self.emit(at, format!("  {}", tail.trim_end()), None);
        self.emit(at, "}".into(), None);
        Ok(())
    }

    /// The written statements of the root block.
    pub fn lines(mut self) -> (Vec<Line>, Needs) {
        (self.bufs.remove(&0).unwrap_or_default(), self.needs)
    }
}
