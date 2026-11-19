// Header: world and medium counts, then (BVH root, medium count) for each world.
// The remaining words pack homogeneous field rows, five vec4s per row.
@group(2) @binding(5) var<storage, read> world_data: array<u32>;

fn world_root(world: u32) -> u32 {
  return world_data[2u + 2u * world];
}

fn world_medium_count(world: u32) -> u32 {
  return world_data[3u + 2u * world];
}

fn medium_row(i: u32, field: u32) -> vec4<f32> {
  let start = 2u + 2u * world_data[0] + 20u * i + 4u * field;
  return bitcast<vec4<f32>>(vec4(world_data[start], world_data[start+1u], world_data[start+2u], world_data[start+3u]));
}

struct WorldSigma { s: vec3<f32>, t: vec3<f32>, g: f32, emission: vec3<f32> }

fn world_medium(world: u32, p: vec3<f32>) -> WorldSigma {
  var s = vec3(0.0);
  var t = vec3(0.0);
  var e = vec3(0.0);
  var g = 0.0;
  for (var i = 0u; i < world_medium_count(world); i++) {
    let lo = medium_row(i, 0u);
    if all(p >= lo.xyz) && all(p <= medium_row(i, 1u).xyz) {
      let scatter = medium_row(i, 3u).xyz;
      t += medium_row(i, 2u).xyz;
      e += medium_row(i, 4u).xyz;
      s += scatter;
      g += dot(scatter, vec3(0.2126, 0.7152, 0.0722)) * lo.w;
    }
  }
  let weight = dot(s, vec3(0.2126, 0.7152, 0.0722));
  return WorldSigma(s, t, g / max(weight, 1e-30), e);
}

fn world_volume(world: u32, ray: LoftRay, steps: u32, illuminate: bool) -> LoftVol {
  var vol = LoftVol(vec3(0.0), vec3(1.0));
  let count = world_medium_count(world);
  if count == 0u || steps == 0u { return vol; }
  let sun = normalize(frame.sun.xyz);
  let mu = dot(ray.d, sun);
  // Split at every bound crossing. Empty distance to the sky never dilutes
  // samples inside a finite slab, and coincident slabs share each integration step.
  var begin = ray.t0;
  while begin < ray.t1 {
    var end = ray.t1;
    for (var i = 0u; i < count; i++) {
      let span = loft_bvh_span(ray, medium_row(i, 0u).xyz, medium_row(i, 1u).xyz);
      if span.x < span.y {
        if span.x > begin { end = min(end, span.x); }
        if span.y > begin { end = min(end, span.y); }
      }
    }
    let middle = begin + (end - begin) * 0.5;
    let sigma = world_medium(world, ray.o + ray.d * middle);
    if any(sigma.t > vec3(0.0)) || any(sigma.emission > vec3(0.0)) {
      let source = select(vec3(0.0), sigma.emission, illuminate);
      if illuminate && any(sigma.s > vec3(0.0)) {
        let u = 1.0 + sigma.g * sigma.g - 2.0 * sigma.g * mu;
        let phase = (1.0 - sigma.g * sigma.g) * 0.0795774715 / (u * sqrt(u));
        let ds = (end - begin) / f32(steps);
        for (var k = 0u; k < steps; k++) {
          let p = ray.o + ray.d * (begin + (f32(k) + 0.5) * ds);
          let shadow = LoftRay(p, sun, 0.0001, 1e6);
          let hit = loft_bvh_trace_at(shadow, world_root(world), true);
          let direct = select(vec3(0.0), sigma.s * loft_params[0].xyz * phase, hit.t == -1.0);
          vol = loft_volume_step(vol, sigma.t, source + direct, ds);
        }
      } else {
        vol = loft_volume_step(vol, sigma.t, source, end - begin);
      }
    }
    begin = end;
  }
  return vol;
}

fn world_visibility(world: u32, ray: LoftRay) -> vec3<f32> {
  let hit = loft_bvh_trace_at(ray, world_root(world), true);
  if hit.t != -1.0 { return vec3(0.0); }
  return world_volume(world, ray, 1u, false).t;
}

fn loft_visible(world: u32, a: vec3<f32>, b: vec3<f32>) -> vec3<f32> {
  let d = b - a;
  let len = length(d);
  if len <= 0.0002 { return vec3(1.0); }
  let dir = d / len;
  let hit = loft_bvh_trace_at(LoftRay(a, dir, 0.0001, len - 0.0001), world_root(world), true);
  if hit.t != -1.0 { return vec3(0.0); }
  return world_volume(world, LoftRay(a, dir, 0.0, len), 1u, false).t;
}

fn loft_trace(world: u32, ray: loft_type_Ray) -> loft_type_Hit {
  let h = world_trace_ray(world, LoftRay(ray.o, ray.d, ray.t0, ray.t1));
  if h.t < 0.0 { return loft_type_Hit(h.t, h.p, h.n, vec2(0.0), h.p, 0xffffffffu, 0xffffffffu, vec4(0.0)); }
  return loft_type_Hit(h.t, h.p, h.n, vec2(0.0), world_local(h.kind, h.at, h.p), h.kind, h.row, vec4(0.0));
}
