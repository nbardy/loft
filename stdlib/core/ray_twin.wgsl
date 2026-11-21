  let pl = loft_xf_pt(xf, a.xyz);
  let dl = normalize(loft_xf_vec(xf, d.xyz));
  let o0 = vec4<f32>(q_trace(row, pl), {{VISIBLE_TWIN}}, 0.0, 0.0);
  let o2 = vec4<f32>(0.0);
  let o3 = vec4<f32>(0.0);
  let nl = dl;
  let nw = normalize(d.xyz);
  let bs = q_material(row, pl, nl);
