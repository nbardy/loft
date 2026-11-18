// Medium {K}, marched (draw.volume): `loft_tr{K}` is its transmittance from p along dir for at most `reach` in n
// samples; `loft_scatter{K}` its single scattering along a view span. Its density is the graph function `loft_d{K}`.
fn loft_density{K}(p: vec3<f32>) -> f32 { return max(f32(loft_d{K}(vec3<lf>(p))), 0.0); }

fn loft_tr{K}(p: vec3<f32>, dir: vec3<f32>, reach: f32, n: u32) -> f32 {
  let s = box_span(p, dir, MEDIUM{K}_LO, MEDIUM{K}_HI);
  let dt = max(min(s.y, reach) - s.x, 0.0) / f32(max(n, 1u));
  var tau = 0.0;
  for (var k = 0u; k < n; k++) { tau += loft_density{K}(p + dir * (s.x + (f32(k) + 0.5) * dt)); }
  return exp(-tau * dt);
}

fn loft_scatter{K}(r: Ray, s: vec2<f32>) -> vec4<f32> {
  let dt = (s.y - s.x) / f32(MEDIUM{K}_STEPS);
  var tr = 1.0;
  var c = vec3<f32>(0.0);
  for (var k = 0u; k < MEDIUM{K}_STEPS; k++) {
    let p = r.o + r.d * (s.x + (f32(k) + 0.5) * dt);
    let a = 1.0 - exp(-loft_density{K}(p) * dt);
    c += tr * a * paint(p, view.sun.xyz, k).rgb * loft_tr{K}(p, view.sun.xyz, 3.4e38, MEDIUM{K}_SHADOW);
    tr *= 1.0 - a;
  }
  return vec4<f32>(c, 1.0 - tr);
}
