fn loft_hash22(p: vec2<f32>) -> vec2<f32> {
  let h = loft_h2(loft_lat(p.x), loft_lat(p.y));
  return vec2<f32>(loft_unit(h), loft_unit(loft_pcg(h)));
}