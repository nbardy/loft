//! WGSL for the portable mesh draw and traversal.
pub(crate) fn draw() -> (String, String, String) {
    (
            concat!(include_str!("../slab.wgsl"), include_str!("../mesh.wgsl")).to_string(),
            "  let base = bitcast<u32>(luts[row.r_asset * 8u]);
  let a = mesh_position(base, 3u * v.y);
  let ab = mesh_position(base, 3u * v.y + 1u) - a;
  let ac = mesh_position(base, 3u * v.y + 2u) - a;
  let local_n = normalize(cross(ab, ac));
  let raw_n = loft_xf_vec(pose, local_n);
  let n = select(raw_n, -raw_n, dot(raw_n, rd) > 0.0);
  let t = dot(loft_xf_pt(pose, a) - frame.eye.xyz, n) / dot(rd, n);
  let p = frame.eye.xyz + t * rd;
  let b = mesh_material(row, v.y);
  let ao = 1.0;
".into(),
            "  let ro = loft_xf_pt(xf, a.xyz);
  let rd = normalize(loft_xf_vec(xf, d.xyz));
  let hit = mesh_intersect(row, ro, rd, vec2f(0.0, 1e30), false);
  let o0 = vec4f(0.0, hit.t, 0.0, 0.0);
  let o2 = vec4f(0.0);
  let o3 = vec4f(0.0);
  let nw = normalize(d.xyz);
  let bs = mesh_material(row, select(0u, hit.triangle, hit.t >= 0.0));
".into(),
        )
}
