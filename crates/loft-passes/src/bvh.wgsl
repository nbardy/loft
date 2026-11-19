struct LoftBvhNode {
  lo: vec3<f32>,
  left_or_first: u32,
  hi: vec3<f32>,
  count: u32,
}

struct LoftLeafRef { kind_id: u32, row_id: u32 }
struct LoftRay { o: vec3<f32>, d: vec3<f32>, t0: f32, t1: f32 }
struct LoftBvhHit { leaf: LoftLeafRef, t: f32 }

// An empty world binds a dummy node and supplies node_count = 0.
fn loft_bvh_span(ray: LoftRay, lo: vec3<f32>, hi: vec3<f32>) -> vec2<f32> {
  var near = ray.t0;
  var far = ray.t1;
  for (var axis = 0u; axis < 3u; axis++) {
    let o = ray.o[axis];
    let d = ray.d[axis];
    if d == 0.0 {
      if o < lo[axis] || o > hi[axis] { return vec2<f32>(1.0, -1.0); }
    } else {
      let a = (lo[axis] - o) / d;
      let b = (hi[axis] - o) / d;
      near = max(near, min(a, b));
      far = min(far, max(a, b));
    }
  }
  return vec2<f32>(near, far);
}

// The kind switch supplies a forward distance, -1 for a miss, or a value below
// -1 for an unresolved query. The result preserves that failure in either mode.
fn loft_bvh_trace(ray: LoftRay, node_count: u32, any_hit: bool) -> LoftBvhHit {
  var best = LoftBvhHit(LoftLeafRef(0xffffffffu, 0xffffffffu), -1.0);
  if node_count == 0u { return best; }
  let root = loft_bvh_nodes[0u];
  let span = loft_bvh_span(ray, root.lo, root.hi);
  if span.x > span.y { return best; }

  var stack: array<u32, 32>;
  var enters: array<f32, 32>;
  stack[0u] = 0u;
  enters[0u] = span.x;
  var depth = 1u;
  var active_ray = ray;
  while depth > 0u {
    depth -= 1u;
    if enters[depth] > active_ray.t1 { continue; }
    let node = loft_bvh_nodes[stack[depth]];
    let first = node.left_or_first;
    if node.count == 0u {
      let a = loft_bvh_nodes[first];
      let b = loft_bvh_nodes[first + 1u];
      let sa = loft_bvh_span(active_ray, a.lo, a.hi);
      let sb = loft_bvh_span(active_ray, b.lo, b.hi);
      if sa.x <= sa.y && sb.x <= sb.y {
        let a_first = sa.x <= sb.x;
        stack[depth] = select(first, first + 1u, a_first);
        enters[depth] = select(sa.x, sb.x, a_first);
        stack[depth + 1u] = select(first + 1u, first, a_first);
        enters[depth + 1u] = select(sb.x, sa.x, a_first);
        depth += 2u;
      } else if sa.x <= sa.y || sb.x <= sb.y {
        stack[depth] = select(first + 1u, first, sa.x <= sa.y);
        enters[depth] = select(sb.x, sa.x, sa.x <= sa.y);
        depth += 1u;
      }
      continue;
    }
    for (var j = 0u; j < node.count; j++) {
      let leaf = loft_bvh_leaves[first + j];
      let t = loft_bvh_intersect(leaf, active_ray);
      if t < -1.0 { return LoftBvhHit(leaf, t); }
      if t >= active_ray.t0 && t <= active_ray.t1 {
        let earlier_id = leaf.kind_id < best.leaf.kind_id ||
          (leaf.kind_id == best.leaf.kind_id && leaf.row_id < best.leaf.row_id);
        if any_hit || best.t < 0.0 || t < best.t || (t == best.t && earlier_id) {
          best = LoftBvhHit(leaf, t);
          if any_hit { return best; }
          active_ray.t1 = t;
        }
      }
    }
  }
  return best;
}

// Evaluate the row's signed field in its own space. The kind switch passes
// a local ray with the world AABB's clipped interval and the row's derived L.
fn loft_bvh_march(leaf: LoftLeafRef, ray: LoftRay, lip: f32) -> f32 {
  var t = ray.t0;
  for (var i = 0u; i < 192u; i++) {
    let d = abs(loft_bvh_field(leaf, ray.o + t * ray.d));
    if d <= 1e-6 { return t; }
    let next = t + d / lip;
    if next > ray.t1 { return -1.0; }
    if !(next > t) { return -2.0; }
    t = next;
  }
  return -2.0;
}
