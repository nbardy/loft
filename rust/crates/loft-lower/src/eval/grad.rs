//! Symbolic forward differentiation of the hash-consed DAG (src/grad.ts).
//! Each pass computes a tangent once per node. Tangents are ordinary expressions,
//! shared and simplified through the same constructors that build the primal graph.
use crate::{
    Error,
    graph::{Arena, Expr, In, LibId, LoopId, Op, Prim, RowId, Ty},
};
use std::collections::HashMap;
use std::f64::consts::LN_2;

// Evaluate operands in order before borrowing the arena to build their parent.
macro_rules! prim {
    ($a:expr, $op:ident, $($x:expr),+ $(,)?) => {{
        let args: &[In<'_>] = &[$(($x).into()),+];
        $a.prim(Prim::$op, args)
    }};
}
macro_rules! simplify {
    ($a:expr, $op:ident, $x:expr, $y:expr) => {{
        let (x, y) = ($x, $y);
        $op($a, x, y)
    }};
}

mod noise;

const NUMERIC: &[Ty] = &[Ty::F32, Ty::V2, Ty::V3, Ty::V4];
const V2_V3: &[Ty] = &[Ty::V2, Ty::V3];

// At |v| = 0 this floor gives the minimum-norm subgradient when v' is zero,
// common inside sdBox. 1e-30 is normal f32, so the GPU preserves it.
const TINY: f64 = 1e-30;

fn bug(detail: String) -> Error {
    Error::Internal(format!("derivative rule bug: {detail}"))
}
fn bad_variable(detail: String) -> Error {
    Error::Scene(format!("cannot differentiate with respect to this: {detail}"))
}
fn known(a: &Arena, e: Expr) -> Option<&[f64]> {
    match &a[e].op {
        Op::Const(v) => Some(v),
        Op::Var(_) | Op::Input(_) | Op::Param(_) | Op::Row { .. } | Op::Call { .. } | Op::Vec(_)
        | Op::Get { .. } | Op::Lib { .. } | Op::Fetch { .. } | Op::Iterate { .. } => None,
    }
}
fn all_equal(a: &Arena, e: Expr, x: f64) -> bool {
    known(a, e).is_some_and(|v| v.iter().all(|&v| v == x))
}
fn is_zero(a: &Arena, e: Expr) -> bool {
    all_equal(a, e, 0.0)
}
fn is_one(a: &Arena, e: Expr) -> bool {
    all_equal(a, e, 1.0)
}
fn const_of(a: &mut Arena, v: &[f64]) -> Result<Expr, Error> {
    if let [x] = v { a.lift(*x) } else { a.lift(v) }
}
fn zero(a: &mut Arena, ty: Ty) -> Result<Expr, Error> {
    a.constant(ty, &[0.0; 4][..usize::from(ty.lits())])
}
/// The unit vector along axis i of a leaf of type ty (1 for a scalar).
fn axis(a: &mut Arena, ty: Ty, i: usize) -> Result<Expr, Error> {
    let mut v = [0.0; 4];
    v[i] = 1.0;
    a.constant(ty, &v[..usize::from(ty.lits())])
}
fn wider_ty(a: Ty, b: Ty) -> Result<Ty, Error> {
    if a == Ty::F32 {
        Ok(b)
    } else if b == Ty::F32 || a == b {
        Ok(a)
    } else {
        Err(bug(format!("mixed vector types {} and {}", a.name(), b.name())))
    }
}
/// Broadcast a scalar the same way as a scalar operand of a vector operation.
fn widen(a: &mut Arena, e: Expr, ty: Ty) -> Result<Expr, Error> {
    if a[e].ty == ty {
        return Ok(e);
    }
    if a[e].ty != Ty::F32 {
        return Err(bug(format!("cannot widen a {} to {}", a[e].ty.name(), ty.name())));
    }
    if let Some(v) = known(a, e) {
        a.constant(ty, &[v[0]; 4][..usize::from(ty.lits())])
    } else {
        a.vec_of(ty, &[e])
    }
}
/// Fold constants with scalar broadcasting.
fn fold2(a: &mut Arena, x: &[f64], y: &[f64], op: fn(f64, f64) -> f64) -> Result<Expr, Error> {
    let v: Vec<_> = (0..x.len().max(y.len()))
        .map(|i| op(x[if x.len() == 1 { 0 } else { i }], y[if y.len() == 1 { 0 } else { i }]))
        .collect();
    const_of(a, &v)
}
fn s_add(a: &mut Arena, x: Expr, y: Expr) -> Result<Expr, Error> {
    let ty = wider_ty(a[x].ty, a[y].ty)?;
    if let (Some(x), Some(y)) = (known(a, x), known(a, y)) {
        let (x, y) = (x.to_vec(), y.to_vec());
        return fold2(a, &x, &y, |x, y| x + y);
    }
    if is_zero(a, x) { return widen(a, y, ty); }
    if is_zero(a, y) { return widen(a, x, ty); }
    prim!(a, Add, x, y)
}
fn s_neg(a: &mut Arena, x: Expr) -> Result<Expr, Error> {
    if let Some(k) = known(a, x) {
        let v: Vec<_> = k.iter().map(|x| -x).collect();
        return const_of(a, &v);
    }
    if let Op::Call { prim: Prim::Neg, args } = &a[x].op {
        return Ok(args[0]);
    }
    prim!(a, Neg, x)
}
fn s_sub(a: &mut Arena, x: Expr, y: Expr) -> Result<Expr, Error> {
    let ty = wider_ty(a[x].ty, a[y].ty)?;
    if let (Some(x), Some(y)) = (known(a, x), known(a, y)) {
        let (x, y) = (x.to_vec(), y.to_vec());
        return fold2(a, &x, &y, |x, y| x - y);
    }
    if x == y { return zero(a, ty); }
    if is_zero(a, y) { return widen(a, x, ty); }
    if is_zero(a, x) {
        let y = s_neg(a, y)?;
        return widen(a, y, ty);
    }
    prim!(a, Sub, x, y)
}
fn s_mul(a: &mut Arena, x: Expr, y: Expr) -> Result<Expr, Error> {
    let ty = wider_ty(a[x].ty, a[y].ty)?;
    if is_zero(a, x) || is_zero(a, y) { return zero(a, ty); }
    if let (Some(x), Some(y)) = (known(a, x), known(a, y)) {
        let (x, y) = (x.to_vec(), y.to_vec());
        return fold2(a, &x, &y, |x, y| x * y);
    }
    if is_one(a, x) { return widen(a, y, ty); }
    if is_one(a, y) { return widen(a, x, ty); }
    prim!(a, Mul, x, y)
}
fn s_div(a: &mut Arena, x: Expr, y: Expr) -> Result<Expr, Error> {
    let ty = wider_ty(a[x].ty, a[y].ty)?;
    if is_zero(a, x) { return zero(a, ty); }
    if is_one(a, y) { return widen(a, x, ty); }
    prim!(a, Div, x, y)
}
/// WGSL dot takes vectors. Two scalars just multiply.
fn s_dot(a: &mut Arena, x: Expr, y: Expr) -> Result<Expr, Error> {
    let ty = wider_ty(a[x].ty, a[y].ty)?;
    if is_zero(a, x) || is_zero(a, y) { return a.lift(0.0); }
    if ty == Ty::F32 { return s_mul(a, x, y); }
    prim!(a, Dot, widen(a, x, ty)?, widen(a, y, ty)?)
}
fn s_cross(a: &mut Arena, x: Expr, y: Expr) -> Result<Expr, Error> {
    if is_zero(a, x) || is_zero(a, y) { return zero(a, Ty::V3); }
    prim!(a, Cross, x, y)
}
fn s_abs(a: &mut Arena, x: Expr) -> Result<Expr, Error> {
    if is_zero(a, x) { Ok(x) } else { prim!(a, Abs, x) }
}
/// A scalar is its own every component.
fn comp(a: &mut Arena, x: Expr, i: usize) -> Result<Expr, Error> {
    if a[x].ty == Ty::F32 { return Ok(x); }
    if let Some(v) = known(a, x) { return a.lift(v[i]); }
    a.get(x, &"xyzw"[i..i + 1])
}
/// A scalar select per component. WGSL select needs both sides to have the result's type.
fn pick_each(
    a: &mut Arena, ty: Ty, yes: Expr, no: Expr, mut cond: impl FnMut(&mut Arena, usize) -> Result<Expr, Error>,
) -> Result<Expr, Error> {
    if is_zero(a, yes) && is_zero(a, no) { return zero(a, ty); }
    if yes == no { return widen(a, yes, ty); }
    if ty == Ty::F32 { return prim!(a, Select, cond(a, 0)?, yes, no); }
    let parts = (0..usize::from(ty.lits()))
        .map(|i| prim!(a, Select, cond(a, i)?, comp(a, yes, i)?, comp(a, no, i)?))
        .collect::<Result<Vec<_>, _>>()?;
    a.vec_of(ty, &parts)
}
fn swizzle(a: &mut Arena, x: Expr, sel: &str, ty: Ty) -> Result<Expr, Error> {
    let index = |c| b"xyzw".iter().position(|&x| x == c).expect("validated swizzle");
    match sel {
        "x" | "y" | "z" | "w" => comp(a, x, index(sel.as_bytes()[0])),
        "xy" | "xz" | "xyz" => a.get(x, sel),
        _ => {
            let parts = sel.bytes().map(|c| comp(a, x, index(c))).collect::<Result<Vec<_>, _>>()?;
            a.vec_of(ty, &parts)
        }
    }
}
/// pow(a, b - 1) without pow for the common small exponents.
fn pow_below(a: &mut Arena, x: Expr, y: Expr) -> Result<Expr, Error> {
    match known(a, y) {
        Some([1.0]) => a.lift(1.0),
        Some([2.0]) => Ok(x),
        Some(v) => {
            let v: Vec<_> = v.iter().map(|v| v - 1.0).collect();
            prim!(a, Pow, x, const_of(a, &v)?)
        }
        None => prim!(a, Pow, x, prim!(a, Sub, y, 1.0)?),
    }
}
/// Read two points inside x's cell, a quarter and three quarters across. This
/// gives the slope of the actual piecewise-linear lookup as the source row is
/// edited live. A baked table of centered differences would go stale and its
/// entries would be derivatives of a different interpolant. The slope outside
/// the clamped domain is zero.
fn row_slope(a: &mut Arena, r: RowId, x: Expr, dx: Expr) -> Result<Expr, Error> {
    if is_zero(a, dx) { return zero(a, Ty::V4); }
    let last = (super::CURVE_N - 1) as f64;
    let cell = prim!(a, Min, prim!(a, Floor, prim!(a, Mul, prim!(a, Saturate, x)?, last)?)?, last - 1.0)?;
    let at = |a: &mut Arena, frac: f64| {
        let u = prim!(a, Div, prim!(a, Add, cell, frac)?, last)?;
        a.row(r, u)
    };
    let slope = prim!(a, Mul, prim!(a, Sub, at(a, 0.75)?, at(a, 0.25)?)?, 2.0 * last)?;
    let inside = prim!(a, And, prim!(a, Ge, x, 0.0)?, prim!(a, Le, x, 1.0)?)?;
    simplify!(a, s_mul, prim!(a, Select, inside, slope, zero(a, Ty::V4)?)?, dx)
}

/// Registered derivative of a library call, given its arguments and their tangents.
pub type DerivativeRule = fn(&mut Arena, &[Expr], &[Expr]) -> Result<Expr, Error>;

/// Derivative registrations and generated library functions for one arena.
pub struct Differentiation<'a> {
    pub arena: &'a mut Arena,
    rules: Rules,
}

#[derive(Default)]
struct Rules {
    registered: HashMap<LibId, DerivativeRule>,
    libs: HashMap<String, LibId>,
    hessians: HashMap<Ty, (Expr, Expr, Expr)>,
}

type Seeds = HashMap<Expr, Expr>;

struct Pass<'a> {
    arena: &'a mut Arena,
    rules: &'a mut Rules,
    seeds: &'a Seeds,
    memo: HashMap<Expr, Expr>,
    loops: HashMap<LoopId, Vec<Expr>>,
}

impl Pass<'_> {
    fn tan(&mut self, e: Expr) -> Result<Expr, Error> {
        if let Some(&d) = self.memo.get(&e) { return Ok(d); }
        let ty = self.arena[e].ty;
        if ty == Ty::Bool {
            return Err(Error::Scene(format!("cannot differentiate a bool expression (node {} ({}))", e.0, op_name(&self.arena[e].op))));
        }
        let d = self.node_rule(e)?;
        if self.arena[d].ty != ty {
            return Err(bug(format!(
                "the tangent of a {} {} node came out {}",
                ty.name(), op_name(&self.arena[e].op), self.arena[d].ty.name()
            )));
        }
        self.memo.insert(e, d);
        Ok(d)
    }

    fn node_rule(&mut self, e: Expr) -> Result<Expr, Error> {
        let ty = self.arena[e].ty;
        match self.arena[e].op.clone() {
            Op::Const(_) => zero(self.arena, ty),
            // Unseeded leaves are constant.
            Op::Var(_) | Op::Input(_) | Op::Param(_) => {
                if let Some(&d) = self.seeds.get(&e) { Ok(d) } else { zero(self.arena, ty) }
            }
            Op::Row { row, x } => {
                let dx = self.tan(x)?;
                row_slope(self.arena, row, x, dx)
            }
            Op::Call { prim, args } => self.call_rule(e, prim, &args),
            Op::Vec(args) => {
                let ds = self.tangents(&args)?;
                let a = &mut *self.arena;
                if ds.iter().all(|&d| is_zero(a, d)) { zero(a, ty) } else { a.vec_of(ty, &ds) }
            }
            Op::Get { a, sel } => {
                let d = self.tan(a)?;
                let a = &mut *self.arena;
                if is_zero(a, d) { zero(a, ty) } else { swizzle(a, d, &sel, ty) }
            }
            Op::Lib { fun, args } => {
                let ds = self.tangents(&args)?;
                if ds.iter().all(|&d| is_zero(self.arena, d)) {
                    return zero(self.arena, ty);
                }
                self.rules.lib_rule(self.arena, fun, &args, &ds)
            }
            // A truncated table index is piecewise constant. Derivatives cannot see
            // the table's CPU fill, even when its data depends on a seeded leaf.
            Op::Fetch { .. } => zero(self.arena, Ty::V4),
            Op::Iterate { lo, k } => self.loop_tangent(lo, k),
        }
    }

    fn tangents(&mut self, roots: &[Expr]) -> Result<Vec<Expr>, Error> {
        roots.iter().map(|&e| self.tan(e)).collect()
    }

    fn call_rule(&mut self, e: Expr, op: Prim, args: &[Expr]) -> Result<Expr, Error> {
        // The rule requests only the tangents it needs. In particular, select
        // never differentiates the condition, and comparisons are constant on each piece.
        macro_rules! args {
            ($($i:literal),*) => {{
                ($(self.tan(args[$i])?,)* &mut *self.arena)
            }};
        }
        let ty = self.arena[e].ty;
        let x = args[0];
        use Prim::*;
        match op {
            Add | Sub => {
                let (dx, dy, a) = args!(0, 1);
                if op == Add { s_add(a, dx, dy) } else { s_sub(a, dx, dy) }
            }
            Neg => {
                let (dx, a) = args!(0);
                s_neg(a, dx)
            }
            Mul => {
                let (dx, dy, a) = args!(0, 1);
                simplify!(a, s_add, s_mul(a, dx, args[1])?, s_mul(a, x, dy)?)
            }
            // Reuse the quotient node: (a / b)' = (a' - (a / b) b') / b.
            Div => {
                let (dx, dy, a) = args!(0, 1);
                simplify!(a, s_div, simplify!(a, s_sub, dx, s_mul(a, e, dy)?)?, args[1])
            }
            // At a tie use the first operand's piece.
            Min | Max => {
                let (dx, dy, a) = args!(0, 1);
                pick_each(a, ty, dx, dy, |a, i| {
                    let (x, y) = (comp(a, x, i)?, comp(a, args[1], i)?);
                    a.prim(if op == Min { Le } else { Ge }, &[x, y])
                })
            }
            Pow => {
                // A seed-independent exponent uses the power rule so negative bases
                // with integer exponents stay defined. A varying exponent needs log(a).
                let dy = self.tan(args[1])?;
                let (dx, a) = args!(0);
                let y = args[1];
                if is_zero(a, dy) {
                    simplify!(a, s_mul, simplify!(a, s_mul, y, pow_below(a, x, y)?)?, dx)
                } else {
                    let first = simplify!(a, s_mul, dy, prim!(a, Log, x)?)?;
                    let second = simplify!(a, s_div, s_mul(a, y, dx)?, x)?;
                    simplify!(a, s_mul, e, s_add(a, first, second)?)
                }
            }
            // floor and trunc are constant between jumps.
            Mod | Rem => {
                let dy = self.tan(args[1])?;
                let (dx, a) = args!(0);
                if is_zero(a, dy) { return Ok(dx); }
                let q = prim!(a, Div, x, args[1])?;
                let q = a.prim(if op == Mod { Floor } else { Trunc }, &[q])?;
                simplify!(a, s_sub, dx, s_mul(a, dy, q)?)
            }
            // atan2(y, x)' = (x y' - y x') / (x^2 + y^2).
            Atan2 => {
                let (dx, dy, a) = args!(0, 1);
                let y = args[1];
                let n = simplify!(a, s_sub, s_mul(a, y, dx)?, s_mul(a, x, dy)?)?;
                simplify!(a, s_div, n, prim!(a, Add, prim!(a, Mul, y, y)?, prim!(a, Mul, x, x)?)?)
            }
            Step | Sign | Floor | Ceil | Trunc | RoundEven => zero(self.arena, ty),
            Fract => self.tan(x),
            Abs => {
                let (dx, a) = args!(0);
                simplify!(a, s_mul, prim!(a, Sign, x)?, dx)
            }
            Sqrt => {
                let (dx, a) = args!(0);
                simplify!(a, s_div, dx, prim!(a, Mul, 2.0, prim!(a, Max, e, TINY)?)?)
            }
            Exp => {
                let (dx, a) = args!(0);
                s_mul(a, e, dx)
            }
            Log => {
                let (dx, a) = args!(0);
                s_div(a, dx, x)
            }
            Sin | Cos | Sinh | Cosh => {
                let (dx, a) = args!(0);
                let other = match op {
                    Sin => Cos, Cos => Sin, Sinh => Cosh, Cosh => Sinh,
                    Add | Sub | Mul | Div | Neg | Min | Max | Pow | Mod | Atan2 | Step | Abs | Sign | Floor | Fract
                    | Sqrt | Exp | Log | Tan | Saturate | Normalize | Mix | Clamp | Smoothstep | Length | Distance | Dot
                    | Cross | Lt | Gt | Le | Ge | Eq | Ne | And | Or | Not | Select | Ceil | Trunc | RoundEven | Rem
                    | InverseSqrt | Exp2 | Log2 | Asin | Acos | Atan | Tanh => unreachable!("trigonometric derivative"),
                };
                let d = simplify!(a, s_mul, a.prim(other, &[x])?, dx)?;
                if op == Cos { s_neg(a, d) } else { Ok(d) }
            }
            Tan | Tanh => {
                let (dx, a) = args!(0);
                let sq = prim!(a, Mul, e, e)?;
                let h = a.prim(if op == Tan { Add } else { Sub }, &[In::Number(1.0), sq.into()])?;
                s_mul(a, h, dx)
            }
            Saturate => {
                let (dx, a) = args!(0);
                let z = zero(a, ty)?;
                pick_each(a, ty, dx, z, |a, i| prim!(a, And, prim!(a, Ge, comp(a, x, i)?, 0.0)?, prim!(a, Le, comp(a, x, i)?, 1.0)?))
            }
            // n = v / |v|: n' = (v' - n (n . v')) / |v|. Scalar normalize is sign(v).
            Normalize => {
                if ty == Ty::F32 { return zero(self.arena, ty); }
                let (dx, a) = args!(0);
                let n = simplify!(a, s_sub, dx, simplify!(a, s_mul, e, s_dot(a, e, dx)?)?)?;
                simplify!(a, s_div, n, prim!(a, Max, prim!(a, Length, x)?, TINY)?)
            }
            // mix(a, b, t) = a + (b - a) t.
            Mix => {
                let dt = self.tan(args[2])?;
                let (dx, dy, a) = args!(0, 1);
                let along = simplify!(a, s_add, dx, simplify!(a, s_mul, s_sub(a, dy, dx)?, args[2])?)?;
                if is_zero(a, dt) { return Ok(along); }
                simplify!(a, s_add, along, simplify!(a, s_mul, prim!(a, Sub, args[1], x)?, dt)?)
            }
            // Use lo's derivative below lo, hi's above hi, x's between.
            Clamp => {
                let (dz, dx, a) = args!(2, 0);
                let inner = pick_each(a, ty, dz, dx, |a, i| prim!(a, Gt, comp(a, x, i)?, comp(a, args[2], i)?))?;
                let (dy, a) = args!(1);
                pick_each(a, ty, dy, inner, |a, i| prim!(a, Lt, comp(a, x, i)?, comp(a, args[1], i)?))
            }
            // s(t) = t^2 (3 - 2t), t = saturate(r). s'(t) is zero at 0 and
            // 1, so the slope already vanishes outside the ramp.
            Smoothstep => {
                let [lo, hi, x] = [x, args[1], args[2]];
                let a = &mut *self.arena;
                let w = prim!(a, Sub, hi, lo)?;
                let r = prim!(a, Div, prim!(a, Sub, x, lo)?, w)?;
                let (dx, dlo, dhi, a) = args!(2, 0, 1);
                let nr = simplify!(a, s_sub, s_sub(a, dx, dlo)?, simplify!(a, s_mul, r, s_sub(a, dhi, dlo)?)?)?;
                let dr = s_div(a, nr, w)?;
                if is_zero(a, dr) { return zero(a, ty); }
                let t = prim!(a, Saturate, r)?;
                let dt = simplify!(a, s_mul, prim!(a, Mul, prim!(a, Mul, t, prim!(a, Sub, 1.0, t)?)?, 6.0)?, dr)?;
                widen(a, dt, ty)
            }
            Length => {
                let (dx, a) = args!(0);
                simplify!(a, s_div, s_dot(a, x, dx)?, prim!(a, Max, e, TINY)?)
            }
            Distance => {
                let (dx, dy, a) = args!(0, 1);
                let dd = s_sub(a, dx, dy)?;
                if is_zero(a, dd) { return a.lift(0.0); }
                let n = simplify!(a, s_dot, prim!(a, Sub, x, args[1])?, dd)?;
                simplify!(a, s_div, n, prim!(a, Max, e, TINY)?)
            }
            Dot | Cross => {
                let (dx, dy, a) = args!(0, 1);
                let f = if op == Dot { s_dot } else { s_cross };
                simplify!(a, s_add, f(a, dx, args[1])?, f(a, x, dy)?)
            }
            Select => {
                let (yes, no, a) = args!(1, 2);
                if is_zero(a, yes) && is_zero(a, no) { return zero(a, ty); }
                prim!(a, Select, x, widen(a, yes, ty)?, widen(a, no, ty)?)
            }
            Lt | Gt | Le | Ge | Eq | Ne | And | Or | Not => {
                Err(Error::Scene(format!("cannot differentiate a bool expression (the {} result of a comparison or logic operator)", ty.name())))
            }
            // (x^-1/2)' = -x^-3/2 / 2 = -e^3 / 2.
            InverseSqrt => {
                let (dx, a) = args!(0);
                simplify!(a, s_mul, prim!(a, Mul, prim!(a, Mul, e, prim!(a, Mul, e, e)?)?, -0.5)?, dx)
            }
            Exp2 => {
                let (dx, a) = args!(0);
                simplify!(a, s_mul, prim!(a, Mul, e, LN_2)?, dx)
            }
            Log2 => {
                let (dx, a) = args!(0);
                simplify!(a, s_div, dx, prim!(a, Mul, x, LN_2)?)
            }
            // asin' = 1 / sqrt(1 - x^2), acos' = -asin'. Floor the
            // denominator as for sqrt to keep a finite slope at the endpoints.
            Asin | Acos => {
                let (dx, a) = args!(0);
                let denom = prim!(a, Max, prim!(a, Sqrt, prim!(a, Sub, 1.0, prim!(a, Mul, x, x)?)?)?, TINY)?;
                let d = s_div(a, dx, denom)?;
                if op == Acos { s_neg(a, d) } else { Ok(d) }
            }
            Atan => {
                let (dx, a) = args!(0);
                simplify!(a, s_div, dx, prim!(a, Add, 1.0, prim!(a, Mul, x, x)?)?)
            }
        }
    }

    fn loop_tangent(&mut self, lo: LoopId, k: usize) -> Result<Expr, Error> {
        if let Some(ds) = self.loops.get(&lo) { return Ok(ds[k]); }
        let l = self.arena.loop_at(lo).clone();
        let dinit = self.tangents(&l.init)?;
        let reads_seeds = self.arena.topo(&l.next).iter().any(|e| self.seeds.contains_key(e));
        let a = &mut *self.arena;
        let ds = if dinit.iter().all(|&d| is_zero(a, d)) && !reads_seeds {
            l.init.iter().map(|&e| zero(a, a[e].ty)).collect::<Result<Vec<_>, _>>()?
        } else {
            // Advance (state, tangent) together. The step's tangent seeds are
            // the outer seeds plus the new state binders and their tangents.
            // The count is truncated, so it contributes nothing.
            let n = l.init.len();
            let init: Vec<_> = l.init.iter().copied().chain(dinit).collect();
            let roots: Vec<_> = l.next.iter().copied().chain(l.stop).collect();
            let out = a.iterate(l.count, &init, |a, st, i| {
                let (s, t) = st.split_at(n);
                let binders = l.state.iter().copied().zip(s.iter().copied()).chain([(l.index, i)]).collect();
                let rebuilt = replace(a, &roots, &binders)?;
                let (next, stop) = rebuilt.split_at(n);
                let mut seeds = self.seeds.clone();
                seeds.extend(s.iter().copied().zip(t.iter().copied()));
                let ds = forward(a, self.rules, next, &seeds)?;
                Ok((next.iter().copied().chain(ds).collect(), stop.first().copied()))
            })?;
            out[n..].to_vec()
        };
        let d = ds[k];
        self.loops.insert(lo, ds);
        Ok(d)
    }
}

fn forward(a: &mut Arena, rules: &mut Rules, roots: &[Expr], seeds: &Seeds) -> Result<Vec<Expr>, Error> {
    Pass { arena: a, rules, seeds, memo: HashMap::new(), loops: HashMap::new() }.tangents(roots)
}

/// Rebind a recorded body through the constructors, rebuilding affected nested
/// loops once over fresh binders so their scoped nodes stay distinct.
fn replace(a: &mut Arena, roots: &[Expr], by: &Seeds) -> Result<Vec<Expr>, Error> {
    let mut done = HashMap::new();
    let mut loops: HashMap<LoopId, Vec<Expr>> = HashMap::new();
    for e in a.topo(roots) {
        let out = if let Some(&to) = by.get(&e) {
            to
        } else {
            let kids = a.children(e);
            let args: Vec<_> = kids.iter().map(|x| done[x]).collect();
            if args == kids {
                e
            } else {
                match a[e].op.clone() {
                    Op::Row { row, .. } => a.row(row, args[0])?,
                    Op::Lib { fun, .. } => a.lib(fun, &args)?,
                    Op::Call { prim, .. } => a.prim(prim, &args)?,
                    Op::Vec(_) => a.vec_of(a[e].ty, &args)?,
                    Op::Get { sel, .. } => a.get(args[0], &sel)?,
                    Op::Fetch { table, lane, .. } => a.fetch(table, args[0], lane)?,
                    Op::Iterate { lo, k } => {
                        if let Some(made) = loops.get(&lo) {
                            made[k]
                        } else {
                            let l = a.loop_at(lo).clone();
                            let roots: Vec<_> = l.next.into_iter().chain(l.stop).collect();
                            let made = a.iterate(args[0], &args[1..1 + l.init.len()], |a, s, i| {
                                let mut inner = by.clone();
                                inner.extend(l.state.iter().copied().zip(s.iter().copied()).chain([(l.index, i)]));
                                let mut next = replace(a, &roots, &inner)?;
                                let stop = l.stop.and_then(|_| next.pop());
                                Ok((next, stop))
                            })?;
                            let out = made[k];
                            loops.insert(lo, made);
                            out
                        }
                    }
                    Op::Const(_) | Op::Var(_) | Op::Input(_) | Op::Param(_) => e,
                }
            }
        };
        done.insert(e, out);
    }
    Ok(roots.iter().map(|e| done[e]).collect())
}

fn op_name(op: &Op) -> &'static str {
    match op {
        Op::Const(_) => "const",
        Op::Var(_) => "var",
        Op::Input(_) => "input",
        Op::Param(_) => "param",
        Op::Row { .. } => "row",
        Op::Call { .. } => "call",
        Op::Vec(_) => "vec",
        Op::Get { .. } => "get",
        Op::Lib { .. } => "lib",
        Op::Fetch { .. } => "fetch",
        Op::Iterate { .. } => "iterate",
    }
}

/// Differentiate only with respect to a numeric variable, input or parameter.
fn leaf_of(a: &Arena, x: Expr, tys: &[Ty], role: &str) -> Result<Ty, Error> {
    let n = &a[x];
    match n.op {
        Op::Var(_) | Op::Input(_) | Op::Param(_) => {
            if tys.contains(&n.ty) { return Ok(n.ty); }
            let expected = tys.iter().copied().map(Ty::name).collect::<Vec<_>>().join(" or ");
            Err(bad_variable(format!("{role} must be {expected}, got {}", n.ty.name())))
        }
        Op::Const(_) | Op::Row { .. } | Op::Call { .. } | Op::Vec(_) | Op::Get { .. } | Op::Lib { .. } | Op::Fetch { .. }
        | Op::Iterate { .. } => Err(bad_variable(format!(
            "{role} must be a variable, input or param leaf, got a \"{}\" node",
            op_name(&n.op)
        ))),
    }
}

/// How a leaf varies per pixel in screen x and y.
pub struct ScreenRate<'a> {
    pub of: Expr,
    pub dx: In<'a>,
    pub dy: In<'a>,
}

/// A coordinate's change per screen pixel and the WGSL fwidth, |dx| + |dy|.
pub struct Footprint {
    pub dx: Expr,
    pub dy: Expr,
    pub width: Expr,
}

impl<'a> Differentiation<'a> {
    pub fn new(arena: &'a mut Arena) -> Self {
        Self { arena, rules: Rules::default() }
    }

    /// A call independent of the seeds needs no registered rule.
    pub fn define_derivative(&mut self, fun: LibId, rule: DerivativeRule) -> Result<(), Error> {
        if let Some(&other) = self.rules.registered.get(&fun)
            && !std::ptr::fn_addr_eq(other, rule)
        {
            return Err(Error::Scene(format!(
                "defineDerivative: \"{}\" already has a derivative", self.arena.lib_fn(fun).name
            )));
        }
        self.rules.registered.insert(fun, rule);
        Ok(())
    }

    fn forward(&mut self, e: Expr, seeds: &Seeds) -> Result<Expr, Error> {
        // Reuse the original library objects when the primal already uses them.
        for id in self.arena.libs_of(&[e])? {
            self.rules.libs.entry(self.arena.lib_fn(id).name.clone()).or_insert(id);
        }
        Ok(forward(self.arena, &mut self.rules, &[e], seeds)?[0])
    }

    /// de/dx for a scalar leaf x.
    pub fn derivative(&mut self, e: Expr, x: Expr) -> Result<Expr, Error> {
        leaf_of(self.arena, x, &[Ty::F32], "derivative: x")?;
        let one = self.arena.lift(1.0)?;
        self.forward(e, &HashMap::from([(x, one)]))
    }

    /// The Jacobian-vector product, d/ds e(p + s dir) at s = 0.
    pub fn derivative_along<'d>(&mut self, e: Expr, p: Expr, dir: impl Into<In<'d>>) -> Result<Expr, Error> {
        let ty = leaf_of(self.arena, p, NUMERIC, "derivativeAlong: p")?;
        let d = self.arena.lift(dir)?;
        let dt = self.arena[d].ty;
        if dt != ty {
            return Err(bad_variable(format!("direction is {} but p is {}", dt.name(), ty.name())));
        }
        self.forward(e, &HashMap::from([(p, d)]))
    }

    /// Gradient of a scalar with respect to a vec2 or vec3 leaf.
    pub fn gradient(&mut self, e: Expr, p: Expr) -> Result<Expr, Error> {
        let et = self.arena[e].ty;
        if et != Ty::F32 {
            return Err(bad_variable(format!("gradient needs a scalar expression, got {} (use jacobian)", et.name())));
        }
        let ty = leaf_of(self.arena, p, V2_V3, "gradient: p")?;
        let cols = self.jacobian(e, p)?;
        self.arena.vec_of(ty, &cols)
    }

    /// The Jacobian as columns, column j being df/dp_j.
    pub fn jacobian(&mut self, fx: Expr, p: Expr) -> Result<Vec<Expr>, Error> {
        let ty = leaf_of(self.arena, p, NUMERIC, "jacobian: p")?;
        (0..usize::from(ty.lits())).map(|j| {
            let dir = axis(self.arena, ty, j)?;
            self.derivative_along(fx, p, dir)
        }).collect()
    }

    /// normalize(ds/du x ds/dv), oriented by the parametrisation.
    pub fn normal_of(&mut self, surface: Expr, u: Expr, v: Expr) -> Result<Expr, Error> {
        let du = self.derivative(surface, u)?;
        let dv = self.derivative(surface, v)?;
        let a = &mut *self.arena;
        prim!(a, Normalize, s_cross(a, du, dv)?)
    }

    /// The square root of the largest Gershgorin row sum of J^T J bounds
    /// stretch and is exact for orthogonal columns. No safety factor is applied.
    pub fn stretch_of(&mut self, map: Expr, p: Expr) -> Result<Expr, Error> {
        leaf_of(self.arena, p, V2_V3, "stretchOf: p")?;
        let cols = self.jacobian(map, p)?;
        let a = &mut *self.arena;
        let gram = cols.iter().map(|&x| cols.iter().map(|&y| s_dot(a, x, y)).collect::<Result<Vec<_>, _>>())
            .collect::<Result<Vec<_>, _>>()?;
        let rows = gram.iter().enumerate().map(|(i, row)| {
            let mut sum = a.lift(0.0)?;
            for (j, &g) in row.iter().enumerate() {
                let x = if i == j { g } else { s_abs(a, g)? };
                sum = s_add(a, sum, x)?;
            }
            Ok(sum)
        }).collect::<Result<Vec<_>, Error>>()?;
        let max = rows[1..].iter().try_fold(rows[0], |m, &r| prim!(a, Max, m, r))?;
        prim!(a, Sqrt, max)
    }

    /// Apply the unfolded screen rates via the chain rule. Fold seams do not
    /// spike the footprint because fract, mod and abs use their local slope.
    pub fn footprint(&mut self, coord: Expr, px: &[ScreenRate<'_>]) -> Result<Footprint, Error> {
        fn seeds(a: &mut Arena, px: &[ScreenRate<'_>], pick: for<'b> fn(&ScreenRate<'b>) -> In<'b>) -> Result<Seeds, Error> {
            px.iter().map(|r| {
                let ty = leaf_of(a, r.of, NUMERIC, "footprint: of")?;
                let d = a.lift(pick(r))?;
                if a[d].ty != ty {
                    return Err(bad_variable(format!("footprint: a {} quantity was given a {} rate", ty.name(), a[d].ty.name())));
                }
                Ok((r.of, d))
            }).collect()
        }
        let sx = seeds(self.arena, px, |r| r.dx)?;
        let dx = self.forward(coord, &sx)?;
        let sy = seeds(self.arena, px, |r| r.dy)?;
        let dy = self.forward(coord, &sy)?;
        let a = &mut *self.arena;
        let width = simplify!(a, s_add, s_abs(a, dx)?, s_abs(a, dy)?)?;
        Ok(Footprint { dx, dy, width })
    }
}
