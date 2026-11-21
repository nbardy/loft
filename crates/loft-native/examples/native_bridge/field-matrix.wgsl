enable wgpu_cooperative_matrix;
@group(1) @binding(3) var<storage, read> Features: array<f32>;
@group(1) @binding(7) var<storage, read> Weights: array<f32>;
@group(2) @binding(5) var<storage, read_write> Image: array<f32>;
var<workgroup> hidden: array<f32, 1024>;
var<workgroup> result: array<f32, 512>;
var<workgroup> zeros: array<f32, 64>;

@compute @workgroup_size(256)
fn field(@builtin(workgroup_id) tile: vec3u, @builtin(local_invocation_index) local: u32) {
    // One 8-sample tile per 32-lane Metal SIMD group.
    let simd = local / 32u;
    let lane = local % 32u;
    let base = simd * 128u;
    let dst = simd * 64u;
    let sample = tile.x * 8u + simd;
    let input = coopLoadT<coop_mat8x8<f32,A>>(&Features[sample * 64u], 8u);
    let zero = coopLoadT<coop_mat8x8<f32,C>>(&zeros[0], 8u);
    let w0 = coopLoadT<coop_mat8x8<f32,B>>(&Weights[0], 16u);
    let w1 = coopLoadT<coop_mat8x8<f32,B>>(&Weights[8], 16u);
    coopStoreT(coopMultiplyAdd(input, w0, zero), &hidden[base], 16u);
    coopStoreT(coopMultiplyAdd(input, w1, zero), &hidden[base + 8u], 16u);
    workgroupBarrier();
    for (var i = lane; i < 128u; i += 32u) { hidden[base + i] = max(hidden[base + i], 0.0); }
    workgroupBarrier();
    let h0 = coopLoadT<coop_mat8x8<f32,A>>(&hidden[base], 16u);
    let h1 = coopLoadT<coop_mat8x8<f32,A>>(&hidden[base + 8u], 16u);
    let w2 = coopLoadT<coop_mat8x8<f32,B>>(&Weights[128], 8u);
    let w3 = coopLoadT<coop_mat8x8<f32,B>>(&Weights[192], 8u);
    coopStoreT(coopMultiplyAdd(h1, w3, coopMultiplyAdd(h0, w2, zero)), &result[dst], 8u);
    workgroupBarrier();
    Image[sample * 32u + lane] = result[dst + lane / 4u * 8u + lane % 4u];
}
