
struct Frame {
  view_proj: mat4x4<f32>,
  inv_view_proj: mat4x4<f32>,
  eye: vec4<f32>,       // xyz: eye position in world space. w: 1 perspective, 0 orthographic
  forward: vec4<f32>,   // xyz: unit view direction (the ray direction for an orthographic camera)
  viewport: vec4<f32>,  // width, height, 1 / width, 1 / height, in pixels
  time: vec4<f32>,      // x: seconds
};
@group(0) @binding(0) var<uniform> frame: Frame;
@group(0) @binding(1) var<storage, read> inputs: array<f32>;        // host inputs, by slot
@group(0) @binding(2) var<storage, read> params: array<f32>;        // tunable numbers, by slot
@group(0) @binding(3) var<storage, read> curves: array<vec4<f32>>;  // rows of CURVE_N samples
@group(0) @binding(4) var<uniform> loft_detail: array<vec4<f32>, 4096>;  // per-frame level of detail (Draw.detail)

const CURVE_N: u32 = 64u;
/// Sample curve row `r` at x in [0, 1]: linear interpolation, clamped at the ends.
fn curve(r: u32, x: f32) -> vec4<f32> {
  let s = clamp(x, 0.0, 1.0) * f32(CURVE_N - 1u);
  let i = min(u32(floor(s)), CURVE_N - 2u);
  return mix(curves[r * CURVE_N + i], curves[r * CURVE_N + i + 1u], s - f32(i));
}
/// Unit direction from the eye toward a world point (the camera's forward direction when orthographic).
fn view_dir(world: vec3<f32>) -> vec3<f32> {
  return normalize(mix(frame.forward.xyz, world - frame.eye.xyz, frame.eye.w));
}
