  let pl = q_surface(row, a.x, a.y);
  {{CROSS}}
  let nl = normalize(cr);
  let nw = loft_xf_vec(pose, nl);
  let o0 = vec4<f32>(0.0);
  let o2 = vec4<f32>(loft_xf_pt(pose, pl), 0.0);
  let o3 = vec4<f32>(nw, 0.0);
  let bs = q_material(row, pl, nl, a.xy);
