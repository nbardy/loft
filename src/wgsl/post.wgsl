
@group(2) @binding(1) var loft_post_colour: texture_2d<f32>;
@group(2) @binding(2) var loft_post_sampler: sampler;

struct PostIn { @builtin(position) px: vec4<f32>, @location(0) uv: vec2<f32> };

@vertex fn post_vs(@builtin(vertex_index) vi: u32) -> PostIn {
  // (0,0), (2,0), (0,2) in uv: one triangle whose inside covers the whole viewport.
  let uv = vec2<f32>(f32((vi << 1u) & 2u), f32(vi & 2u));
  return PostIn(vec4<f32>(uv.x * 2.0 - 1.0, 1.0 - uv.y * 2.0, 0.0, 1.0), uv);
}
/// The picture so far at uv (0..1, v down), bilinearly filtered, clamped at the edges.
fn post_color(uv: vec2<f32>) -> vec4<f32> {
  return textureSampleLevel(loft_post_colour, loft_post_sampler, uv, 0.0);
}
