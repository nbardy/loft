// Every draw module's prelude (lean Rust plan §1.5): the interstage record and rays. Read with the graph module above
// it, whose emit prelude (loft-core emit/prelude.rs) holds the camera (`frame.view_proj`, `frame.inv_view_proj`,
// `frame.viewport`, `frame.sun`) at @group(0) @binding(0) and kernel rows at @group(1), and the glue the lowering
// writes below it (`proxy`, `veil`, the medium functions, @group(2) bindings).

struct Out {
  @builtin(position) pos: vec4<f32>,
  @location(0) world: vec3<f32>,
  @location(1) normal: vec3<f32>,
  @location(2) uv: vec2<f32>,
  @location(3) @interpolate(flat) id: u32,
}
struct Lit {
  @location(0) colour: vec4<f32>,
  @builtin(frag_depth) depth: f32,
}

fn to_clip(p: vec3<f32>) -> vec4<f32> { return frame.view_proj * vec4<f32>(p, 1.0); }
fn depth_of(p: vec3<f32>) -> f32 { let c = to_clip(p); return c.z / c.w; }
fn world_at(px: vec2<f32>, z: f32) -> vec3<f32> {
  let w = frame.inv_view_proj * vec4<f32>(px.x * frame.viewport.z * 2.0 - 1.0, 1.0 - px.y * frame.viewport.w * 2.0, z, 1.0);
  return w.xyz / w.w;
}

// The ray through pixel `px` from the near plane (so a perspective and an orthographic view are one case), and its width
// along t: `w.x + w.y * t` (TS `Ray.width`).
struct Ray { o: vec3<f32>, d: vec3<f32>, w: vec2<f32> }
fn ray_at(px: vec2<f32>) -> Ray {
  let o = world_at(px, 0.0);
  let d = normalize(world_at(px, 1.0) - o);
  let q = px + vec2<f32>(1.0, 0.0);
  let o1 = world_at(q, 0.0);
  return Ray(o, d, vec2<f32>(length(o1 - o), length(normalize(world_at(q, 1.0) - o1) - d)));
}

// Where o + t d is inside the box [lo, hi], from t = 0: (enter, leave); enter >= leave is a miss. An axis the ray is
// parallel to bounds nothing when o is within it and misses otherwise: the plain slab gives 0 / 0 = NaN for an origin
// on that face, and the NaN emptied the span (measured: a speck on the medium box's top face was not veiled).
fn box_span(o: vec3<f32>, d: vec3<f32>, lo: vec3<f32>, hi: vec3<f32>) -> vec2<f32> {
  let flat = d == vec3<f32>(0.0);
  if (any(flat & ((o < lo) | (o > hi)))) { return vec2<f32>(1.0, 0.0); }
  let a = select((lo - o) / d, vec3<f32>(-3.4e38), flat);
  let b = select((hi - o) / d, vec3<f32>(3.4e38), flat);
  let n = min(a, b);
  let f = max(a, b);
  return vec2<f32>(max(max(n.x, n.y), max(n.z, 0.0)), min(f.x, min(f.y, f.z)));
}

// The full-screen triangle (Screen covers and the composer).
fn screen_out(k: u32) -> Out {
  let uv = vec2<f32>(f32((k << 1u) & 2u), f32(k & 2u));
  return Out(vec4<f32>(uv.x * 2.0 - 1.0, 1.0 - uv.y * 2.0, 0.0, 1.0), vec3<f32>(0.0), vec3<f32>(0.0), uv, 0u);
}

// erf by Abramowitz and Stegun 7.1.26 (|error| < 1.5e-7), for the Gaussian media's closed form.
fn loft_erf(x: f32) -> f32 {
  let t = 1.0 / (1.0 + 0.3275911 * abs(x));
  let y = t * (0.254829592 + t * (-0.284496736 + t * (1.421413741 + t * (-1.453152027 + t * 1.061405429))));
  return sign(x) * (1.0 - y * exp(-x * x));
}
