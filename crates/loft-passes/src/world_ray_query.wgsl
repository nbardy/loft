@group(3) @binding(RAY_QUERY_SLOT) var loft_ray_bounds: acceleration_structure;

fn world_secondary(world: u32, ray: LoftRay, any_hit: bool) -> LoftBvhHit {
    var best_id = vec2u(0xffffffffu);
    var best_t = -1.0;
    if world_root(world) == 0xffffffffu || ray.RAY_NEAR >= ray.RAY_FAR {
        return LoftBvhHit(LoftLeafRef(best_id.x, best_id.y), best_t);
    }
    let base = bitcast<u32>(luts[5]);
    let stride = bitcast<u32>(luts[6]);
    var query: ray_query;
    rayQueryInitialize(&query, loft_ray_bounds, RayDesc(0u, 0xffu, ray.RAY_NEAR, ray.RAY_FAR, ray.RAY_ORIGIN, ray.RAY_DIRECTION));
    var ray_segment = ray;
    while rayQueryProceed(&query) {
        let candidate = rayQueryGetCandidateIntersection(&query);
        let word = base + stride * candidate.primitive_index;
        let mask = bitcast<u32>(luts[word + 2u + world / 32u]);
        if (mask & (1u << (world % 32u))) == 0u { continue; }
        let id = vec2u(bitcast<u32>(luts[word]), bitcast<u32>(luts[word + 1u]));
        let leaf = LoftLeafRef(id.x, id.y);
        let t = loft_bvh_intersect(leaf, ray_segment);
        if t < -1.0 { return LoftBvhHit(leaf, t); }
        if t >= ray_segment.RAY_NEAR && t <= ray_segment.RAY_FAR {
            let earlier = id.x < best_id.x || (id.x == best_id.x && id.y < best_id.y);
            if any_hit { return LoftBvhHit(leaf, t); }
            if best_t < 0.0 || t < best_t || (t == best_t && earlier) {
                best_id = id;
                best_t = t;
                ray_segment.RAY_FAR = t;
            }
        }
    }
    return LoftBvhHit(LoftLeafRef(best_id.x, best_id.y), best_t);
}
