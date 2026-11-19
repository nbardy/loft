// Mesh descriptors precede the asset's packed local triangles in the read-only table.
fn mesh_position(base: u32, vertex: u32) -> vec3f {
    let i = base + 3u * vertex;
    return vec3f(luts[i], luts[i + 1u], luts[i + 2u]);
}
fn mesh_material(row: Row) -> Bsdf {
    let m = row.r_asset * 8u;
    return Bsdf(vec3f(luts[m + 2u], luts[m + 3u], luts[m + 4u]), luts[m + 5u], luts[m + 6u]);
}
fn mesh_hit(base: u32, tri: u32, ro: vec3f, rd: vec3f) -> f32 {
    let a = mesh_position(base, tri * 3u);
    let ab = mesh_position(base, tri * 3u + 1u) - a;
    let ac = mesh_position(base, tri * 3u + 2u) - a;
    let p = cross(rd, ac);
    let det = dot(ab, p);
    if det == 0.0 { return -1.0; }
    let s = ro - a;
    let u = dot(s, p) / det;
    let q = cross(s, ab);
    let v = dot(rd, q) / det;
    return select(-1.0, dot(ac, q) / det, u >= 0.0 && v >= 0.0 && u + v <= 1.0);
}
struct MeshVertex {
    @builtin(position) clip: vec4f,
    @location(0) @interpolate(flat) row: u32,
    @location(1) @interpolate(flat) triangle: u32,
}
@vertex fn vs(@builtin(vertex_index) vi: u32, @builtin(instance_index) ii: u32) -> MeshVertex {
    let r = visible[kp.base + ii];
    let row = load_row(r);
    let m = row.r_asset * 8u;
    let count = bitcast<u32>(luts[m + 1u]);
    if vi >= count * 3u { return MeshVertex(vec4f(2.0, 2.0, 2.0, 1.0), r, 0u); }
    let p = mesh_position(bitcast<u32>(luts[m]), vi);
    return MeshVertex(frame.view_proj * vec4f(loft_xf_pt(loft_pose(row), p), 1.0), r, vi / 3u);
}
@fragment fn fs(v: MeshVertex) -> @location(0) vec2u {
    if frame.cold.x == LOFT_NONCE { discard; }
    return vec2u((kp.kind_id << 24u) | v.row, v.triangle);
}
