@group(2) @binding(0) var<storage, read> world_words: array<f32>;
@group(2) @binding(1) var<storage, read> loft_bvh_nodes: array<LoftBvhNode>;
@group(2) @binding(2) var<storage, read> luts: array<f32>;
@group(2) @binding(3) var<storage, read> world_lamps: array<PathLamp>;

@group(2) @binding(4) var<storage, read> loft_bvh_leaves: array<LoftLeafRef>;

fn world_word(i: u32) -> f32 { return world_words[i]; }
fn world_lamp(i: u32) -> PathLamp { return world_lamps[i]; }
