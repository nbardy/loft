@group(1) @binding(0) var hdr_tex: texture_2d<f32>;
@group(1) @binding(1) var vis_tex: texture_2d<u32>;

@vertex fn vs_present(@builtin(vertex_index) vi: u32) -> @builtin(position) vec4<f32> {
  let uv = vec2<f32>(f32((vi << 1u) & 2u), f32(vi & 2u));
  return vec4<f32>(uv * 2.0 - 1.0, 0.0, 1.0);
}

// ACES filmic fit (Narkowicz 2015); the target is sRGB, so the hardware encodes.
fn loft_aces(x: vec3<f32>) -> vec3<f32> {
  return clamp((x * (2.51 * x + 0.03)) / (x * (2.43 * x + 0.59) + 0.14), vec3<f32>(0.0), vec3<f32>(1.0));
}

@fragment fn fs_present(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<f32> {
  if frame.cold.x == LOFT_NONCE { discard; }
  let px = vec2<u32>(pos.xy);
  var c = textureLoad(hdr_tex, px, 0).rgb;
  {{RESOLVE}}
}
