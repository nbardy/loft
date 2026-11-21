use loft_std::mips::{Clock, Dimension, source};

#[test]
fn every_dimension_and_format_validates() {
    for dim in [Dimension::D2, Dimension::D3, Dimension::Cube] {
        for (format, scalar) in [("r8unorm", true), ("r16float", true), ("r32float", true),
            ("rgba8unorm", false), ("rgba16float", false)]
        {
            let text = source(dim, format, scalar, dim.group(64, 256));
            let ir =
                naga::front::wgsl::parse_str(&text).unwrap_or_else(|e| panic!("{}", e.emit_to_string(&text)));
            naga::valid::Validator::new(
                naga::valid::ValidationFlags::all(),
                naga::valid::Capabilities::default(),
            )
            .validate(&ir)
            .unwrap_or_else(|e| panic!("{}", e.emit_to_string(&text)));
        }
    }
    assert_eq!(Dimension::Cube.extent([128, 128, 6], 7), [1, 1, 6]);
    assert_eq!(Dimension::D3.extent([2, 5, 64], 3), [1, 1, 8]);
}

#[test]
fn initial_and_dirty_clocks_are_edge_triggered() {
    let mut load = Clock::Load { done: false };
    let mut dirty = Clock::Dirty {
        words: vec![1],
        last: None,
    };
    assert!(load.due(true, &[]));
    assert!(!dirty.due(true, &[]));
    load.complete(&[]);
    assert!(!load.due(true, &[]));
    assert!(!load.due(false, &[]));
    assert!(dirty.due(false, &[1.0, 0.0]));
    dirty.complete(&[1.0, 0.0]);
    assert!(!dirty.due(false, &[0.0, 0.0]));
    assert!(dirty.due(false, &[0.0, 1.0]));
    // No encode yet, so a reverted edit remains clean.
    assert!(!dirty.due(false, &[0.0, 0.0]));
    dirty.complete(&[0.0, 1.0]);
    assert!(!dirty.due(false, &[0.0, 1.0]));
}
