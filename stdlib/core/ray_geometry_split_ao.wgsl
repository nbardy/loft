  let t = bitcast<f32>(v.y);
  let p = frame.eye.xyz + rd * t;
  let pl = loft_xf_pt(xf, p);
  let e = 5e-4 * max(1.0, 0.25 * t);
  var g = vec3<f32>(0.0);
  for (var i = 0u; i < 4u; i++) {
    let offset = LOFT_TETRA[i];
    g += offset * q_trace(row, pl + offset * e);
  }
  let nl = normalize(g);
  var occ = 0.0;
  var w = 1.0;
  for (var i = 0u; i < 5u; i++) {
    let h = 0.02 + 0.06 * f32(i);
    occ += (h - q_trace(row, pl + nl * h)) * w;
    w *= 0.8;
  }
  let ao = clamp(1.0 - 2.0 * occ, 0.0, 1.0);
  let n = normalize(loft_xf_vec(pose, nl));
  let b = q_material(row, pl, nl);
