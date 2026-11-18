//! JavaScript's `Math` on f64, bit for bit as Bun (JavaScriptCore) computes it on this platform. The TypeScript twin
//! evaluates in these functions; a kernel that must give the same bytes must too. The differences from Rust's own:
//! - `min`/`max` propagate NaN and order -0 below +0 (Rust's `f64::min` drops NaN).
//! - `pow` with a nonnegative int32 exponent is square-and-multiply (JSC `operationMathPow`), not libm's `pow`, which
//!   rounds differently for exponents above 2. Checked by `probe` against Bun (tools in rust/loft-core/README.md).
//! - `sign` of 0 keeps the zero's sign; `round` is JS round-half-up.
//! - pow's operands are opaque to LLVM, which would rewrite pow(2, y) as exp2(y) (a different last bit).
//! The transcendental functions (sin, cos, exp, log, atan2, ...) call the platform libm, as JSC does on Darwin.

#[inline]
pub fn min(a: f64, b: f64) -> f64 {
    if a.is_nan() || b.is_nan() {
        return f64::NAN;
    }
    if a == b {
        // -0 < +0 for Math.min
        return if a.is_sign_negative() { a } else { b };
    }
    if a < b { a } else { b }
}

#[inline]
pub fn max(a: f64, b: f64) -> f64 {
    if a.is_nan() || b.is_nan() {
        return f64::NAN;
    }
    if a == b {
        return if a.is_sign_positive() { a } else { b };
    }
    if a > b { a } else { b }
}

/// JSC: a nonnegative int32 exponent multiplies by squaring; anything else is libm `pow` after JS's special cases.
pub fn pow(x: f64, y: f64) -> f64 {
    if y.is_nan() {
        return f64::NAN;
    }
    if y.is_infinite() && x.abs() == 1.0 {
        return f64::NAN;
    }
    if y >= 0.0 && y <= i32::MAX as f64 && (y as i32) as f64 == y {
        let mut n = y as i32;
        let mut r = 1.0f64;
        let mut b = x;
        while n != 0 {
            if n & 1 != 0 {
                r *= b;
            }
            b *= b;
            n >>= 1;
        }
        return r;
    }
    // Opaque operands: LLVM rewrites pow(2, y) as exp2(y), which differs from libm pow (and JSC) in the last bit.
    std::hint::black_box(x).powf(std::hint::black_box(y))
}

#[inline]
pub fn sign(x: f64) -> f64 {
    if x.is_nan() || x == 0.0 { x } else if x > 0.0 { 1.0 } else { -1.0 }
}

/// `Math.round`: halves toward +infinity.
pub fn round(x: f64) -> f64 {
    if !x.is_finite() || x == 0.0 {
        return x;
    }
    let f = x.floor();
    let r = if x - f >= 0.5 { f + 1.0 } else { f };
    // Math.round(-0.4) is -0
    if r == 0.0 && x < 0.0 { -0.0 } else { r }
}

/// WGSL `round` as compileJs writes it: ties to even.
pub fn round_half_even(x: f64) -> f64 {
    let r = round(x);
    if r - x == 0.5 && r % 2.0 != 0.0 { r - 1.0 } else { r }
}

pub fn smooth(lo: f64, hi: f64, x: f64) -> f64 {
    let t = min(1.0, max(0.0, (x - lo) / (hi - lo)));
    t * t * (3.0 - 2.0 * t)
}

#[inline]
pub fn trunc(x: f64) -> f64 { x.trunc() }

/// `Math.fround`.
#[inline]
pub fn fround(x: f64) -> f64 { x as f32 as f64 }

/// The functions a probe compares with Bun, by number (the `MathFn` codes of src/core/pack.ts).
pub fn unary(code: u32, x: f64) -> f64 {
    match code {
        0 => x.sin(),
        1 => x.cos(),
        2 => x.tan(),
        3 => x.exp(),
        4 => x.ln(),
        5 => x.sqrt(),
        6 => x.asin(),
        7 => x.acos(),
        8 => x.atan(),
        9 => x.sinh(),
        10 => x.cosh(),
        11 => x.tanh(),
        12 => x.log2(),
        13 => pow(2.0, x),
        _ => f64::NAN,
    }
}
