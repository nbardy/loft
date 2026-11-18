//! Interval enclosures of the f32 twin (plan §3.4, `next.md` §6.5 "f32-padded enclosures").
//!
//! Each atom's rule encloses the exact operation over its operand intervals in f64. The fold then widens that
//! outward by the atom's f32 rounding bound (`AtomDef::ulps`), so by induction every interval contains the value
//! the tape computes at every f32 point of the leaf box. DERIVATION: for |x| > 0, |x| 2^-23 is at least one f32 ulp
//! of anything in [x, 2x], and the pad is twice that per ulp of bound; the f64 rounding of the bound itself
//! (2^-53 relative, or exact at 0) is far inside it. The minimum pad 2^-149 is the least f32 subnormal.
//!
//! Plain intervals only. The affine forms of `src/interval.ts` (correlation through long chains) are not here yet.

use crate::prim;
use crate::tape::{Id, Ins, Ssa};
use core::f64::consts::{FRAC_PI_2, PI, TAU};
use libm as m;

/// [lo, hi], lo <= hi.
pub type I = [f64; 2];

/// Why an atom has no finite enclosure over this box. A smaller box may have one (the solver splits).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Why {
    DivisorHasZero,
    /// The operand leaves the function's domain (sqrt, log, asin, acos, pow's base).
    OutsideDomain,
    /// tan's operand spans a pole.
    Pole,
    /// The enclosure leaves f32's finite range.
    Overflow,
    /// The program reads a leaf slot the box does not bind.
    Unbound,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Unbounded { pub at: Id, pub why: Why }

/// Encloses every register of the program over the box `leaves` (one interval per leaf slot).
pub fn enclose(s: &Ssa, leaves: &[I]) -> Result<Vec<I>, Unbounded> {
    let mut v: Vec<I> = Vec::with_capacity(s.ins.len());
    for (r, i) in s.ins.iter().enumerate() {
        let fail = |why| Unbounded { at: r as Id, why };
        let x = match *i {
            Ins::Lit(c) => [c as f64; 2],
            Ins::Leaf(k) => *leaves.get(k as usize).ok_or(fail(Why::Unbound))?,
            Ins::Op(a, [p, q, t]) => {
                let d = prim::atom(a);
                let x = (d.iv)([v[p as usize], v[q as usize], v[t as usize]]).map_err(fail)?;
                pad(x, d.ulps)
            }
        };
        if !(x[0] >= -(f32::MAX as f64) && x[1] <= f32::MAX as f64) { return Err(fail(Why::Overflow)); }
        v.push(x);
    }
    Ok(v)
}

fn pad(x: I, ulps: u8) -> I {
    let k = ulps as f64 * 2.0;
    let e = |b: f64| k * (b.abs() * f64::powi(2.0, -23)).max(f64::powi(2.0, -149));
    if ulps == 0 { x } else { [x[0] - e(x[0]), x[1] + e(x[1])] }
}

type R = Result<I, Why>;
fn hull4(p: [f64; 4]) -> I { [p.iter().copied().fold(f64::INFINITY, f64::min), p.iter().copied().fold(f64::NEG_INFINITY, f64::max)] }
fn up(a: I, f: fn(f64) -> f64) -> I { [f(a[0]), f(a[1])] }
fn hull(a: I, b: I) -> I { [a[0].min(b[0]), a[1].max(b[1])] }
const BOOL: I = [0.0, 1.0];
const T1: I = [1.0, 1.0];
const F0: I = [0.0, 0.0];
/// Truthy at every point (bool registers are 0 or 1; a number is truthy when nonzero).
fn always(a: I) -> bool { a[0] > 0.0 || a[1] < 0.0 }
fn never(a: I) -> bool { a[0] == 0.0 && a[1] == 0.0 }
/// Whether [lo, hi] meets c + 2k pi for some k, decided with slack (a near miss counts as a hit: only wider).
fn meets(a: I, c: f64, period: f64) -> bool {
    let k = ((a[0] - c) / period).ceil();
    c + k * period <= a[1] + 1e-9 * (1.0 + a[0].abs() + a[1].abs())
}

pub fn add([a, b, _]: [I; 3]) -> R { Ok([a[0] + b[0], a[1] + b[1]]) }
pub fn sub([a, b, _]: [I; 3]) -> R { Ok([a[0] - b[1], a[1] - b[0]]) }
pub fn mul([a, b, _]: [I; 3]) -> R { Ok(hull4([a[0] * b[0], a[0] * b[1], a[1] * b[0], a[1] * b[1]])) }
/// a / b for b clear of 0.
fn quot(a: I, b: I) -> I { hull4([a[0] / b[0], a[0] / b[1], a[1] / b[0], a[1] / b[1]]) }
pub fn div([a, b, _]: [I; 3]) -> R { if b[0] <= 0.0 && b[1] >= 0.0 { Err(Why::DivisorHasZero) } else { Ok(quot(a, b)) } }
pub fn neg([a, _, _]: [I; 3]) -> R { Ok([-a[1], -a[0]]) }
pub fn min([a, b, _]: [I; 3]) -> R { Ok([a[0].min(b[0]), a[1].min(b[1])]) }
pub fn max([a, b, _]: [I; 3]) -> R { Ok([a[0].max(b[0]), a[1].max(b[1])]) }
/// a^b = exp(b ln a) for a > 0: bilinear in (b, ln a), so extremes are at corners.
pub fn pow([a, b, _]: [I; 3]) -> R {
    if a[0] <= 0.0 { return Err(Why::OutsideDomain); }
    let (l0, l1) = (m::log(a[0]), m::log(a[1]));
    Ok(up(hull4([b[0] * l0, b[0] * l1, b[1] * l0, b[1] * l1]), m::exp))
}
/// Off the branch cut, atan2 is atan of a quotient (whose divisor is clear of 0) plus a quadrant constant.
pub fn atan2([y, x, _]: [I; 3]) -> R {
    let turn = |c: f64, t: I| [c - t[1], c - t[0]];
    Ok(if x[0] > 0.0 { up(quot(y, x), m::atan) }
       else if y[0] > 0.0 { turn(FRAC_PI_2, up(quot(x, y), m::atan)) }
       else if y[1] < 0.0 { turn(-FRAC_PI_2, up(quot(x, y), m::atan)) }
       else { [-PI, PI] })
}
/// Truncated remainder: the sign of a, smaller in size than both |a| and |b|.
pub fn rem([a, b, _]: [I; 3]) -> R {
    if b[0] <= 0.0 && b[1] >= 0.0 { return Err(Why::DivisorHasZero); }
    let w = b[0].abs().max(b[1].abs());
    Ok([a[0].max(-w).min(0.0), a[1].min(w).max(0.0)])
}
pub fn abs([a, _, _]: [I; 3]) -> R {
    Ok(if a[0] >= 0.0 { a } else if a[1] <= 0.0 { [-a[1], -a[0]] } else { [0.0, a[1].max(-a[0])] })
}
pub fn sign([a, _, _]: [I; 3]) -> R { Ok(up(a, |x| if x > 0.0 { 1.0 } else if x < 0.0 { -1.0 } else { 0.0 })) }
pub fn floor([a, _, _]: [I; 3]) -> R { Ok(up(a, m::floor)) }
pub fn ceil([a, _, _]: [I; 3]) -> R { Ok(up(a, m::ceil)) }
pub fn trunc([a, _, _]: [I; 3]) -> R { Ok(up(a, m::trunc)) }
pub fn round_even([a, _, _]: [I; 3]) -> R { Ok(up(a, m::rint)) }
pub fn sqrt([a, _, _]: [I; 3]) -> R { if a[0] < 0.0 { Err(Why::OutsideDomain) } else { Ok(up(a, m::sqrt)) } }
pub fn exp([a, _, _]: [I; 3]) -> R { Ok(up(a, m::exp)) }
pub fn exp2([a, _, _]: [I; 3]) -> R { Ok(up(a, m::exp2)) }
pub fn log([a, _, _]: [I; 3]) -> R { if a[0] <= 0.0 { Err(Why::OutsideDomain) } else { Ok(up(a, m::log)) } }
pub fn log2([a, _, _]: [I; 3]) -> R { if a[0] <= 0.0 { Err(Why::OutsideDomain) } else { Ok(up(a, m::log2)) } }
fn wave(a: I, f: fn(f64) -> f64, top: f64, bottom: f64) -> I {
    if a[1] - a[0] >= TAU { return [-1.0, 1.0]; }
    let (p, q) = (f(a[0]), f(a[1]));
    [if meets(a, bottom, TAU) { -1.0 } else { p.min(q) }, if meets(a, top, TAU) { 1.0 } else { p.max(q) }]
}
pub fn sin([a, _, _]: [I; 3]) -> R { Ok(wave(a, m::sin, FRAC_PI_2, -FRAC_PI_2)) }
pub fn cos([a, _, _]: [I; 3]) -> R { Ok(wave(a, m::cos, 0.0, PI)) }
pub fn tan([a, _, _]: [I; 3]) -> R {
    if a[1] - a[0] >= PI || meets(a, FRAC_PI_2, PI) { Err(Why::Pole) } else { Ok(up(a, m::tan)) }
}
pub fn asin([a, _, _]: [I; 3]) -> R { if a[0] < -1.0 || a[1] > 1.0 { Err(Why::OutsideDomain) } else { Ok(up(a, m::asin)) } }
pub fn acos([a, _, _]: [I; 3]) -> R {
    if a[0] < -1.0 || a[1] > 1.0 { Err(Why::OutsideDomain) } else { Ok([m::acos(a[1]), m::acos(a[0])]) }
}
pub fn atan([a, _, _]: [I; 3]) -> R { Ok(up(a, m::atan)) }
pub fn sinh([a, _, _]: [I; 3]) -> R { Ok(up(a, m::sinh)) }
pub fn cosh([a, _, _]: [I; 3]) -> R {
    let (p, q) = (m::cosh(a[0]), m::cosh(a[1]));
    Ok([if a[0] <= 0.0 && a[1] >= 0.0 { 1.0 } else { p.min(q) }, p.max(q)])
}
pub fn tanh([a, _, _]: [I; 3]) -> R { Ok(up(a, m::tanh)) }
fn decide(yes: bool, no: bool) -> I { if yes { T1 } else if no { F0 } else { BOOL } }
pub fn lt([a, b, _]: [I; 3]) -> R { Ok(decide(a[1] < b[0], a[0] >= b[1])) }
pub fn gt([a, b, _]: [I; 3]) -> R { Ok(decide(a[0] > b[1], a[1] <= b[0])) }
pub fn le([a, b, _]: [I; 3]) -> R { Ok(decide(a[1] <= b[0], a[0] > b[1])) }
pub fn ge([a, b, _]: [I; 3]) -> R { Ok(decide(a[0] >= b[1], a[1] < b[0])) }
fn same(a: I, b: I) -> bool { a[0] == a[1] && b[0] == b[1] && a[0] == b[0] }
fn apart(a: I, b: I) -> bool { a[1] < b[0] || b[1] < a[0] }
pub fn eq([a, b, _]: [I; 3]) -> R { Ok(decide(same(a, b), apart(a, b))) }
pub fn ne([a, b, _]: [I; 3]) -> R { Ok(decide(apart(a, b), same(a, b))) }
/// `c ? y : n`, with `and`/`or`/`not` as its special cases on 0/1 values.
fn pick(c: I, y: I, n: I) -> I { if always(c) { y } else if never(c) { n } else { hull(y, n) } }
pub fn and([a, b, _]: [I; 3]) -> R { Ok(pick(a, b, a)) }
pub fn or([a, b, _]: [I; 3]) -> R { Ok(pick(a, a, b)) }
pub fn not([a, _, _]: [I; 3]) -> R { Ok(pick(a, F0, T1)) }
pub fn select([c, y, n]: [I; 3]) -> R { Ok(pick(c, y, n)) }
