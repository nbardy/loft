struct PathHit { t: f32, p: vec3<f32>, n: vec3<f32>, kind: u32, at: u32, entering: bool, row: u32 }

struct PathLamp { position: vec4<f32>, intensity: vec4<f32> }
struct PathLight { wi: vec3<f32>, l: vec3<f32>, t: f32 }
struct PathRay { o: vec3<f32>, d: vec3<f32> }
// Word zero addresses the descriptor suffix. The kind headers and row
// offsets keep their layout so the same arena serves intersection and media.
fn world_descriptor_base() -> u32 {
  return bitcast<u32>(world_word(0));
}
fn world_root(world: u32) -> u32 {
  let base = world_descriptor_base();
  if world >= bitcast<u32>(world_word(base)) { return 0xffffffffu; }
  return bitcast<u32>(world_word(base + 2u + 2u * world)) - 1u;
}
fn world_medium_count(world: u32) -> u32 {
  let base = world_descriptor_base();
  if world >= bitcast<u32>(world_word(base)) { return 0u; }
  return bitcast<u32>(world_word(base + 3u + 2u * world));
}
fn medium_row(i: u32, field: u32) -> vec4<f32> {
  let base = world_descriptor_base();
  let start = base + 2u + 2u * bitcast<u32>(world_word(base)) + 20u * i + 4u * field;
  return vec4(world_word(start), world_word(start+1u), world_word(start+2u), world_word(start+3u));
}

fn path_hash(input: u32) -> u32 {
  var h = (input ^ (input >> 16u)) * 0x7feb352du;
  h = (h ^ (h >> 15u)) * 0x846ca68bu;
  return h ^ (h >> 16u);
}
fn path_random(key: u32, i: u32) -> f32 {
  return f32(path_hash(key ^ path_hash(i)) >> 8u) / 16777216.0;
}
fn path_cosine(n: vec3<f32>, xi: vec2<f32>) -> vec3<f32> {
  let phi = 6.2831853 * xi.y;
  let r = sqrt(xi.x);
  let x = normalize(cross(select(vec3<f32>(0.0,1.0,0.0), vec3<f32>(1.0,0.0,0.0), abs(n.y)>0.9), n));
  return normalize(r * (x*cos(phi) + cross(n,x)*sin(phi)) + sqrt(1.0-xi.x)*n);
}
fn path_camera(px: vec2<f32>, seed: u32) -> PathRay {
  let d = loft_ray(px);
  if LENS_RADIUS == 0.0 { return PathRay(frame.eye.xyz, d); }
  let axis = loft_ray(vec2<f32>(frame.size.xy) * 0.5);
  let x = normalize(cross(axis, vec3<f32>(0.0,1.0,0.0)));
  let y = cross(x, axis);
  let r = LENS_RADIUS * sqrt(path_random(seed, 0xffffffd0u));
  let phi = 6.2831853 * path_random(seed, 0xffffffd1u);
  let offset = r * (cos(phi)*x + sin(phi)*y);
  let focal_point = d * (FOCUS / max(1e-4,dot(axis,d)));
  return PathRay(frame.eye.xyz + offset, normalize(focal_point-offset));
}
fn world_at(leaf: LoftLeafRef) -> u32 {
  let i = 1u + 2u * leaf.kind_id;
  return bitcast<u32>(world_word(i)) + leaf.row_id * bitcast<u32>(world_word(i+1u));
}
fn loft_bvh_field(leaf: LoftLeafRef, p: vec3<f32>) -> f32 {
  return world_field(leaf.kind_id, world_at(leaf), p);
}
fn world_trace(o: vec3<f32>, d: vec3<f32>, limit: f32) -> PathHit {
  return world_trace_ray(0u, LoftRay(o, d, 0.0001, limit));
}

fn world_trace_ray(world: u32, ray: LoftRay) -> PathHit {
  return world_hit(world_secondary(world, ray, false), ray);
}

fn world_secondary(world: u32, ray: LoftRay, any_hit: bool) -> LoftBvhHit {
  return loft_bvh_trace_at(ray, world_root(world), any_hit);
}

fn world_hit(hit: LoftBvhHit, ray: LoftRay) -> PathHit {
  let o = ray.o;
  let d = ray.d;
  let nearest = hit.t;
  if nearest < 0.0 { return PathHit(nearest, vec3(0.0), vec3(0.0), 0u, 0u, false, 0u); }
  let kind = hit.leaf.kind_id;
  let at = world_at(hit.leaf);
  let p = o + d*nearest;
  var g = vec3<f32>(0.0);
  for (var j=0u; j<6u; j++) {
    let axis = j / 2u;
    let sign = select(-1.0, 1.0, (j&1u)==0u);
    var q = p;
    q[axis] += 0.0001 * sign;
    g[axis] += sign * world_field(kind, at, q);
  }
  if !(dot(g, g) > 0.0) { return PathHit(-2.0, p, vec3(0.0), kind, at, false, hit.leaf.row_id); }
  let entering = dot(g,d) < 0.0;
  let n = select(-normalize(g), normalize(g), entering);
  return PathHit(nearest,p,n,kind,at,entering,hit.leaf.row_id);
}

fn world_light(id: u32, point: vec3<f32>) -> PathLight {
  if id==0u { return PathLight(normalize(frame.sun.xyz), loft_params[0].xyz, 1e6); }
  let lamp = world_lamp(id);
  let v = lamp.position.xyz - point;
  let d = length(v);
  if d==0.0 || d>lamp.position.w { return PathLight(vec3<f32>(0.0,1.0,0.0), vec3<f32>(0.0), 0.0); }
  return PathLight(v/d, lamp.intensity.xyz/(d*d), d);
}
