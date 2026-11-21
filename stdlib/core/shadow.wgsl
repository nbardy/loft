// The sun's shadow map, 3x3 bilinear comparisons; the point moves 1.5 texels along its normal first.
fn loft_shadow(p: vec3<f32>, n: vec3<f32>) -> f32 {
  if frame.cold.z == 0u { return 1.0; }
  let q = frame.light_vp * vec4<f32>(p + n * (1.5 * frame.sun.w), 1.0);
  let uv = vec2<f32>(q.x * 0.5 + 0.5, 0.5 - q.y * 0.5);
  if any(uv < vec2<f32>(0.0)) || any(uv > vec2<f32>(1.0)) || q.z > 1.0 || q.z < 0.0 { return 1.0; }
  let texel = 1.0 / f32(textureDimensions(shadow_tex).x);
  let depth = q.z - 0.0005;
  var s = 0.0;
  s += textureSampleCompareLevel(shadow_tex, shadow_smp, uv + vec2<f32>(-1.0, -1.0) * texel, depth);
  s += textureSampleCompareLevel(shadow_tex, shadow_smp, uv + vec2<f32>(-1.0, 0.0) * texel, depth);
  s += textureSampleCompareLevel(shadow_tex, shadow_smp, uv + vec2<f32>(-1.0, 1.0) * texel, depth);
  s += textureSampleCompareLevel(shadow_tex, shadow_smp, uv + vec2<f32>(0.0, -1.0) * texel, depth);
  s += textureSampleCompareLevel(shadow_tex, shadow_smp, uv + vec2<f32>(0.0, 0.0) * texel, depth);
  s += textureSampleCompareLevel(shadow_tex, shadow_smp, uv + vec2<f32>(0.0, 1.0) * texel, depth);
  s += textureSampleCompareLevel(shadow_tex, shadow_smp, uv + vec2<f32>(1.0, -1.0) * texel, depth);
  s += textureSampleCompareLevel(shadow_tex, shadow_smp, uv + vec2<f32>(1.0, 0.0) * texel, depth);
  s += textureSampleCompareLevel(shadow_tex, shadow_smp, uv + vec2<f32>(1.0, 1.0) * texel, depth);
  return s / 9.0;
}
