struct Frame {
  view_proj: mat4x4<f32>,
  inv_view_proj: mat4x4<f32>,
  eye: vec4<f32>,
  sun: vec4<f32>,
  size: vec4<u32>,
  cold: vec4<u32>,
  light_vp: mat4x4<f32>,
}
@group(0) @binding(0) var<uniform> frame: Frame;

@group(0) @binding(2) var<storage, read> loft_params: array<vec4<f32>>;
const LOFT_NONCE: u32 = {{NONCE}};
const LOFT_EMPTY: u32 = 0xffffffffu;

struct LoftXform { r: mat3x3<f32>, t: vec3<f32> }
fn loft_xf_pt(x: LoftXform, p: vec3<f32>) -> vec3<f32> { return x.r * p + x.t; }
fn loft_xf_vec(x: LoftXform, v: vec3<f32>) -> vec3<f32> { return x.r * v; }
fn loft_xf_inv(x: LoftXform) -> LoftXform { let rt = transpose(x.r); return LoftXform(rt, -(rt * x.t)); }
fn loft_xf_mul(a: LoftXform, b: LoftXform) -> LoftXform { return LoftXform(a.r * b.r, a.r * b.t + a.t); }
// Rotations of the plane (i, j): i' = cos i + sin j, j' = cos j - sin i (the evaluator's rule).
fn loft_rot_x(p: vec3<f32>, a: f32) -> vec3<f32> {
  let s = sin(a);
  let c = cos(a);
  return vec3<f32>(p.x, c * p.y + s * p.z, c * p.z - s * p.y);
}
fn loft_rot_y(p: vec3<f32>, a: f32) -> vec3<f32> {
  let s = sin(a);
  let c = cos(a);
  return vec3<f32>(c * p.x + s * p.z, p.y, c * p.z - s * p.x);
}
fn loft_rot_z(p: vec3<f32>, a: f32) -> vec3<f32> {
  let s = sin(a);
  let c = cos(a);
  return vec3<f32>(c * p.x + s * p.y, c * p.y - s * p.x, p.z);
}
fn loft_unit(e0: f32, e1: f32, x: f32) -> f32 { return clamp((x - e0) / (e1 - e0), 0.0, 1.0); }
fn loft_smootherstep(e0: f32, e1: f32, x: f32) -> f32 { let t = loft_unit(e0, e1, x); return t * t * t * ((6.0 * t - 15.0) * t + 10.0); }
fn loft_dsmoothstep(e0: f32, e1: f32, x: f32) -> f32 { let t = loft_unit(e0, e1, x); return 6.0 * t * (1.0 - t) / (e1 - e0); }
fn loft_dsmootherstep(e0: f32, e1: f32, x: f32) -> f32 { let t = loft_unit(e0, e1, x); return 30.0 * t * t * (1.0 - t) * (1.0 - t) / (e1 - e0); }
struct LoftFrame { origin: vec3<f32>, tangent: vec3<f32>, normal: vec3<f32>, binormal: vec3<f32> }
// The camera ray through a pixel position (pixel units, centre at +0.5).
fn loft_ray(px: vec2<f32>) -> vec3<f32> {
  let ndc = vec2<f32>(px.x / f32(frame.size.x) * 2.0 - 1.0, 1.0 - px.y / f32(frame.size.y) * 2.0);
  let far = frame.inv_view_proj * vec4<f32>(ndc, 1.0, 1.0);
  return normalize(far.xyz / far.w - frame.eye.xyz);
}
