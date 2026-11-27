@group(3) @binding(0) var<storage, read> twin_in: array<vec4<f32>>;
@group(3) @binding(1) var<storage, read_write> twin_out: array<vec4<f32>>;

// The twin gate: the frame's queries at CPU-chosen samples; the material is lit by one fixed light plus emission.
@compute @workgroup_size(64) fn twin(@builtin(global_invocation_id) g: vec3<u32>) {
  let i = g.x;
  if i >= arrayLength(&twin_out) / 4u { return; }
  let a = twin_in[2u * i];
  let d = twin_in[2u * i + 1u];
  let row = load_row(bitcast<u32>(a.w));
  let pose = loft_pose(row);
  let xf = loft_xf_inv(pose);
{{TWIN_GEO}}  // Lit 60 degrees off the normal, away from any specular peak (where a 1e-7 change moves D by 1e-3).
  let lt = nw * 0.5 + normalize(cross(nw, vec3<f32>({{L0}}, {{L1}}, {{L2}}))) * 0.8660254;
  let m = eval_bsdf(bs, nw, nw, lt) + eval_bsdf(bs, nw, nw, vec3<f32>(0.0));
  twin_out[4u * i] = o0;
  twin_out[4u * i + 1u] = vec4<f32>(m, 0.0);
  twin_out[4u * i + 2u] = o2;
  twin_out[4u * i + 3u] = o3;
}
