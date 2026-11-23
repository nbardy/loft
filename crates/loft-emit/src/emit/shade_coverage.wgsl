// `edges: coverage`: analytic edge antialiasing with no extra BVH traversal.
// The closest approach of this pixel's own ray to a neighbour's row, in
// pixels, is the signed distance to that row's silhouette; the 1-pixel box
// filter of it is the coverage (agent_notes/2026-11-23_perf/perf-edges.md).
fn loft_coverage_usable(key: u32) -> bool { return {{USABLE}}; }

// Min of the row's field along the ray near t0: three taps one pixel apart and
// the vertex of their parabola (one Newton step on the closest approach). A hit
// in front of a rounded rim sits several pixels before the rim's depth, where
// the field alone reads ~0. Every value is a field sample on the ray, so the
// minimum never under-reports the gap of an exact SDF. One loop keeps one
// call site of the field (Metal inlines each call site).
fn loft_coverage_reach(key: u32, o: vec3f, d: vec3f, t0: f32, w: f32, t_max: f32) -> f32 {
{{SETUP}}
  var h = vec3f(0.0);
  var reach = 1e30;
  for (var i = 0u; i < 4u; i++) {
    var t = t0 + (f32(i) - 1.0) * w;
    if i == 3u {
      let bend = h.x - 2.0 * h.y + h.z;
      if !(bend > 0.0) { break; }
      t = t0 + clamp(0.5 * w * (h.x - h.z) / bend, -2.0 * w, 64.0 * w);
    }
    // Behind the pixel's own hit the ray is blocked.
    let x = o + d * min(t, t_max);
    let f = {{FIELD}};
    if i < 3u { h[i] = f; }
    reach = min(reach, f);
  }
  return reach;
}

const LOFT_COVERAGE_STEPS = array(vec2i(1, 0), vec2i(-1, 0), vec2i(0, 1), vec2i(0, -1));

// HDR alpha for present: 0 sky, 1 plain, +-(2 + k + w) blend toward 4-neighbour
// k by w <= 0.5, negative when this pixel itself is sky.
fn loft_coverage_alpha(px: vec2u, v: vec2u, edge: bool) -> f32 {
  let base = select(1.0, 0.0, v.x == LOFT_EMPTY);
  if !edge { return base; }
  let c = vec2f(px) + 0.5;
  let d = loft_ray(c);
  let angle = length(loft_ray(c + vec2f(1.0, 0.0)) - d);
  let own_t = select(bitcast<f32>(v.y), 1e30, v.x == LOFT_EMPTY);
  let limit = vec2i(frame.size.xy) - 1;
  var weight = 0.0;
  var side = 0u;
  for (var k = 0u; k < 4u; k++) {
    let q = vec2i(px) + LOFT_COVERAGE_STEPS[k];
    if any(q < vec2i(0)) || any(q > limit) { continue; }
    let n = textureLoad(vis_tex, q, 0).xy;
    if n.x == v.x { continue; }
    // A nearer neighbour occludes this pixel near its silhouette; a farther one
    // (or sky) is the background of this pixel's own silhouette.
    let n_t = select(bitcast<f32>(n.y), 1e30, n.x == LOFT_EMPTY);
    let occluded = n_t < own_t;
    let key = select(v.x, n.x, occluded);
    if key == LOFT_EMPTY || !loft_coverage_usable(key) { continue; }
    let t0 = select(own_t, n_t, occluded);
    let w = t0 * angle;
    let s = -loft_coverage_reach(key, frame.eye.xyz, d, t0, w, select(1e30, own_t, occluded)) / w;
    let blend = clamp(0.5 + select(-s, s, occluded), 0.0, 0.5);
    if blend > weight {
      weight = blend;
      side = k;
    }
  }
  if weight <= 0.0 { return base; }
  let code = 2.0 + f32(side) + weight;
  return select(code, -code, v.x == LOFT_EMPTY);
}
