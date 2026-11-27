// One workgroup per 8x8 tile that classify listed for this kind.
@compute @workgroup_size(8, 8) fn shade(@builtin(workgroup_id) wg: vec3<u32>, @builtin(local_invocation_id) lid: vec3<u32>) {
  if frame.cold.x == LOFT_NONCE { return; }
  let tile = tiles[kp.kind_id * frame.size.w + wg.x];
  let px = vec2<u32>(tile & 0xffffu, tile >> 16u) * 8u + lid.xy;
  if px.x >= frame.size.x || px.y >= frame.size.y { return; }
  let v = textureLoad(vis_tex, px, 0).xy;
{{OWNERSHIP}}  let row = load_row(v.x & 0xffffffu);
  let rd = loft_ray(vec2<f32>(px) + 0.5);
  let pose = loft_pose(row);
  let xf = loft_xf_inv(pose);
{{GEOMETRY}}  let sun = normalize(frame.sun.xyz);
  textureStore(hdr, px, vec4<f32>({{SHADE}}, {{ALPHA}}));
}
