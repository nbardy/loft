//! JavaScriptCore's scalar Math behavior where it differs from Rust's.
//! Bun 1.3.10 WebKit 0ddf6f47: JavaScriptCore/runtime/{MathCommon,MathObject}.cpp.

/// Math.min/max propagate NaN and order the two signed zeros.
pub(super) fn min(a: f64, b: f64) -> f64 {
    if a.is_nan() || b.is_nan() {
        f64::NAN
    } else if a < b || (a == b && a.is_sign_negative()) {
        a
    } else {
        b
    }
}

pub(super) fn max(a: f64, b: f64) -> f64 {
    if a.is_nan() || b.is_nan() {
        f64::NAN
    } else if a > b || (a == b && a.is_sign_positive()) {
        a
    } else {
        b
    }
}

pub(super) fn sign(a: f64) -> f64 {
    if a == 0.0 || a.is_nan() { a } else { a.signum() }
}

/// The TS WGSL round helper first uses Math.round (ties toward +infinity).
pub(super) fn round_half_even(x: f64) -> f64 {
    let ceil = x.ceil();
    let r = ceil - u8::from(ceil - 0.5 > x) as f64;
    if r - x == 0.5 && r % 2.0 != 0.0 { r - 1.0 } else { r }
}

/// The JSC fast paths precede libm pow on 64-bit hosts.
pub(super) fn pow(x: f64, y: f64) -> f64 {
    let abs = x.abs();
    if y.is_nan() || (abs == 1.0 && y.is_infinite()) {
        return f64::NAN;
    }
    if y == 0.5 {
        return if abs == 0.0 || abs.is_infinite() { abs } else { x.sqrt() };
    }
    if y == -0.5 {
        return if abs == 0.0 || abs.is_infinite() { 1.0 / abs } else { 1.0 / x.sqrt() };
    }
    let mut exponent = y as i32;
    if f64::from(exponent) == y && (0..=1000).contains(&exponent) {
        // A small non-negative integer exponent multiplies in this order.
        let mut result = 1.0;
        let mut base = x;
        while exponent != 0 {
            if exponent & 1 != 0 {
                result *= base;
            }
            base *= base;
            exponent >>= 1;
        }
        return result;
    }
    x.powf(y)
}

/// JavaScript ToUint32, including the wrap used by ToInt32 and Math.imul.
pub(super) fn to_uint32(x: f64) -> u32 {
    let residue = x.trunc() % 4_294_967_296.0;
    if residue < 0.0 { (residue as i64) as u32 } else { residue as u32 }
}

/// A JS number written back into generated source via String(number).
pub(super) fn source_number(x: f64) -> f64 {
    // String(-0) is "0"; compile-time arithmetic still uses the original sign.
    if x == 0.0 { 0.0 } else { x }
}

/// Math.hypot in Bun's 64-bit macOS build. Two arguments use libm, three use
/// libc++'s scaled sum, and more use JSC's compensated sum.
pub fn hypot(values: &[f64]) -> f64 {
    if values.iter().any(|v| v.is_infinite()) {
        return f64::INFINITY;
    }
    if values.iter().any(|v| v.is_nan()) {
        return f64::NAN;
    }
    match values {
        [] => 0.0,
        [x] => x.abs(),
        [x, y] => x.hypot(*y),
        [x, y, z] => {
            const THRESHOLD: f64 = f64::from_bits((1023 + 512) << 52);
            const SCALE: f64 = f64::from_bits((1023 - 532) << 52);
            let max = x.abs().max(y.abs()).max(z.abs());
            let scale = if max > THRESHOLD {
                SCALE
            } else if max < 1.0 / THRESHOLD {
                1.0 / SCALE
            } else {
                1.0
            };
            let (x, y, z) = (x * scale, y * scale, z * scale);
            (x * x + y * y + z * z).sqrt() / scale
        }
        _ => {
            let max = values.iter().fold(0.0_f64, |m, x| m.max(x.abs()));
            let max = if max == 0.0 { 1.0 } else { max };
            // Kahan summation reduces the error of the sum of squares.
            let (mut sum, mut compensation) = (0.0, 0.0);
            for x in values {
                let scaled = x / max;
                let summand = scaled.mul_add(scaled, -compensation);
                let preliminary = sum + summand;
                compensation = (preliminary - sum) - summand;
                sum = preliminary;
            }
            sum.sqrt() * max
        }
    }
}
