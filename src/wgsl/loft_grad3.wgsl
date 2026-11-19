fn loft_grad3(h: u32) -> vec3<f32> {
  let z = 1.0 - 2.0 * loft_unit(h);
  let a = loft_unit(loft_pcg(h)) * 6.283185307179586;
  let r = sqrt((1.0 - z) * (1.0 + z));
  return vec3<f32>(r * cos(a), r * sin(a), z);
}