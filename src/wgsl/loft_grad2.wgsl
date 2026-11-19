fn loft_grad2(h: u32) -> vec2<f32> {
  let a = loft_unit(h) * 6.283185307179586;
  return vec2<f32>(cos(a), sin(a));
}