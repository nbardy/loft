//! Unproject the packed ID and depth at the selected pixel.
use loft_ir::PickResult;
#[path = "../../shared/visibility_ids.rs"]
mod visibility;
use visibility::visibility_ids;

fn miss(depth_ndc: f32) -> PickResult {
    PickResult {
        hit: false,
        kind_id: 0,
        row_id: 0,
        payload: 0,
        depth_ndc,
        world_pos: [0.; 3],
    }
}

pub fn decode(
    [id, payload, z]: [u32; 3],
    px: [u32; 2],
    size: [u32; 2],
    inv: [[f32; 4]; 4],
) -> PickResult {
    let z = f32::from_bits(z);
    let (kind_id, row_id) = visibility_ids(id, payload);
    if kind_id == 0 {
        return miss(z);
    }
    let p = [
        2. * (px[0] as f32 + 0.5) / size[0] as f32 - 1.,
        1. - 2. * (px[1] as f32 + 0.5) / size[1] as f32,
        z,
        1.,
    ];
    let world: [f32; 4] = std::array::from_fn(|row| (0..4).map(|col| inv[col][row] * p[col]).sum());
    if !world.iter().all(|x| x.is_finite()) || world[3] == 0. {
        return miss(z);
    }
    PickResult {
        hit: true,
        kind_id,
        row_id,
        payload,
        depth_ndc: z,
        world_pos: std::array::from_fn(|i| world[i] / world[3]),
    }
}
