fn loft_noise2(p: vec2<f32>) -> f32 {
  let ix = loft_lat(p.x);
  let iy = loft_lat(p.y);
  let f = p - floor(p);
  let u = f * f * f * (f * (f * 6.0 - 15.0) + 10.0);
  let a = dot(loft_grad2(loft_h2(ix, iy)), f);
  let b = dot(loft_grad2(loft_h2(ix + 1u, iy)), f - vec2<f32>(1.0, 0.0));
  let c = dot(loft_grad2(loft_h2(ix, iy + 1u)), f - vec2<f32>(0.0, 1.0));
  let d = dot(loft_grad2(loft_h2(ix + 1u, iy + 1u)), f - vec2<f32>(1.0, 1.0));
  return mix(mix(a, b, u.x), mix(c, d, u.x), u.y) * 1.4142135623730951;
}