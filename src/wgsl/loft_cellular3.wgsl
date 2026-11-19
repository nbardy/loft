fn loft_cellular3(p: vec3<f32>) -> vec2<f32> {
  let ix = loft_lat(p.x);
  let iy = loft_lat(p.y);
  let iz = loft_lat(p.z);
  let f = p - floor(p);
  var d1 = 8.0;
  var d2 = 8.0;
  for (var k = -1; k <= 1; k++) {
    for (var j = -1; j <= 1; j++) {
      for (var i = -1; i <= 1; i++) {
        let h = loft_h3(ix + bitcast<u32>(i), iy + bitcast<u32>(j), iz + bitcast<u32>(k));
        let h1 = loft_pcg(h);
        let r = vec3<f32>(f32(i), f32(j), f32(k)) + vec3<f32>(loft_unit(h), loft_unit(h1), loft_unit(loft_pcg(h1))) - f;
        let d = dot(r, r);
        if (d < d1) { d2 = d1; d1 = d; } else if (d < d2) { d2 = d; }
      }
    }
  }
  return vec2<f32>(sqrt(d1), sqrt(d2));
}