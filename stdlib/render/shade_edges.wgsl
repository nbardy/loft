const LOFT_PRIMARY_WORLD = {{PRIMARY_WORLD}}u;
fn loft_edge_sky(rd: vec3f) -> vec3f { return {{SKY}}(rd, normalize(frame.sun.xyz)); }
// Refine coverage where the raster IDs disagree. Interiors reuse their
// stored row. Match the standard four sample positions and shade unique rows.
fn loft_shade_samples(center: LoftRay, px: vec2f, vis: vec2u, edge: bool) -> vec3f {
  let taps = array(vec2f(0.375, 0.125), vec2f(0.875, 0.375),
                   vec2f(0.125, 0.625), vec2f(0.625, 0.875));
  // Replay only the stored row at the center. Keeping its intersection in
  // compute gives the same rounding as the edge taps at secondary ray origins.
  if !edge {
    let leaf = LoftLeafRef(vis.x >> 24u, vis.x & 0xffffffu);
    let center_t = loft_bvh_intersect(leaf, center);
    let t = select(bitcast<f32>(vis.y), center_t, center_t >= 0.0);
    return loft_shade_hit(leaf, center, t, -center.d, px);
  }
  let root = world_root(LOFT_PRIMARY_WORLD);
  let sky = loft_edge_sky(center.d);
  var keys: array<vec2u, 4>;
  var colors: array<vec3f, 4>;
  var count = 0u;
  var sum = vec3f(0.0);
  for (var i = 0u; i < 4u; i++) {
    var ray = center;
    ray.d = loft_ray(px + taps[i]);
    let tap = loft_bvh_trace_at(ray, root, false);
    if tap.t < 0.0 {
      sum += select(vec3f(0.0), sky, tap.t == -1.0);
      continue;
    }
    let key = vec2u(tap.leaf.kind_id, tap.leaf.row_id);
    var slot = 0u;
    while slot < count && any(keys[slot] != key) { slot++; }
    if slot == count {
      var t = tap.t;
      let center_t = loft_bvh_intersect(tap.leaf, center);
      if center_t >= 0.0 {
        ray = center;
        t = center_t;
      }
      keys[slot] = key;
      colors[slot] = loft_shade_hit(tap.leaf, ray, t, -center.d, px);
      count++;
    }
    sum += colors[slot];
  }
  return sum * 0.25;
}
