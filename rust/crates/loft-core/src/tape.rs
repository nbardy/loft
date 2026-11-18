//! The f32 twin (plan §3.4): a straight-line program of atoms over scalar registers, run for many points ("lanes")
//! at once. Vectors are split into components while it is built; every operator lowers to atoms through its
//! `PRIMS` row, so the tape, intervals and derivatives see one small instruction set.
//!
//! Inherited from feat/rust-core's `tape.rs`: one register per value, lanes contiguous per register, componentwise
//! broadcast of scalars. Changed: f32 with `libm` instead of f64 with JSC's bits, and no folding at build (folding
//! is its own pass on the arena, §1.10).

use crate::prim::{self, Atom, Means, Prim};

/// A register: the index of the instruction that writes it.
pub type Id = u32;

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Ins {
    /// An authored number, rounded once to f32.
    Lit(f32),
    /// Leaf slot (time, a param, an input component, a binder).
    Leaf(u32),
    /// Operands past the atom's arity are ignored.
    Op(Atom, [Id; 3]),
}

#[derive(Clone, Default, Debug)]
pub struct Ssa { pub ins: Vec<Ins> }

/// A lane's leaf vector is shorter than a leaf slot the program reads.
#[derive(Debug, PartialEq)]
pub struct Unbound { pub slot: u32, pub given: usize }

impl Ssa {
    fn push(&mut self, i: Ins) -> Id { self.ins.push(i); (self.ins.len() - 1) as Id }
    pub fn lit(&mut self, v: f32) -> Id { self.push(Ins::Lit(v)) }
    pub fn leaf(&mut self, slot: u32) -> Id { self.push(Ins::Leaf(slot)) }
    pub fn op(&mut self, a: Atom, x: [Id; 3]) -> Id { self.push(Ins::Op(a, x)) }

    /// One authored operator over component lists (a scalar is a list of one and broadcasts).
    pub fn apply(&mut self, p: Prim, args: &[&[Id]]) -> Vec<Id> {
        let each = |s: &mut Ssa, f: &dyn Fn(&mut Ssa, [Id; 3]) -> Id| -> Vec<Id> {
            let w = args.iter().map(|a| a.len()).max().unwrap_or(1);
            (0..w).map(|i| {
                let mut x = [0; 3];
                for (k, a) in args.iter().enumerate() { x[k] = if a.len() == 1 { a[0] } else { a[i] }; }
                f(s, x)
            }).collect()
        };
        match prim::def(p).means {
            Means::Atom(a) => each(self, &|s, x| s.op(a, x)),
            Means::Each(f) => each(self, &f),
            Means::Whole(f) => f(self, args),
        }
    }

    /// The largest leaf slot read, plus one.
    pub fn slots(&self) -> usize {
        self.ins.iter().map(|i| match *i { Ins::Leaf(s) => s as usize + 1, Ins::Lit(_) | Ins::Op(..) => 0 }).max().unwrap_or(0)
    }
}

/// Runs every instruction for `lanes` points. Lane l reads leaf slot s at `leaves[l * slots + s]`; register r of
/// lane l is left at `regs[r * lanes + l]`.
pub fn run(ssa: &Ssa, lanes: usize, slots: usize, leaves: &[f32], regs: &mut Vec<f32>) -> Result<(), Unbound> {
    let need = ssa.slots();
    if need > slots || leaves.len() < lanes * slots {
        return Err(Unbound { slot: need.saturating_sub(1) as u32, given: slots });
    }
    regs.clear();
    regs.resize(ssa.ins.len() * lanes, 0.0);
    for (r, i) in ssa.ins.iter().enumerate() {
        let (done, rest) = regs.split_at_mut(r * lanes);
        let out = &mut rest[..lanes];
        match *i {
            Ins::Lit(v) => out.fill(v),
            Ins::Leaf(s) => for (l, o) in out.iter_mut().enumerate() { *o = leaves[l * slots + s as usize] },
            Ins::Op(a, [x, y, z]) => {
                let f = prim::atom(a).f;
                // Operands are earlier instructions (decode validates the order; a later one is an index panic, i.e. Internal).
                let reg = |k: Id| &done[k as usize * lanes..k as usize * lanes + lanes];
                let (x, y, z) = (reg(x), reg(y), reg(z));
                for l in 0..lanes { out[l] = f(x[l], y[l], z[l]); }
            }
        }
    }
    Ok(())
}
