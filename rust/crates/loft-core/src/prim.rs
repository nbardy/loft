//! The operator table (lean Rust plan §1.3): one row per operator, read by four folds. `emit` reads the WGSL
//! spelling and type rule; the tape (f32 twin), intervals and derivatives read the meaning.
//!
//! An operator's meaning is an `Atom` (one f32 operation with an evaluation, an f32 rounding bound, an interval rule
//! and a derivative rule) or an expansion into atoms. The expansion is the twin's definition of the builtin, as
//! `compileJs` wrote it (`mix` is `a + (b - a) * t`), so the tape, intervals and derivatives of `mix` are those of its
//! atoms and need no rule of their own. The GPU still runs the builtin; the difference is f32 reordering (§2.2a).
//! Neither table has optional fields: an operator without a rule does not compile.

use crate::grad::G;
use crate::interval::{self as iv, Why, I};
use crate::tape::{Id, Ssa};
use libm as m;

/// The authored operators, in the order of `src/expr.ts` `Fn` (the recorder writes this index).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
#[repr(u8)]
pub enum Prim {
    Add, Sub, Mul, Div, Neg, Min, Max, Pow, Mod, Atan2, Step,
    Abs, Sign, Floor, Fract, Sqrt, Exp, Log, Sin, Cos, Tan, Saturate, Normalize,
    Mix, Clamp, Smoothstep, Length, Distance, Dot, Cross,
    Lt, Gt, Le, Ge, Eq, Ne, And, Or, Not, Select,
    Ceil, Trunc, RoundEven, Rem, InverseSqrt, Exp2, Log2, Asin, Acos, Atan, Sinh, Cosh, Tanh,
}

/// The single f32 operations every operator reduces to. Booleans are 0 and 1.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
#[repr(u8)]
pub enum Atom {
    Add, Sub, Mul, Div, Neg, Min, Max, Pow, Atan2, Rem,
    Abs, Sign, Floor, Ceil, Trunc, RoundEven,
    Sqrt, Exp, Exp2, Log, Log2, Sin, Cos, Tan, Asin, Acos, Atan, Sinh, Cosh, Tanh,
    Lt, Gt, Le, Ge, Eq, Ne, And, Or, Not, Select,
}

/// How `emit` writes an operator. `Mod` and `Select` have WGSL forms of their own.
pub enum Spell { Infix(&'static str), Prefix(&'static str), Call(&'static str), Mod, Select }

/// The result type from the operand types. Vector widths broadcast scalars.
pub enum TyRule { Widest, Compare, Logic, Pick, Scalar, Vec3, Same }

pub enum Means {
    Atom(Atom),
    /// Component by component, scalars broadcast, defined by atoms.
    Each(fn(&mut Ssa, [Id; 3]) -> Id),
    /// Over whole vectors (`dot`, `cross`, ...).
    Whole(fn(&mut Ssa, &[&[Id]]) -> Vec<Id>),
}

pub struct PrimDef { pub wgsl: Spell, pub ty: TyRule, pub arity: u8, pub splat: bool, pub means: Means }

/// Derivative rule: operands, their derivatives (None: structurally zero) and the result -> the result's derivative.
pub type Deriv = fn(&mut G, [Id; 3], [Option<Id>; 3], Id) -> Option<Id>;

pub struct AtomDef {
    pub arity: u8,
    /// The f32 twin: `libm`'s pure-Rust f32 functions, so the bits are the same on every OS and on wasm.
    pub f: fn(f32, f32, f32) -> f32,
    /// Bound on |f - exact| in f32 ulps of the result (0: exact). The interval fold pads by it.
    pub ulps: u8,
    /// A real enclosure of the exact operation over operand intervals (padding is the fold's job).
    pub iv: fn([I; 3]) -> Result<I, Why>,
    pub d: Deriv,
}

pub fn def(p: Prim) -> &'static PrimDef { &PRIMS[p as usize] }
pub fn atom(a: Atom) -> &'static AtomDef { &ATOMS[a as usize] }

const fn row(wgsl: Spell, ty: TyRule, arity: u8, splat: bool, means: Means) -> PrimDef { PrimDef { wgsl, ty, arity, splat, means } }
const fn at(arity: u8, f: fn(f32, f32, f32) -> f32, ulps: u8, iv: fn([I; 3]) -> Result<I, Why>, d: Deriv) -> AtomDef {
    AtomDef { arity, f, ulps, iv, d }
}
fn bit(x: bool) -> f32 { if x { 1.0 } else { 0.0 } }
fn truthy(x: f32) -> bool { x != 0.0 && !x.is_nan() }

use Means::{Atom as A, Each, Whole};
use Spell::{Call, Infix, Prefix};
use TyRule::*;

pub static PRIMS: [PrimDef; 53] = [
    row(Infix("+"), Widest, 2, false, A(Atom::Add)),
    row(Infix("-"), Widest, 2, false, A(Atom::Sub)),
    row(Infix("*"), Widest, 2, false, A(Atom::Mul)),
    row(Infix("/"), Widest, 2, false, A(Atom::Div)),
    row(Prefix("-"), Widest, 1, false, A(Atom::Neg)),
    row(Call("min"), Widest, 2, true, A(Atom::Min)),
    row(Call("max"), Widest, 2, true, A(Atom::Max)),
    row(Call("pow"), Widest, 2, true, A(Atom::Pow)),
    // Floored: a - b floor(a / b) (WGSL `%` is `rem`).
    row(Spell::Mod, Widest, 2, false, Each(|s, [a, b, _]| {
        let q = s.op(Atom::Div, [a, b, 0]);
        let f = s.op(Atom::Floor, [q, 0, 0]);
        let p = s.op(Atom::Mul, [b, f, 0]);
        s.op(Atom::Sub, [a, p, 0])
    })),
    row(Call("atan2"), Widest, 2, true, A(Atom::Atan2)),
    // step(edge, x) = x >= edge.
    row(Call("step"), Widest, 2, true, Each(|s, [e, x, _]| s.op(Atom::Ge, [x, e, 0]))),
    row(Call("abs"), Widest, 1, false, A(Atom::Abs)),
    row(Call("sign"), Widest, 1, false, A(Atom::Sign)),
    row(Call("floor"), Widest, 1, false, A(Atom::Floor)),
    row(Call("fract"), Widest, 1, false, Each(|s, [a, _, _]| { let f = s.op(Atom::Floor, [a, 0, 0]); s.op(Atom::Sub, [a, f, 0]) })),
    row(Call("sqrt"), Widest, 1, false, A(Atom::Sqrt)),
    row(Call("exp"), Widest, 1, false, A(Atom::Exp)),
    row(Call("log"), Widest, 1, false, A(Atom::Log)),
    row(Call("sin"), Widest, 1, false, A(Atom::Sin)),
    row(Call("cos"), Widest, 1, false, A(Atom::Cos)),
    row(Call("tan"), Widest, 1, false, A(Atom::Tan)),
    row(Call("saturate"), Widest, 1, false, Each(|s, [a, _, _]| {
        let (z, o) = (s.lit(0.0), s.lit(1.0));
        let lo = s.op(Atom::Max, [z, a, 0]);
        s.op(Atom::Min, [o, lo, 0])
    })),
    // A scalar's direction is its sign.
    row(Call("normalize"), Same, 1, false, Whole(|s, a| match a[0] {
        [x] => vec![s.op(Atom::Sign, [*x, 0, 0])],
        v => { let d = dot(s, v, v); let l = s.op(Atom::Sqrt, [d, 0, 0]); v.iter().map(|&c| s.op(Atom::Div, [c, l, 0])).collect() }
    })),
    row(Call("mix"), Widest, 3, true, Each(|s, [a, b, t]| {
        let d = s.op(Atom::Sub, [b, a, 0]);
        let p = s.op(Atom::Mul, [d, t, 0]);
        s.op(Atom::Add, [a, p, 0])
    })),
    row(Call("clamp"), Widest, 3, true, Each(|s, [x, lo, hi]| { let m = s.op(Atom::Max, [lo, x, 0]); s.op(Atom::Min, [hi, m, 0]) })),
    row(Call("smoothstep"), Widest, 3, true, Each(|s, [lo, hi, x]| {
        let (n, w) = (s.op(Atom::Sub, [x, lo, 0]), s.op(Atom::Sub, [hi, lo, 0]));
        let q = s.op(Atom::Div, [n, w, 0]);
        let (z, o, two, three) = (s.lit(0.0), s.lit(1.0), s.lit(2.0), s.lit(3.0));
        let q = s.op(Atom::Max, [z, q, 0]);
        let t = s.op(Atom::Min, [o, q, 0]);
        let tt = s.op(Atom::Mul, [t, t, 0]);
        let t2 = s.op(Atom::Mul, [two, t, 0]);
        let r = s.op(Atom::Sub, [three, t2, 0]);
        s.op(Atom::Mul, [tt, r, 0])
    })),
    row(Call("length"), Scalar, 1, false, Whole(|s, a| { let d = dot(s, a[0], a[0]); vec![s.op(Atom::Sqrt, [d, 0, 0])] })),
    row(Call("distance"), Scalar, 2, false, Whole(|s, a| {
        let d = each(s, Atom::Sub, a[0], a[1]);
        let q = dot(s, &d, &d);
        vec![s.op(Atom::Sqrt, [q, 0, 0])]
    })),
    row(Call("dot"), Scalar, 2, false, Whole(|s, a| vec![dot(s, a[0], a[1])])),
    row(Call("cross"), Vec3, 2, false, Whole(|s, a| {
        let (x, y) = (a[0], a[1]);
        [(1, 2), (2, 0), (0, 1)].iter().map(|&(i, j)| {
            let p = s.op(Atom::Mul, [x[i], y[j], 0]);
            let q = s.op(Atom::Mul, [x[j], y[i], 0]);
            s.op(Atom::Sub, [p, q, 0])
        }).collect()
    })),
    row(Infix("<"), Compare, 2, false, A(Atom::Lt)),
    row(Infix(">"), Compare, 2, false, A(Atom::Gt)),
    row(Infix("<="), Compare, 2, false, A(Atom::Le)),
    row(Infix(">="), Compare, 2, false, A(Atom::Ge)),
    row(Infix("=="), Compare, 2, false, A(Atom::Eq)),
    row(Infix("!="), Compare, 2, false, A(Atom::Ne)),
    row(Infix("&&"), Logic, 2, false, A(Atom::And)),
    row(Infix("||"), Logic, 2, false, A(Atom::Or)),
    row(Prefix("!"), Logic, 1, false, A(Atom::Not)),
    row(Spell::Select, Pick, 3, true, A(Atom::Select)),
    row(Call("ceil"), Widest, 1, false, A(Atom::Ceil)),
    row(Call("trunc"), Widest, 1, false, A(Atom::Trunc)),
    row(Call("round"), Widest, 1, false, A(Atom::RoundEven)),
    row(Infix("%"), Widest, 2, true, A(Atom::Rem)),
    row(Call("inverseSqrt"), Widest, 1, false, Each(|s, [a, _, _]| { let r = s.op(Atom::Sqrt, [a, 0, 0]); let o = s.lit(1.0); s.op(Atom::Div, [o, r, 0]) })),
    row(Call("exp2"), Widest, 1, false, A(Atom::Exp2)),
    row(Call("log2"), Widest, 1, false, A(Atom::Log2)),
    row(Call("asin"), Widest, 1, false, A(Atom::Asin)),
    row(Call("acos"), Widest, 1, false, A(Atom::Acos)),
    row(Call("atan"), Widest, 1, false, A(Atom::Atan)),
    row(Call("sinh"), Widest, 1, false, A(Atom::Sinh)),
    row(Call("cosh"), Widest, 1, false, A(Atom::Cosh)),
    row(Call("tanh"), Widest, 1, false, A(Atom::Tanh)),
];

fn each(s: &mut Ssa, a: Atom, x: &[Id], y: &[Id]) -> Vec<Id> { x.iter().zip(y).map(|(&p, &q)| s.op(a, [p, q, 0])).collect() }
/// Left to right, as compileJs sums.
fn dot(s: &mut Ssa, x: &[Id], y: &[Id]) -> Id {
    let p = each(s, Atom::Mul, x, y);
    p[1..].iter().fold(p[0], |acc, &q| s.op(Atom::Add, [acc, q, 0]))
}

/// ULP bounds of the `libm` f32 functions (musl's): the max measured against an f64 reference in `tests/it`
/// (2.07 for tanh, 1.71 sinh, 1.38 atan2, under 1 for the rest), plus one.
const T: u8 = 3;
const LN2: f32 = core::f32::consts::LN_2;

pub static ATOMS: [AtomDef; 40] = [
    at(2, |a, b, _| a + b, 1, iv::add, |g, _, [da, db, _], _| g.add(da, db)),
    at(2, |a, b, _| a - b, 1, iv::sub, |g, _, [da, db, _], _| g.sub(da, db)),
    at(2, |a, b, _| a * b, 1, iv::mul, |g, [a, b, _], [da, db, _], _| { let (x, y) = (g.mul(b, da), g.mul(a, db)); g.add(x, y) }),
    at(2, |a, b, _| a / b, 1, iv::div, |g, [_, b, _], [da, db, _], o| { let t = g.mul(o, db); let n = g.sub(da, t); g.div(n, b) }),
    at(1, |a, _, _| -a, 0, iv::neg, |g, _, [da, _, _], _| g.neg(da)),
    at(2, js_min, 0, iv::min, |g, [a, b, _], [da, db, _], _| {
        let c = g.op(Atom::Le, [a, b, 0]); g.select(c, da, db)
    }),
    at(2, js_max, 0, iv::max, |g, [a, b, _], [da, db, _], _| {
        let c = g.op(Atom::Ge, [a, b, 0]); g.select(c, da, db)
    }),
    at(2, |a, b, _| m::powf(a, b), T, iv::pow, |g, [a, b, _], [da, db, _], o| {
        // d(a^b) = a^b (db ln a + b da / a)
        let l = g.op(Atom::Log, [a, 0, 0]);
        let x = g.mul(l, db);
        let q = g.op(Atom::Div, [b, a, 0]);
        let y = g.mul(q, da);
        let s = g.add(x, y);
        g.mul(o, s)
    }),
    at(2, |y, x, _| m::atan2f(y, x), T, iv::atan2, |g, [y, x, _], [dy, dx, _], _| {
        // (x dy - y dx) / (x^2 + y^2)
        let (p, q) = (g.mul(x, dy), g.mul(y, dx));
        let n = g.sub(p, q);
        let (xx, yy) = (g.op(Atom::Mul, [x, x, 0]), g.op(Atom::Mul, [y, y, 0]));
        let r = g.op(Atom::Add, [xx, yy, 0]);
        g.div(n, r)
    }),
    // fmodf is exact. d(a - b trunc(a / b)) = da - db trunc(a / b).
    at(2, |a, b, _| m::fmodf(a, b), 0, iv::rem, |g, [a, b, _], [da, db, _], _| {
        let q = g.op(Atom::Div, [a, b, 0]);
        let t = g.op(Atom::Trunc, [q, 0, 0]);
        let p = g.mul(t, db);
        g.sub(da, p)
    }),
    at(1, |a, _, _| a.abs(), 0, iv::abs, |g, [a, _, _], [da, _, _], _| { let s = g.op(Atom::Sign, [a, 0, 0]); g.mul(s, da) }),
    // JS Math.sign: keeps -0 and NaN.
    at(1, |a, _, _| if a > 0.0 { 1.0 } else if a < 0.0 { -1.0 } else { a }, 0, iv::sign, |_, _, _, _| None),
    at(1, |a, _, _| m::floorf(a), 0, iv::floor, |_, _, _, _| None),
    at(1, |a, _, _| m::ceilf(a), 0, iv::ceil, |_, _, _, _| None),
    at(1, |a, _, _| m::truncf(a), 0, iv::trunc, |_, _, _, _| None),
    at(1, |a, _, _| m::rintf(a), 0, iv::round_even, |_, _, _, _| None),
    at(1, |a, _, _| m::sqrtf(a), 1, iv::sqrt, |g, _, [da, _, _], o| { let h = g.lit(0.5); let q = g.op(Atom::Div, [h, o, 0]); g.mul(q, da) }),
    at(1, |a, _, _| m::expf(a), T, iv::exp, |g, _, [da, _, _], o| g.mul(o, da)),
    at(1, |a, _, _| m::exp2f(a), T, iv::exp2, |g, _, [da, _, _], o| { let k = g.lit(LN2); let q = g.op(Atom::Mul, [o, k, 0]); g.mul(q, da) }),
    at(1, |a, _, _| m::logf(a), T, iv::log, |g, [a, _, _], [da, _, _], _| g.div(da, a)),
    at(1, |a, _, _| m::log2f(a), T, iv::log2, |g, [a, _, _], [da, _, _], _| { let k = g.lit(LN2); let q = g.op(Atom::Mul, [a, k, 0]); g.div(da, q) }),
    at(1, |a, _, _| m::sinf(a), T, iv::sin, |g, [a, _, _], [da, _, _], _| { let c = g.op(Atom::Cos, [a, 0, 0]); g.mul(c, da) }),
    at(1, |a, _, _| m::cosf(a), T, iv::cos, |g, [a, _, _], [da, _, _], _| { let s = g.op(Atom::Sin, [a, 0, 0]); let d = g.mul(s, da); g.neg(d) }),
    at(1, |a, _, _| m::tanf(a), T, iv::tan, |g, _, [da, _, _], o| {
        let oo = g.op(Atom::Mul, [o, o, 0]); let one = g.lit(1.0); let s = g.op(Atom::Add, [one, oo, 0]); g.mul(s, da)
    }),
    at(1, |a, _, _| m::asinf(a), T, iv::asin, |g, [a, _, _], [da, _, _], _| { let r = one_minus_sq_root(g, a); g.div(da, r) }),
    at(1, |a, _, _| m::acosf(a), T, iv::acos, |g, [a, _, _], [da, _, _], _| { let r = one_minus_sq_root(g, a); let q = g.div(da, r); g.neg(q) }),
    at(1, |a, _, _| m::atanf(a), T, iv::atan, |g, [a, _, _], [da, _, _], _| {
        let aa = g.op(Atom::Mul, [a, a, 0]); let one = g.lit(1.0); let s = g.op(Atom::Add, [one, aa, 0]); g.div(da, s)
    }),
    at(1, |a, _, _| m::sinhf(a), T + 1, iv::sinh, |g, [a, _, _], [da, _, _], _| { let c = g.op(Atom::Cosh, [a, 0, 0]); g.mul(c, da) }),
    at(1, |a, _, _| m::coshf(a), T + 1, iv::cosh, |g, [a, _, _], [da, _, _], _| { let s = g.op(Atom::Sinh, [a, 0, 0]); g.mul(s, da) }),
    at(1, |a, _, _| m::tanhf(a), T + 1, iv::tanh, |g, _, [da, _, _], o| {
        let oo = g.op(Atom::Mul, [o, o, 0]); let one = g.lit(1.0); let s = g.op(Atom::Sub, [one, oo, 0]); g.mul(s, da)
    }),
    at(2, |a, b, _| bit(a < b), 0, iv::lt, |_, _, _, _| None),
    at(2, |a, b, _| bit(a > b), 0, iv::gt, |_, _, _, _| None),
    at(2, |a, b, _| bit(a <= b), 0, iv::le, |_, _, _, _| None),
    at(2, |a, b, _| bit(a >= b), 0, iv::ge, |_, _, _, _| None),
    at(2, |a, b, _| bit(a == b), 0, iv::eq, |_, _, _, _| None),
    at(2, |a, b, _| bit(a != b), 0, iv::ne, |_, _, _, _| None),
    // JS `a && b`, `a || b`: the deciding operand (a number when authored on numbers, so it has a derivative).
    at(2, |a, b, _| if truthy(a) { b } else { a }, 0, iv::and, |g, [a, _, _], [da, db, _], _| g.select(a, db, da)),
    at(2, |a, b, _| if truthy(a) { a } else { b }, 0, iv::or, |g, [a, _, _], [da, db, _], _| g.select(a, da, db)),
    at(1, |a, _, _| bit(!truthy(a)), 0, iv::not, |_, _, _, _| None),
    at(3, |c, y, n| if truthy(c) { y } else { n }, 0, iv::select, |g, [c, _, _], [_, dy, dn], _| g.select(c, dy, dn)),
];

// JS Math.min/max: NaN if either is NaN, and -0 < +0. Rust's `f32::min`/`max` leave the sign of an equal zero to
// the target: aarch64 (fminnm) and x86 (minss) disagree on min(-0, +0), which broke check (d) across hosts.
fn js_min(a: f32, b: f32, _: f32) -> f32 {
    if a.is_nan() || b.is_nan() { f32::NAN } else if a < b || (a == b && a.is_sign_negative()) { a } else { b }
}
fn js_max(a: f32, b: f32, _: f32) -> f32 {
    if a.is_nan() || b.is_nan() { f32::NAN } else if a > b || (a == b && a.is_sign_positive()) { a } else { b }
}

fn one_minus_sq_root(g: &mut G, a: Id) -> Id {
    let aa = g.op(Atom::Mul, [a, a, 0]);
    let one = g.lit(1.0);
    let s = g.op(Atom::Sub, [one, aa, 0]);
    g.op(Atom::Sqrt, [s, 0, 0])
}
