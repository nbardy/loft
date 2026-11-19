// Mesh descriptors precede the asset's packed local triangles in the read-only table.
fn mesh_position(base: u32, vertex: u32) -> vec3f {
    let i = base + 3u * vertex;
    return vec3f(luts[i], luts[i + 1u], luts[i + 2u]);
}
fn mesh_material(row: Row, triangle: u32) -> Bsdf {
    let m = row.r_asset * 8u;
    let descriptor = bitcast<u32>(luts[m + 7u]);
    let ranges = bitcast<u32>(luts[descriptor + 2u]);
    if ranges != 0u {
        // Find the first exclusive primitive end above this triangle.
        var lo = 0u;
        var hi = bitcast<u32>(luts[ranges]);
        while lo < hi {
            let mid = lo + (hi - lo) / 2u;
            let slot = ranges + 1u + 6u * mid;
            if triangle < bitcast<u32>(luts[slot]) { hi = mid; } else { lo = mid + 1u; }
        }
        let slot = ranges + 1u + 6u * lo;
        return Bsdf(vec3f(luts[slot + 1u], luts[slot + 2u], luts[slot + 3u]),
                    luts[slot + 4u], luts[slot + 5u]);
    }
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
struct MeshHit { t: f32, triangle: u32 }

fn mesh_overlaps(ro: vec3f, rd: vec3f, lo: vec3f, hi: vec3f, interval: vec2f) -> bool {
    var near = interval.x;
    var far = interval.y;
    for (var axis = 0u; axis < 3u; axis++) {
        if rd[axis] == 0.0 {
            if ro[axis] < lo[axis] || ro[axis] > hi[axis] { return false; }
        } else {
            let a = (lo[axis] - ro[axis]) / rd[axis];
            let b = (hi[axis] - ro[axis]) / rd[axis];
            near = max(near, min(a, b));
            far = min(far, max(a, b));
        }
    }
    return near <= far;
}

fn mesh_intersect(row: Row, ro: vec3f, rd: vec3f, interval: vec2f, any: bool) -> MeshHit {
    let header = row.r_asset * 8u;
    let base = bitcast<u32>(luts[header]);
    let descriptor = bitcast<u32>(luts[header + 7u]);
    let nodes = bitcast<u32>(luts[descriptor]);
    let leaves = bitcast<u32>(luts[descriptor + 1u]);
    var nearest = interval.y;
    var triangle = 0xffffffffu;
    // The CPU builder halves a u32-bounded triangle set at every split.
    // Its depth fits 32 deferred siblings.
    var stack: array<u32, 32>;
    var depth = 1u;
    loop {
        if depth == 0u { break; }
        depth -= 1u;
        let node = nodes + 8u * stack[depth];
        let lo = vec3f(luts[node], luts[node + 1u], luts[node + 2u]);
        let hi = vec3f(luts[node + 3u], luts[node + 4u], luts[node + 5u]);
        if !mesh_overlaps(ro, rd, lo, hi, vec2f(interval.x, nearest)) { continue; }
        let first = bitcast<u32>(luts[node + 6u]);
        let count = bitcast<u32>(luts[node + 7u]);
        if count == 0u {
            stack[depth] = first + 1u;
            stack[depth + 1u] = first;
            depth += 2u;
            continue;
        }
        for (var i = 0u; i < count; i++) {
            let candidate = bitcast<u32>(luts[leaves + first + i]);
            let t = mesh_hit(base, candidate, ro, rd);
            if t >= interval.x && t <= nearest {
                if any { return MeshHit(t, candidate); }
                if t < nearest || candidate < triangle {
                    nearest = t;
                    triangle = candidate;
                }
            }
        }
    }
    return MeshHit(select(nearest, -1.0, triangle == 0xffffffffu), triangle);
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
