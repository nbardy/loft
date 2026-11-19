// Medium {K} as Gaussians (draw.density), integrated in closed form. Row i of {ROWS}(k) is two lanes: (centre, sigma) and
// (peak extinction, 0, 0, 0), in the order its kernel sorted them (front to back). `n` is unused: the form is exact.
fn loft_tau{K}(i: u32, o: vec3<f32>, d: vec3<f32>, a: f32, b: f32) -> f32 {
  let g = {ROWS}(2u * i);
  let q = o - g.xyz;
  let m = dot(q, d);
  let s = g.w * 1.4142135;
  let e = exp(-(dot(q, q) - m * m) / (s * s));
  return {ROWS}(2u * i + 1u).x * e * g.w * 1.2533141 * (loft_erf((b + m) / s) - loft_erf((a + m) / s));
}

fn loft_tr{K}(p: vec3<f32>, dir: vec3<f32>, reach: f32, n: u32) -> f32 {
  var tau = 0.0;
  for (var i = 0u; i < {COUNT} / 2u; i++) { tau += loft_tau{K}(i, p, dir, 0.0, reach); }
  return exp(-tau);
}

fn loft_scatter{K}(r: Ray, s: vec2<f32>) -> vec4<f32> {
  var tr = 1.0;
  var c = vec3<f32>(0.0);
  for (var i = 0u; i < {COUNT} / 2u; i++) {
    let a = 1.0 - exp(-loft_tau{K}(i, r.o, r.d, s.x, s.y));
    let p = r.o + r.d * clamp(dot({ROWS}(2u * i).xyz - r.o, r.d), s.x, s.y);
    c += tr * a * paint(p, frame.sun.xyz, vec2<f32>(0.0), i).rgb * loft_tr{K}(p, frame.sun.xyz, 3.4e38, 0u);
    tr *= 1.0 - a;
  }
  return vec4<f32>(c, 1.0 - tr);
}
