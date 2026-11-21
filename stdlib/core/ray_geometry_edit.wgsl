  let t = bitcast<f32>(v.y);
  let p = frame.eye.xyz + rd * t;
  let pl = loft_xf_pt(xf, p);
  let normal_ao = loft_edit_normal_ao(row, pl, t);
  let nl = normal_ao.xyz;
  let ao = normal_ao.w;
  let n = normalize(loft_xf_vec(pose, nl));
  let b = q_material(row, pl, nl);
