fn loft_cellular2(p: vec2<f32>) -> vec4<f32> {
  let ix = loft_lat(p.x);
  let iy = loft_lat(p.y);
  let f = p - floor(p);
  var d1 = 8.0;
  var d2 = 8.0;
  var near = vec2<f32>(0.0);
  for (var j = -1; j <= 1; j++) {
    for (var i = -1; i <= 1; i++) {
      let h = loft_h2(ix + bitcast<u32>(i), iy + bitcast<u32>(j));
      let site = vec2<f32>(f32(i), f32(j)) + vec2<f32>(loft_unit(h), loft_unit(loft_pcg(h)));
      let r = site - f;
      let d = dot(r, r);
      // floor(p) + site, not p + r: the same bits at every pixel of the cell, so a hash of it is one colour.
      if (d < d1) { d2 = d1; d1 = d; near = floor(p) + site; } else if (d < d2) { d2 = d; }
    }
  }
  return vec4<f32>(sqrt(d1), sqrt(d2), near);
}