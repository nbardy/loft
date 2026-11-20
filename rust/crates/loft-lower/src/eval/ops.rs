//! The scalarised calls in expr.ts's JavaScript backend.

use super::math;
use loft_ir::graph::Prim;

#[derive(Clone, Copy, PartialEq)]
pub(super) enum Scalar {
    Number(f64),
    Bool(bool),
    Undefined,
}

impl Scalar {
    pub(super) fn number(self) -> f64 {
        match self {
            Self::Number(x) => x,
            Self::Bool(b) => u8::from(b).into(),
            Self::Undefined => f64::NAN,
        }
    }

    pub(super) fn truthy(self) -> bool {
        match self {
            Self::Number(x) => x != 0.0 && !x.is_nan(),
            Self::Bool(b) => b,
            Self::Undefined => false,
        }
    }
}

pub(super) fn each(fn_: Prim, [a, b, c]: [Scalar; 3]) -> Scalar {
    use Prim::*;
    let (x, y, z) = (a.number(), b.number(), c.number());
    let value = match fn_ {
        Add => x + y,
        Sub => x - y,
        Mul => x * y,
        Div => x / y,
        Min => math::min(x, y),
        Max => math::max(x, y),
        Pow => math::pow(x, y),
        Atan2 => x.atan2(y),
        Mod => x - y * (x / y).floor(),
        Step => f64::from(u8::from(y >= x)),
        Neg => -x,
        Abs => x.abs(),
        Sign => math::sign(x),
        Floor => x.floor(),
        Sqrt => x.sqrt(),
        Exp => x.exp(),
        Log => x.ln(),
        Sin => x.sin(),
        Cos => x.cos(),
        Tan => x.tan(),
        Fract => x - x.floor(),
        Saturate => math::min(1.0, math::max(0.0, x)),
        Mix => x + (y - x) * z,
        Clamp => math::min(z, math::max(y, x)),
        Smoothstep => {
            let t = math::min(1.0, math::max(0.0, (z - x) / (y - x)));
            t * t * (3.0 - 2.0 * t)
        }
        Lt => return Scalar::Bool(x < y),
        Gt => return Scalar::Bool(x > y),
        Le => return Scalar::Bool(x <= y),
        Ge => return Scalar::Bool(x >= y),
        Eq => return Scalar::Bool(a == b),
        Ne => return Scalar::Bool(a != b),
        And => return if a.truthy() { b } else { a },
        Or => return if a.truthy() { a } else { b },
        Not => return Scalar::Bool(!a.truthy()),
        Ceil => x.ceil(),
        Trunc => x.trunc(),
        RoundEven => math::round_half_even(x),
        // JavaScript's % is the truncated remainder, as WGSL's on floats.
        Rem => x % y,
        InverseSqrt => 1.0 / x.sqrt(),
        Exp2 => math::pow(2.0, x),
        Log2 => x.log2(),
        Asin => x.asin(),
        Acos => x.acos(),
        Atan => x.atan(),
        Sinh => x.sinh(),
        Cosh => x.cosh(),
        Tanh => x.tanh(),
        Select => return if a.truthy() { b } else { c },
        Normalize | Length | Distance | Dot | Cross => unreachable!("compiled as scalar operations"),
    };
    Scalar::Number(value)
}
