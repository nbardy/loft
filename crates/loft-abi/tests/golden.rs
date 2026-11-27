use loft_abi::*;
use std::mem::{align_of, offset_of, size_of};
macro_rules! layout {
    ($t:ty, $size:literal, $align:literal; $($field:ident=$offset:literal),+) => {{
        assert_eq!(size_of::<$t>(), $size);
        assert_eq!(align_of::<$t>(), $align);
        $(assert_eq!(offset_of!($t, $field), $offset, "{}.{}", stringify!($t), stringify!($field));)+
    }};
}
#[test]
fn storage_layouts() {
    assert_eq!((size_of::<MeshAsset>(), align_of::<MeshAsset>()), (4, 4));
    layout!(MeshRow, 56, 4; pose=0, asset=48, lod=52);
    layout!(BvhNode, 32, 16; lo=0, left_or_first=12, hi=16, count=28);
    layout!(LeafRef, 8, 4; kind_id=0, row_id=4);
    layout!(VerdictRecord, 32, 4; constraint_id=0, instance_id=4, site_id=8, tick=12, tag=16, reason_code=20, value=24, law_hash_lo=28);
    layout!(AdvanceArgs, 12, 4; from_tick=0, to_tick=4, keep_stride=8);
}

#[test]
fn verdict_codes() {
    use VerdictTag::*;
    assert_eq!(
        [Skipped, Held, Missed, Unknown].map(|tag| tag as u32),
        [0, 1, 2, 3]
    );
    use VerdictReason::*;
    assert_eq!(
        [
            None,
            IterCap,
            NonFinite,
            MissingSurface,
            MissingFrame,
            SolveFailed
        ]
        .map(|reason| reason as u32),
        [0, 1, 2, 3, 4, 5]
    );
}

#[test]
fn visibility_lanes() {
    let v = VisRec::pack(0x123, true, 0x20fedc, 0x89abcdef).unwrap();
    assert_eq!(v.0, [0xfedc, 0x91e0, 0xcdef, 0x89ab]);
    assert_eq!(v.unpack(), (0x123, true, 0x20fedc, 0x89abcdef));
    assert_eq!(
        VisRec::pack(511, true, 0x3fffff, u32::MAX).unwrap().0,
        [0xffff; 4]
    );
    assert!(VisRec::pack(512, false, 0, 0).is_none());
    assert!(VisRec::pack(1, false, 1 << 22, 0).is_none());
}

#[test]
fn deterministic_outputs() {
    use det::*;
    assert_eq!(
        SIN.map(f32::to_bits),
        [
            0x2f309231, 0xb2d7322b, 0x3638ef1d, 0xb9500d01, 0x3c088889, 0xbe2aaaab
        ]
    );
    assert_eq!(
        ATAN.map(f32::to_bits),
        [
            0x3d9d89d9, 0xbdba2e8c, 0x3de38e39, 0xbe124925, 0x3e4ccccd, 0xbeaaaaab
        ]
    );
    assert_eq!(
        EXP.map(f32::to_bits),
        [
            0x39500d01, 0x3ab60b61, 0x3c088889, 0x3d2aaaab, 0x3e2aaaab, 0x3f000000
        ]
    );
    assert_eq!(
        [
            det_sin(0.125_f32),
            det_cos(0.125_f32),
            det_atan2(-0.75_f32, 0.125_f32),
            det_acos(0.125_f32),
            det_exp(0.125_f32)
        ]
        .map(f32::to_bits),
        [0x3dff5577, 0x3f7e00aa, 0xbfb3ec44, 0x3fb9051d, 0x3f910b02]
    );
    assert_eq!(
        [
            det_sin(-0.9375_f32),
            det_cos(-0.9375_f32),
            det_atan2(0.0625_f32, -0.9375_f32),
            det_acos(-0.9375_f32),
            det_exp(-0.9375_f32)
        ]
        .map(f32::to_bits),
        [0xbf4e5b55, 0x3f17808a, 0x4044cd34, 0x403250a2, 0x3ec88088]
    );
    assert_eq!(
        [
            det_sin(0.75_f32),
            det_cos(0.75_f32),
            det_atan2(0.75_f32, 0.75_f32),
            det_acos(0.75_f32),
            det_exp(0.75_f32)
        ]
        .map(f32::to_bits),
        [0x3f2e7fe1, 0x3f3b4ff6, 0x3f490fdb, 0x3f39051d, 0x40077cee]
    );
    assert_eq!(
        [
            det_sin(0_f32),
            det_cos(0_f32),
            det_atan2(0_f32, 0_f32),
            det_acos(0_f32),
            det_exp(0_f32)
        ]
        .map(f32::to_bits),
        [0x00000000, 0x3f800000, 0x00000000, 0x3fc90fdb, 0x3f800000]
    );
}
#[test]
fn philox_vectors() {
    use rand::*;
    assert_eq!(
        philox([0, 0, 0, 0], [0, 0]),
        [0x5f6fb709, 0x0d893f64, 0x4f121f81, 0x4f730a48]
    );
    assert_eq!(
        philox([1, 2, 3, 4], [5, 6]),
        [0xcceb838b, 0x94b8d4ab, 0x3b19758c, 0x0e1a9304]
    );
    assert_eq!(
        philox(
            [4294967295, 4294967295, 4294967295, 4294967295],
            [4294967295, 4294967295]
        ),
        [0x5207ddc2, 0x45165e59, 0x4d8ee751, 0x8c52f662]
    );
    assert_eq!(rng(123, 17).to_bits(), 0x3ce04160);
    assert_eq!(hash1(17).to_bits(), 0x3eff8d40);
    assert_eq!(hash2([-0.25, 7.5]).to_bits(), 0x3f077fda);
    assert_eq!(hash3([-0.25, 7.5, 3.5]).to_bits(), 0x3f37799e);
    assert_eq!(
        rng_key([15.5, 3.0], 2, 1),
        [0x2ce6f4d2, 0x63f78ec5, 0x6ab658cd, 0x344d99d3]
    );
}
