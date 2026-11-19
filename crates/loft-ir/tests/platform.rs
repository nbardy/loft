use loft_ir::*;
use std::mem::{offset_of, size_of};
#[test]
fn platform_abi() {
    let features = [
        GpuFeatures::TIMESTAMPS,
        GpuFeatures::PIPELINE_CACHE,
        GpuFeatures::SUBGROUPS,
        GpuFeatures::F16,
        GpuFeatures::INT_ATOMICS,
        GpuFeatures::MULTI_DRAW_INDIRECT,
        GpuFeatures::BINDLESS_TEXTURES,
        GpuFeatures::RAY_QUERY,
        GpuFeatures::PACKED_INT8_DOT,
        GpuFeatures::COOP_MATRIX,
    ];
    for (bit, feature) in features.into_iter().enumerate() {
        assert_eq!(feature.bits(), 1 << bit);
    }
    assert_eq!(size_of::<OutboxHeader>(), 16);
    assert_eq!(
        [
            offset_of!(OutboxHeader, write_count),
            offset_of!(OutboxHeader, capacity),
            offset_of!(OutboxHeader, dropped),
            offset_of!(OutboxHeader, frame_idx)
        ],
        [0, 4, 8, 12]
    );
    assert_eq!(size_of::<OutboxEvent>(), 32);
    assert_eq!(
        [
            offset_of!(OutboxEvent, tag),
            offset_of!(OutboxEvent, instance_id),
            offset_of!(OutboxEvent, site_id),
            offset_of!(OutboxEvent, tick),
            offset_of!(OutboxEvent, payload)
        ],
        [0, 4, 8, 12, 16]
    );
    assert_eq!(size_of::<PickResult>(), 32);
    assert_eq!(
        [
            offset_of!(PickResult, hit),
            offset_of!(PickResult, kind_id),
            offset_of!(PickResult, row_id),
            offset_of!(PickResult, payload),
            offset_of!(PickResult, depth_ndc),
            offset_of!(PickResult, world_pos)
        ],
        [0, 4, 8, 12, 16, 20]
    );
}
