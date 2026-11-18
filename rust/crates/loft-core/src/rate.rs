//! Rates (plan §1.4, §1.10): which clocks a node depends on. `frame`: time, a param, an input, a curve row or a
//! resource refilled per frame. `item`: a per-invocation binder of the program being planned (a kernel's row index, a
//! draw's vertex), or a raw function, which may read what only its program binds. A node with neither is Const: the
//! compiler computes it on the tape. Binders owned by a `Fn` or `Loop` decl are neutral, so a call or a loop has the
//! rate of what it reads from outside.
//!
//! Two bits instead of TS's ranked `build < frame < copy < item` (src/compile/rates.ts): a Map body that reads its row
//! index and a param is per item for lifting but per frame for classification, and one rank cannot say both.
use loft_ir::graph::{Graph, Op};

#[derive(Clone, Copy, Default, PartialEq, Eq, Debug)]
pub struct Rate {
    pub frame: bool,
    pub item: bool,
}

impl Rate {
    fn join(self, o: Rate) -> Rate {
        Rate { frame: self.frame | o.frame, item: self.item | o.item }
    }
}

/// The rate of every node. `frame_res[r]`: resource `r` changes per frame. `items`: the per-invocation binders
/// (sorted node ids). Operands precede their users and a decl's roots precede the node that names it (the wire's
/// order), so one forward pass is the whole fixed point.
pub fn rates(g: &Graph, frame_res: &[bool], items: &[u32]) -> Vec<Rate> {
    let mut r: Vec<Rate> = Vec::with_capacity(g.nodes.len());
    let frame = Rate { frame: true, item: false };
    for (id, n) in g.nodes.iter().enumerate() {
        let args = g.args_of(n).iter().fold(Rate::default(), |a, &x| a.join(r[x as usize]));
        let over = |xs: &mut dyn Iterator<Item = u32>| xs.fold(args, |a, x| a.join(r[x as usize]));
        r.push(match n.op {
            Op::Lit(_) => Rate::default(),
            Op::Time | Op::Param(_) | Op::Input(_) => frame,
            Op::Bind(_) | Op::Var(_) => Rate { frame: false, item: items.binary_search(&(id as u32)).is_ok() },
            Op::Prim(_) | Op::Swizzle(_) | Op::Vec | Op::Grad => args,
            Op::Fetch { kernel, .. } => args.join(Rate { frame: frame_res[kernel as usize], item: false }),
            Op::Call(f) => over(&mut std::iter::once(g.fns[f as usize].body)),
            Op::Loop { decl, .. } => over(&mut g.loops[decl as usize].nodes()),
            Op::Raw(_) => Rate { frame: true, item: true },
        });
    }
    r
}

/// A lifted value must cost more than the fetch that replaces it (one read and an index: about two operations).
pub const LIFT_MIN: u32 = 3;

/// What evaluating a node costs in operations, as src/compile/rates.ts counts (a move is free; a loop is large).
fn own_cost(op: Op) -> u32 {
    match op {
        Op::Lit(_) | Op::Time | Op::Param(_) | Op::Input(_) | Op::Bind(_) | Op::Var(_) | Op::Swizzle(_) | Op::Vec => 0,
        Op::Prim(_) | Op::Fetch { .. } | Op::Grad => 1,
        Op::Call(_) | Op::Raw(_) => 4,
        Op::Loop { .. } => 64,
    }
}

/// The per-frame frontier of a program invoked per item (TS `plan`, runs = "item"): the frame-rate, item-free number
/// nodes reached from `roots` (not entering decls) whose own sub-graph costs at least `LIFT_MIN`. Each is computed once
/// per frame into a frame-values resource and fetched. Nodes in first-reached order.
pub fn frontier(g: &Graph, rate: &[Rate], roots: &[u32]) -> Vec<u32> {
    let (mut out, mut seen, mut stack) = (Vec::new(), std::collections::HashSet::new(), roots.to_vec());
    stack.reverse();
    while let Some(x) = stack.pop() {
        let (n, r) = (&g.nodes[x as usize], rate[x as usize]);
        if !seen.insert(x) || !(r.frame || r.item) {
            continue;
        }
        if r.frame && !r.item && n.ty.width().is_some() && cost_under(g, rate, x) >= LIFT_MIN {
            out.push(x);
            continue;
        }
        stack.extend(g.args_of(n).iter().rev());
    }
    out
}

/// The cost of the non-Const sub-graph under `x`, counted until it reaches `LIFT_MIN`.
fn cost_under(g: &Graph, rate: &[Rate], x: u32) -> u32 {
    let (mut total, mut seen, mut stack) = (0, Vec::new(), vec![x]);
    while let Some(y) = stack.pop() {
        if total >= LIFT_MIN || seen.contains(&y) || rate[y as usize] == Rate::default() {
            continue;
        }
        seen.push(y);
        total += own_cost(g.nodes[y as usize].op);
        stack.extend(g.args_of(&g.nodes[y as usize]));
    }
    total
}
