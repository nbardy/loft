  let uv = vec2<f32>(f32(v.y & 0xffffu), f32(v.y >> 16u)) / 65535.0;
  let pl = q_surface(row, uv.x, uv.y);
  let cr = cross(loft_surface_jvp(row, uv.x, uv.y, 1.0, 0.0), loft_surface_jvp(row, uv.x, uv.y, 0.0, 1.0));
  let nl = cr * inverseSqrt(max(dot(cr, cr), 1e-30));
  let p = loft_xf_pt(pose, pl);
  var n = normalize(loft_xf_vec(pose, nl));
  // Two-sided: the inside of the aperture faces the camera too.
  if dot(n, rd) > 0.0 { n = -n; }
  let ao = 1.0;
  let b = q_material(row, pl, nl, uv);
