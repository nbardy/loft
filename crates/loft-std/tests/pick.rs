use loft_std::pick::{decode, shader};

#[test]
fn picking_shader_and_unprojection() {
    for msaa in [false, true] {
        let ir = naga::front::wgsl::parse_str(&shader(msaa)).unwrap();
        naga::valid::Validator::new(
            naga::valid::ValidationFlags::all(),
            naga::valid::Capabilities::empty(),
        )
        .validate(&ir)
        .unwrap();
    }
    let inv = [
        [2., 0., 0., 0.],
        [0., 4., 0., 0.],
        [0., 0., 8., 0.],
        [1., 2., 3., 2.],
    ];
    let hit = decode([(7 << 23) | 91, 42, 0.5f32.to_bits()], [0, 0], [2, 2], inv);
    assert!(hit.hit);
    assert_eq!((hit.kind_id, hit.row_id, hit.payload), (7, 91, 42));
    assert_eq!(hit.world_pos, [0., 2., 3.5]);
    assert!(!decode([0, 0, 0], [0, 0], [2, 2], inv).hit);
}
