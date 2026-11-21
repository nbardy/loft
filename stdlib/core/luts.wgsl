fn loft_lut(i: i32, off: u32, n: u32) -> LoftFrame {
  let o = (off + u32(clamp(i, 0, i32(n) - 1))) * 12u;
  return LoftFrame(vec3<f32>(luts[o], luts[o + 1u], luts[o + 2u]), vec3<f32>(luts[o + 3u], luts[o + 4u], luts[o + 5u]),
    vec3<f32>(luts[o + 6u], luts[o + 7u], luts[o + 8u]), vec3<f32>(luts[o + 9u], luts[o + 10u], luts[o + 11u]));
}
fn loft_blend(i: i32, off: u32, n: u32, w: vec4<f32>) -> LoftFrame {
  let a = loft_lut(i - 1, off, n);
  let b = loft_lut(i, off, n);
  let c = loft_lut(i + 1, off, n);
  let d = loft_lut(i + 2, off, n);
  return LoftFrame(a.origin * w.x + b.origin * w.y + c.origin * w.z + d.origin * w.w,
    a.tangent * w.x + b.tangent * w.y + c.tangent * w.z + d.tangent * w.w,
    a.normal * w.x + b.normal * w.y + c.normal * w.z + d.normal * w.w,
    a.binormal * w.x + b.binormal * w.y + c.binormal * w.z + d.binormal * w.w);
}
fn loft_sample(off: u32, n: u32, lo: f32, hi: f32, t: f32) -> LoftFrame {
  let s = (t - lo) / (hi - lo) * f32(n - 1u);
  let i = clamp(floor(s), 0.0, f32(n - 1u));
  let f = s - i;
  let g = 1.0 - f;
  return loft_blend(i32(i), off, n, vec4<f32>(g * g * g / 6.0, (3.0 * f * f * f - 6.0 * f * f + 4.0) / 6.0,
    (-3.0 * f * f * f + 3.0 * f * f + 3.0 * f + 1.0) / 6.0, f * f * f / 6.0));
}
fn loft_dsample(off: u32, n: u32, lo: f32, hi: f32, t: f32, dt: f32) -> LoftFrame {
  let s = (t - lo) / (hi - lo) * f32(n - 1u);
  let i = clamp(floor(s), 0.0, f32(n - 1u));
  let f = s - i;
  let g = 1.0 - f;
  let k = f32(n - 1u) / (hi - lo) * dt;
  return loft_blend(i32(i), off, n, vec4<f32>(-g * g / 2.0, (3.0 * f * f - 4.0 * f) / 2.0, (-3.0 * f * f + 2.0 * f + 1.0) / 2.0, f * f / 2.0) * k);
}
