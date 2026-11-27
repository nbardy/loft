//! CPU reference for the f32 operations in det.wgsl. Each mul_add is one rounding.
#![allow(clippy::excessive_precision, clippy::lossy_float_literal)]
use std::f32::consts::{FRAC_PI_2, FRAC_PI_4, LN_2, LOG2_E, PI, TAU};
pub const SIN: [f32; 6] = [
    1. / 6227020800.,
    -1. / 39916800.,
    1. / 362880.,
    -1. / 5040.,
    1. / 120.,
    -1. / 6.,
];
pub const ATAN: [f32; 6] = [1. / 13., -1. / 11., 1. / 9., -1. / 7., 1. / 5., -1. / 3.];
pub const EXP: [f32; 6] = [1. / 5040., 1. / 720., 1. / 120., 1. / 24., 1. / 6., 0.5];
fn horner(x: f32, coefficients: [f32; 6]) -> f32 {
    coefficients[1..]
        .iter()
        .fold(coefficients[0], |p, &c| x.mul_add(p, c))
}
pub fn det_sin(a: f32) -> f32 {
    let r = (-(a / TAU + 0.5).floor()).mul_add(TAU, a);
    // WGSL sign returns positive zero for either sign of zero.
    let sign = if r > 0.0 { 1.0 } else if r < 0.0 { -1.0 } else { 0.0 };
    let x = sign * r.abs().min(PI - r.abs());
    let z = x * x;
    x * z.mul_add(horner(z, SIN), 1.)
}
pub fn det_cos(a: f32) -> f32 {
    det_sin(a + FRAC_PI_2)
}
pub fn det_atan2(y: f32, x: f32) -> f32 {
    let hi = x.abs().max(y.abs());
    if hi == 0. {
        return 0.;
    }
    let a = x.abs().min(y.abs()) / hi;
    let shift = a > 0.414213562373095;
    let r = if shift { (a - 1.) / (a + 1.) } else { a };
    let z = r * r;
    let mut v = r.mul_add(
        z.mul_add(horner(z, ATAN), 1.),
        if shift { FRAC_PI_4 } else { 0. },
    );
    if y.abs() > x.abs() {
        v = FRAC_PI_2 - v
    }
    if x < 0. {
        v = PI - v
    }
    if y < 0. { -v } else { v }
}
pub fn det_acos(x: f32) -> f32 {
    det_atan2(((1. - x) * (1. + x)).sqrt(), x)
}
fn exp_poly(r: f32) -> f32 {
    (r * r).mul_add(horner(r, EXP), 1. + r)
}
fn pow2(k: i32) -> f32 {
    f32::from_bits(((k + 127) as u32) << 23)
}
pub fn det_exp(x: f32) -> f32 {
    if x < -103.97208 {
        return 0.;
    }
    if x > 88.72283 {
        return f32::INFINITY;
    }
    let k = (x.mul_add(LOG2_E, 0.5).floor() as i32).clamp(-126, 127);
    let r = (-(k as f32)).mul_add(LN_2, x);
    if r < -0.346574 {
        let j = r.mul_add(LOG2_E, 0.5).floor() as i32;
        return pow2(-126) * (exp_poly((-(j as f32)).mul_add(LN_2, r)) * pow2(j));
    }
    pow2(k) * exp_poly(r)
}

pub fn det_log(x: f32) -> f32 {
    if x == 0.0 {
        return f32::NEG_INFINITY;
    }
    if x < 0.0 {
        return f32::from_bits(0x7fc00000);
    }
    if x == f32::INFINITY {
        return x;
    }
    // Normalize subnormals before extracting the exponent, as in det.wgsl.
    let small = x < f32::MIN_POSITIVE;
    let bits = if small { x * 8_388_608.0 } else { x }.to_bits();
    let k = (bits >> 23) as i32 - if small { 150 } else { 127 };
    let m = f32::from_bits((bits & 0x007f_ffff) | 0x3f80_0000);
    let z = (m - 1.0) / (m + 1.0);
    let w = z * z;
    let coefficients = [1. / 13., 1. / 11., 1. / 9., 1. / 7., 1. / 5., 1. / 3.];
    (k as f32).mul_add(LN_2, (2.0 * z) * w.mul_add(horner(w, coefficients), 1.0))
}
