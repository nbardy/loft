
/// A hash of an integer lattice point, uniform in [0, 1] (DESIGN.md: no randomness except hashes of indices).
fn sty_hash(p: vec2<i32>) -> f32 {
  var h = bitcast<u32>(p.x) * 1664525u + bitcast<u32>(p.y) * 1013904223u;
  h = (h ^ (h >> 16u)) * 2246822519u;
  h = (h ^ (h >> 13u)) * 3266489917u;
  return f32(h ^ (h >> 16u)) / 4294967295.0;
}
/// Value noise on the lattice of p, smooth (cubic), in [0, 1].
fn sty_noise(p: vec2<f32>) -> f32 {
  let i = vec2<i32>(floor(p));
  let f = fract(p);
  let u = f * f * (3.0 - 2.0 * f);
  let a = sty_hash(i);
  let b = sty_hash(i + vec2<i32>(1, 0));
  let c = sty_hash(i + vec2<i32>(0, 1));
  let d = sty_hash(i + vec2<i32>(1, 1));
  return mix(mix(a, b, u.x), mix(c, d, u.x), u.y);
}
/// Distance along the view direction to the opaque surface at px (both camera kinds); large where there is none.
fn sty_view_depth(px: vec2<f32>) -> f32 { return dot(scene_position(px) - frame.eye.xyz, frame.forward.xyz); }
/// Normal of the opaque surface at px, from neighbours s pixels away: of each pair, the one nearer in 3D, so a
/// neighbour across a silhouette does not bend it. Faces the eye.
fn sty_normal(px: vec2<f32>, s: f32) -> vec3<f32> {
  let p = scene_position(px);
  let xa = scene_position(px + vec2<f32>(s, 0.0)) - p;
  let xb = p - scene_position(px - vec2<f32>(s, 0.0));
  let ya = scene_position(px + vec2<f32>(0.0, s)) - p;
  let yb = p - scene_position(px - vec2<f32>(0.0, s));
  let dx = select(xb, xa, dot(xa, xa) < dot(xb, xb));
  let dy = select(yb, ya, dot(ya, ya) < dot(yb, yb));
  // A zero cross product (no surface change across the offsets) has no direction: report the eye's, not a NaN.
  let c = cross(dy, dx);
  let n = select(-frame.forward.xyz, normalize(c), dot(c, c) > 1e-20);
  let toward = select(-frame.forward.xyz, frame.eye.xyz - p, frame.eye.w > 0.5);
  return select(-n, n, dot(n, toward) > 0.0);
}
/// True where an opaque surface is drawn at px (the cleared depth is 1).
fn sty_solid(px: vec2<f32>) -> bool { return scene_depth(px) < 1.0; }
/// The view ray through pixel px: origin and unit direction (both camera kinds).
fn sty_ray(px: vec2<f32>) -> array<vec3<f32>, 2> {
  let ndc = vec2<f32>(px.x * frame.viewport.z * 2.0 - 1.0, 1.0 - px.y * frame.viewport.w * 2.0);
  let n = frame.inv_view_proj * vec4<f32>(ndc, 0.0, 1.0);
  let f = frame.inv_view_proj * vec4<f32>(ndc, 1.0, 1.0);
  let a = n.xyz / n.w;
  return array<vec3<f32>, 2>(a, normalize(f.xyz / f.w - a));
}
/// 1 where the pixel shows the sea (the ray meets the mean sea level before any opaque surface), else 0. The mean
/// level, not the swell: a post pass is per pixel, and the swell's height is small beside what this separates.
fn sty_water(px: vec2<f32>) -> f32 {
  let r = sty_ray(px);
  if (r[1].y > -1e-4) { return 0.0; }
  let t = -r[0].y / r[1].y;
  let hit = dot(r[0] + r[1] * t - frame.eye.xyz, frame.forward.xyz);
  return select(1.0, 0.0, sty_solid(px) && sty_view_depth(px) < hit);
}
/// True where the opaque surface at px is below the sea surface (hidden or tinted by the water drawn over it).
fn sty_submerged(px: vec2<f32>) -> bool {
  let p = scene_position(px);
  return sty_solid(px) && p.y < sea_height(p.xz) - 0.02;
}
