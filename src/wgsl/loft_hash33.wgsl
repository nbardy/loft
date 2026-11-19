fn loft_hash33(p: vec3<f32>) -> vec3<f32> {
  let h = loft_h3(loft_lat(p.x), loft_lat(p.y), loft_lat(p.z));
  let h1 = loft_pcg(h);
  return vec3<f32>(loft_unit(h), loft_unit(h1), loft_unit(loft_pcg(h1)));
}