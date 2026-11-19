
fn loft_erfcp(z: f32) -> f32 {
  let t = 1.0 / (1.0 + 0.5 * z);
  return t * exp(-z * z - 1.26551223 + t * (1.00002368 + t * (0.37409196 + t * (0.09678418 + t * (-0.18628806 + t * (0.27886807 + t * (-1.13520398 + t * (1.48851587 + t * (-0.82215223 + t * 0.17087277)))))))));
}
fn loft_gseg(a: f32, b: f32) -> f32 {
  // erfc at |a| and |b| once each; the three cases (both ends above the centre, both below, astride) are differences
  // of those two of the same sign, so the tails keep their relative accuracy.
  let pa = loft_erfcp(abs(a));
  let pb = loft_erfcp(abs(b));
  return 0.8862269254527579 * select(select(2.0 - pa - pb, pb - pa, b <= 0.0), pa - pb, a >= 0.0);
}
fn loft_gtail(a: f32) -> f32 {
  let pa = loft_erfcp(abs(a));
  return 0.8862269254527579 * select(2.0 - pa, pa, a >= 0.0);
}
fn loft_q4(o: vec4<f32>, d: vec4<f32>, t: f32) -> f32 { let p = o + d * t; return dot(p, p); }
fn loft_gauss_ray(o: vec4<f32>, d: vec4<f32>, t0: f32, t1: f32) -> vec3<f32> {
  let L = max(t1 - t0, 0.0);
  let al = max(dot(d, d), 1e-20);
  let tm = -dot(o, d) / al;
  let s = sqrt(al);
  let spread = min(L * 0.288675, 0.707107 / s);
  if (s * L < 0.05) {
    let tmid = t0 + 0.5 * L;
    let w0 = exp(-loft_q4(o, d, t0));
    let wm = 4.0 * exp(-loft_q4(o, d, tmid));
    let w1 = exp(-loft_q4(o, d, t0 + L));
    let sum = w0 + wm + w1;
    let tc = select(tmid, (t0 * w0 + tmid * wm + (t0 + L) * w1) / sum, sum > 0.0);
    return vec3<f32>((L / 6.0) * sum, tc, spread);
  }
  let qmin = loft_q4(o, d, tm);
  let a = s * (t0 - tm);
  let b = s * (t0 + L - tm);
  let g = loft_gseg(a, b);
  let tc = select(clamp(tm, t0, t0 + L), tm + (exp(-a * a) - exp(-b * b)) / (2.0 * s * g), g > 1e-30);
  return vec3<f32>(exp(-qmin) / s * g, tc, spread);
}
fn loft_gauss_tail(o: vec3<f32>, d: vec3<f32>) -> f32 {
  let al = max(dot(d, d), 1e-12);
  let tm = -dot(o, d) / al;
  let s = sqrt(al);
  let p = o + d * tm;
  return exp(-dot(p, p)) / s * loft_gtail(-s * tm);
}
