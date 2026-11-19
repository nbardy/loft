//! Derived bounds (draft §2D): interval arithmetic with a Lipschitz bound carried beside every value, run through the
//! shared evaluator. Per row, a branch-and-bound over the field's interval extension finds the world AABB of its
//! surface; one more evaluation over that AABB gives L. Nothing is authored: an unbounded result is E211/E212.
//!
//! Precision: intervals are f64 without outward rounding, so they bound the f64 field. The GPU evaluates in f32, so
//! the AABB is padded by `PAD` (absolute) + `PAD_REL` (relative to its size); the truth gate samples both claims.

use crate::eval::{Eval, Scalar, Un, Val};
use loft_ir::ast::{BinOp, FnDecl};
use loft_ir::{Code, Diag, Res, Span};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Iv {
    pub lo: f64,
    pub hi: f64,
    /// Lipschitz bound in the traced point; `INFINITY` is `Unbounded`.
    pub l: f64,
}

/// `m * l` with `0 * inf = 0`: a constant factor of an unbounded term, or an unbounded factor of a constant.
fn ml(m: f64, l: f64) -> f64 {
    if m == 0.0 || l == 0.0 { 0.0 } else { m * l }
}

impl Iv {
    fn new(lo: f64, hi: f64, l: f64) -> Iv {
        if lo.is_nan() || hi.is_nan() { Iv { lo: f64::NEG_INFINITY, hi: f64::INFINITY, l: f64::INFINITY } } else { Iv { lo, hi, l } }
    }
    fn contains_zero(&self) -> bool {
        self.lo <= 0.0 && self.hi >= 0.0
    }
}

impl Scalar for Iv {
    fn c(x: f64) -> Iv {
        Iv { lo: x, hi: x, l: 0.0 }
    }
    fn add(&self, o: &Iv) -> Iv {
        Iv::new(self.lo + o.lo, self.hi + o.hi, self.l + o.l)
    }
    fn sub(&self, o: &Iv) -> Iv {
        Iv::new(self.lo - o.hi, self.hi - o.lo, self.l + o.l)
    }
    fn mul(&self, o: &Iv) -> Iv {
        let p = [self.lo * o.lo, self.lo * o.hi, self.hi * o.lo, self.hi * o.hi];
        let lo = p.iter().copied().fold(f64::INFINITY, f64::min);
        let hi = p.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        Iv::new(lo, hi, ml(self.mag(), o.l) + ml(o.mag(), self.l))
    }
    fn div(&self, o: &Iv) -> Iv {
        if o.contains_zero() {
            return Iv { lo: f64::NEG_INFINITY, hi: f64::INFINITY, l: if self.l == 0.0 && o.l == 0.0 { 0.0 } else { f64::INFINITY } };
        }
        let r = Iv { lo: 1.0 / o.hi, hi: 1.0 / o.lo, l: 0.0 };
        let mut q = self.mul(&r);
        let m = o.mig();
        q.l = (ml(o.mag(), self.l) + ml(self.mag(), o.l)) / (m * m);
        q
    }
    fn neg(&self) -> Iv {
        Iv { lo: -self.hi, hi: -self.lo, l: self.l }
    }
    fn un(&self, f: Un) -> Iv {
        let Iv { lo, hi, l } = *self;
        let jump = |lo_i: f64, hi_i: f64| if lo_i == hi_i || l == 0.0 { 0.0 } else { f64::INFINITY };
        match f {
            Un::Sin => sin_iv(lo, hi, l),
            Un::Cos => sin_iv(lo + std::f64::consts::FRAC_PI_2, hi + std::f64::consts::FRAC_PI_2, l),
            Un::Sqrt => {
                let dl = if l == 0.0 {
                    0.0
                } else if lo > 0.0 {
                    l / (2.0 * lo.sqrt())
                } else {
                    f64::INFINITY
                };
                Iv::new(lo.max(0.0).sqrt(), hi.max(0.0).sqrt(), dl)
            }
            Un::Abs => {
                if lo >= 0.0 {
                    *self
                } else if hi <= 0.0 {
                    self.neg()
                } else {
                    Iv::new(0.0, (-lo).max(hi), l)
                }
            }
            Un::Exp => Iv::new(lo.exp(), hi.exp(), ml(hi.exp(), l)),
            Un::Floor => Iv::new(lo.floor(), hi.floor(), jump(lo.floor(), hi.floor())),
            Un::Fract => {
                if lo.floor() == hi.floor() {
                    Iv::new(lo - lo.floor(), hi - lo.floor(), l)
                } else {
                    Iv::new(0.0, 1.0, if l == 0.0 { 0.0 } else { f64::INFINITY })
                }
            }
            Un::Sign => {
                if lo > 0.0 {
                    Iv::c(1.0)
                } else if hi < 0.0 {
                    Iv::c(-1.0)
                } else {
                    Iv::new(if lo < 0.0 { -1.0 } else { 0.0 }, if hi > 0.0 { 1.0 } else { 0.0 }, jump(lo, hi))
                }
            }
        }
    }
    fn min(&self, o: &Iv) -> Iv {
        Iv::new(self.lo.min(o.lo), self.hi.min(o.hi), self.l.max(o.l))
    }
    fn max(&self, o: &Iv) -> Iv {
        Iv::new(self.lo.max(o.lo), self.hi.max(o.hi), self.l.max(o.l))
    }
    fn pow(&self, o: &Iv) -> Iv {
        match o.exact() {
            Some(k) if k >= 0.0 && k.fract() == 0.0 && k <= 16.0 => {
                let mut r = Iv::c(1.0);
                for _ in 0..k as u32 {
                    r = r.mul(self);
                }
                r
            }
            Some(k) if self.lo > 0.0 => {
                let (a, b) = (self.lo.powf(k), self.hi.powf(k));
                let d = (k * self.lo.powf(k - 1.0)).abs().max((k * self.hi.powf(k - 1.0)).abs());
                Iv::new(a.min(b), a.max(b), ml(d, self.l))
            }
            _ => Iv { lo: f64::NEG_INFINITY, hi: f64::INFINITY, l: f64::INFINITY },
        }
    }
    fn cmp(&self, op: BinOp, o: &Iv) -> Option<bool> {
        let (all, none) = match op {
            BinOp::Lt => (self.hi < o.lo, self.lo >= o.hi),
            BinOp::Le => (self.hi <= o.lo, self.lo > o.hi),
            BinOp::Gt => (self.lo > o.hi, self.hi <= o.lo),
            BinOp::Ge => (self.lo >= o.hi, self.hi < o.lo),
            BinOp::Eq => (self.exact().is_some() && self.exact() == o.exact(), self.hi < o.lo || self.lo > o.hi),
            BinOp::Ne => (self.hi < o.lo || self.lo > o.hi, self.exact().is_some() && self.exact() == o.exact()),
            BinOp::Add | BinOp::Sub | BinOp::Mul | BinOp::Div | BinOp::Rem | BinOp::And | BinOp::Or | BinOp::BitAnd | BinOp::BitOr | BinOp::BitXor | BinOp::Shl | BinOp::Shr => unreachable!("comparison"),
        };
        if all {
            Some(true)
        } else if none {
            Some(false)
        } else {
            None
        }
    }
    fn mag(&self) -> f64 {
        self.lo.abs().max(self.hi.abs())
    }
    fn mig(&self) -> f64 {
        if self.contains_zero() { 0.0 } else { self.lo.abs().min(self.hi.abs()) }
    }
    fn lip(&self) -> f64 {
        self.l
    }
    fn with_lip(self, l: f64) -> Iv {
        Iv { l, ..self }
    }
    fn exact(&self) -> Option<f64> {
        (self.lo == self.hi).then_some(self.lo)
    }
    fn join(&self, o: &Iv) -> Iv {
        Iv::new(self.lo.min(o.lo), self.hi.max(o.hi), self.l.max(o.l))
    }
}

/// sin over [lo, hi]: the endpoints, plus ±1 where a crest or trough lies inside. |sin'| <= 1 keeps L.
fn sin_iv(lo: f64, hi: f64, l: f64) -> Iv {
    use std::f64::consts::{FRAC_PI_2, TAU};
    if !(hi - lo).is_finite() || hi - lo >= TAU {
        return Iv::new(-1.0, 1.0, l);
    }
    let (a, b) = (lo.sin(), hi.sin());
    let (mut mn, mut mx) = (a.min(b), a.max(b));
    // Crest at pi/2 + 2k pi, trough at -pi/2 + 2k pi.
    let has = |phase: f64| ((lo - phase) / TAU).ceil() <= ((hi - phase) / TAU).floor();
    if has(FRAC_PI_2) {
        mx = 1.0;
    }
    if has(-FRAC_PI_2) {
        mn = -1.0;
    }
    Iv::new(mn, mx, l)
}

/// The search region for a surface: a cube of half-size `REACH` m about the world origin.
pub const REACH: f64 = 1024.0;
/// Bisect until a cell is at most this many metres.
pub const CELL: f64 = 1.0 / 16.0;
/// The breadth-first phase stops when its cells are 1/RESOLVE of the surface's extent.
pub const RESOLVE: f64 = 4.0;
pub const PAD: f64 = 1e-3;
pub const PAD_REL: f64 = 1e-4;

/// One row's derived world AABB and Lipschitz bound, and how many field evaluations it took.
pub struct Derived {
    pub lo: [f64; 3],
    pub hi: [f64; 3],
    pub l: f64,
    pub evals: u32,
}

type Cell = ([f64; 3], [f64; 3]);

/// Derives one row's bounds and L for a kind's `trace` query.
pub fn derive<'a>(ev: &mut Eval<'_, 'a, Iv>, trace: &'a FnDecl, kind: usize, row: usize, span: Span) -> Res<Derived> {
    let evals = std::cell::Cell::new(0u32);
    let once = |ev: &mut Eval<'_, 'a, Iv>, (lo, hi): Cell| -> Res<Iv> {
        evals.set(evals.get() + 1);
        let p = Val::V((0..3).map(|i| Iv { lo: lo[i], hi: hi[i], l: 1.0 }).collect(), 1.0);
        match ev.call_fn(trace, vec![Val::Row(kind, row), p], span)? {
            Val::S(s) => Ok(s),
            other @ Val::Void | other @ Val::V(..) | other @ Val::B(_) | other @ Val::U(_) | other @ Val::I(_) | other @ Val::X(_) | other @ Val::Row(..) | other @ Val::MapRef(..) => unreachable!("checked: trace returns f32, got {other:?}"),
        }
    };
    // The natural interval, tightened by the centred (mean value) form f(c) ± L·r with the cell's own derived L.
    // Both enclose f over the cell, so their intersection does; for a distance field it is near exact, where the
    // natural interval alone loses to the dependency problem (`pa` appears twice in `sd_capsule`).
    let field = |ev: &mut Eval<'_, 'a, Iv>, c: Cell| -> Res<Iv> {
        let iv = once(ev, c)?;
        // Decided already (or no finite L to centre with): skip the second evaluation.
        if iv.lo > 0.0 || iv.hi <= 0.0 || !iv.l.is_finite() {
            return Ok(iv);
        }
        let mid = [0, 1, 2].map(|i| 0.5 * (c.0[i] + c.1[i]));
        let at = once(ev, (mid, mid))?;
        let r = 0.5 * (0..3).map(|i| (c.1[i] - c.0[i]).powi(2)).sum::<f64>().sqrt();
        Ok(Iv { lo: iv.lo.max(at.lo - iv.l * r), hi: iv.hi.min(at.hi + iv.l * r), l: iv.l })
    };
    let split = |(lo, hi): Cell| -> Vec<Cell> {
        let mid = [0, 1, 2].map(|i| 0.5 * (lo[i] + hi[i]));
        (0..8)
            .map(|k| {
                let pick = |i: usize| if k >> i & 1 == 0 { (lo[i], mid[i]) } else { (mid[i], hi[i]) };
                let (a, b, c) = (pick(0), pick(1), pick(2));
                ([a.0, b.0, c.0], [a.1, b.1, c.1])
            })
            .collect()
    };
    let size = |(lo, hi): &Cell| (0..3).map(|i| hi[i] - lo[i]).fold(0.0, f64::max);
    let none = || Diag::new(
        Code::E212,
        span,
        format!("row {row}: the field has no surface within ±{REACH} m of the origin"),
        "check the row's values; a field that is positive everywhere draws nothing",
    );
    // Breadth first until the surface is resolved by ~RESOLVE cells across: those cells seed all six searches.
    // (A single-cell descent stalls when a row straddles a split plane of the search cube: every grid scene does.)
    let mut alive: Vec<Cell> = vec![([-REACH; 3], [REACH; 3])];
    loop {
        let (mut lo, mut hi) = ([f64::INFINITY; 3], [f64::NEG_INFINITY; 3]);
        for c in &alive {
            for i in 0..3 {
                lo[i] = lo[i].min(c.0[i]);
                hi[i] = hi[i].max(c.1[i]);
            }
        }
        let extent = (0..3).map(|i| hi[i] - lo[i]).fold(0.0, f64::max);
        let cell = alive.first().map_or(0.0, size);
        if alive.is_empty() || cell <= CELL || cell * RESOLVE <= extent {
            break;
        }
        let mut next = vec![];
        for c in alive.iter().flat_map(|c| split(*c)) {
            if field(ev, c)?.lo <= 0.0 {
                next.push(c);
            }
        }
        alive = next;
    }
    if alive.is_empty() {
        return Err(none());
    }
    let mut lo = [0.0; 3];
    let mut hi = [0.0; 3];
    for axis in 0..3 {
        for sign in [1.0f64, -1.0] {
            let mut best = f64::NEG_INFINITY;
            let mut stack = alive.clone();
            stack.sort_by(|a, b| {
                let k = |x: &Cell| if sign > 0.0 { x.1[axis] } else { -x.0[axis] };
                k(a).total_cmp(&k(b))
            });
            while let Some(c) = stack.pop() {
                let key = if sign > 0.0 { c.1[axis] } else { -c.0[axis] };
                if key <= best {
                    continue;
                }
                let iv = field(ev, c)?;
                if iv.lo > 0.0 {
                    continue;
                }
                if iv.hi <= 0.0 || size(&c) <= CELL {
                    best = key;
                    continue;
                }
                let mut kids = split(c);
                kids.sort_by(|a, b| {
                    let k = |x: &Cell| if sign > 0.0 { x.1[axis] } else { -x.0[axis] };
                    k(a).total_cmp(&k(b))
                });
                stack.extend(kids);
            }
            if best == f64::NEG_INFINITY {
                return Err(none());
            }
            if best.abs() >= REACH {
                return Err(Diag::new(
                    Code::E212,
                    span,
                    format!("row {row}: the surface reaches the search limit ±{REACH} m, so it has no finite bound"),
                    "bound the shape (intersect with `sd_box`), or keep it within the search region",
                ));
            }
            if sign > 0.0 {
                hi[axis] = best;
            } else {
                lo[axis] = -best;
            }
        }
    }
    for i in 0..3 {
        let pad = PAD + PAD_REL * (hi[i] - lo[i]);
        lo[i] -= pad;
        hi[i] += pad;
    }
    ev.cause = None;
    let l = field(ev, (lo, hi))?.l;
    if !l.is_finite() {
        let (at, why) = ev.cause.clone().unwrap_or((span, "unbounded".into()));
        return Err(Diag::new(
            Code::E211,
            span,
            format!("sphere tracer requires a proven Lipschitz bound; row {row} derives `Unbounded`"),
            "clamp the denominator away from zero, keep branches out of the field, or move the term to `material`",
        )
        .note(at, why));
    }
    Ok(Derived { lo, hi, l: l.max(1e-6), evals: evals.get() })
}
