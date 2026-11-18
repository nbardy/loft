//! Derivatives (plan §1.3: one rule per atom, read from `ATOMS`). Forward mode over the straight-line program: each
//! instruction's derivative with respect to one leaf is appended as new instructions, so the result is itself a
//! program that the tape runs, intervals enclose and `grad` differentiates again (Hessians).
//!
//! A structural zero is `None` and is never written: a derivative through floors, comparisons and constants costs
//! nothing, which keeps `d/dt` of a pose the size of the pose's smooth part.

use crate::prim::{self, Atom};
use crate::tape::{Id, Ins, Ssa};

/// The derivative builder the rules write through: arithmetic on optional (structurally zero) operands.
pub struct G<'a> { s: &'a mut Ssa }

impl G<'_> {
    pub fn op(&mut self, a: Atom, x: [Id; 3]) -> Id { self.s.op(a, x) }
    pub fn lit(&mut self, v: f32) -> Id { self.s.lit(v) }
    pub fn add(&mut self, a: Option<Id>, b: Option<Id>) -> Option<Id> {
        match (a, b) { (Some(a), Some(b)) => Some(self.op(Atom::Add, [a, b, 0])), (x, None) | (None, x) => x }
    }
    pub fn sub(&mut self, a: Option<Id>, b: Option<Id>) -> Option<Id> {
        match (a, b) { (Some(a), Some(b)) => Some(self.op(Atom::Sub, [a, b, 0])), (a, None) => a, (None, b) => self.neg(b) }
    }
    pub fn neg(&mut self, a: Option<Id>) -> Option<Id> { a.map(|a| self.op(Atom::Neg, [a, 0, 0])) }
    /// k * a for a value k.
    pub fn mul(&mut self, k: Id, a: Option<Id>) -> Option<Id> { a.map(|a| self.op(Atom::Mul, [k, a, 0])) }
    /// a / k for a value k.
    pub fn div(&mut self, a: Option<Id>, k: Id) -> Option<Id> { a.map(|a| self.op(Atom::Div, [a, k, 0])) }
    pub fn select(&mut self, c: Id, y: Option<Id>, n: Option<Id>) -> Option<Id> {
        let (y, n) = match (y, n) {
            (None, None) => return None,
            (Some(y), Some(n)) => (y, n),
            (Some(y), None) => (y, self.lit(0.0)),
            (None, Some(n)) => (self.lit(0.0), n),
        };
        Some(self.op(Atom::Select, [c, y, n]))
    }
}

/// Appends d(root)/d(leaf `wrt`) for each root and returns their registers (a structural zero becomes a literal 0).
pub fn grad(s: &mut Ssa, roots: &[Id], wrt: u32) -> Vec<Id> {
    let end = roots.iter().map(|&r| r as usize + 1).max().unwrap_or(0);
    let mut d: Vec<Option<Id>> = Vec::with_capacity(end);
    for r in 0..end {
        let di = match s.ins[r] {
            Ins::Lit(_) => None,
            Ins::Leaf(k) if k == wrt => Some(s.lit(1.0)),
            Ins::Leaf(_) => None,
            Ins::Op(a, x) => {
                let dx = [d[x[0] as usize], d[x[1] as usize], d[x[2] as usize]];
                let def = prim::atom(a);
                let live = dx[..def.arity as usize].iter().any(Option::is_some);
                if live { (def.d)(&mut G { s }, x, dx, r as Id) } else { None }
            }
        };
        d.push(di);
    }
    roots.iter().map(|&r| d[r as usize].unwrap_or_else(|| s.lit(0.0))).collect()
}
