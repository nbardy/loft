use loft_abi::det::{det_log, det_sin};
use std::f32::consts::LN_2;

#[test]
fn sin_normalizes_signed_zero() {
    assert_eq!(det_sin(0.0).to_bits(), 0);
    assert_eq!(det_sin(-0.0).to_bits(), 0);
}

#[test]
fn log_matches_wgsl_range_reduction_and_special_values() {
    for (x, expected) in [
        (0.0, f32::NEG_INFINITY),
        (-0.0, f32::NEG_INFINITY),
        (-1.0, f32::from_bits(0x7fc00000)),
        (f32::INFINITY, f32::INFINITY),
        (1.0, 0.0),
        (2.0, LN_2),
        (0.5, -LN_2),
        (f32::MIN_POSITIVE, -126.0 * LN_2),
        (f32::from_bits(1), -149.0 * LN_2),
    ] {
        assert_eq!(det_log(x).to_bits(), expected.to_bits(), "x = {x}");
    }
    for x in [f32::from_bits(3), 0.75, 1.5, f32::MAX] {
        assert!(det_log(x).to_bits().abs_diff(x.ln().to_bits()) <= 2, "x = {x}");
    }
}
