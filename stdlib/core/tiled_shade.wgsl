
@compute @workgroup_size(8, 8) fn shade(@builtin(workgroup_id) wg: vec3u, @builtin(local_invocation_id) lid: vec3u) {
  if frame.cold.x == LOFT_NONCE { return; }
  let tile = tiles[kp.kind_id * frame.size.w + wg.x];
  let px = vec2u(tile & 0xffffu, tile >> 16u) * 8u + lid.xy;
  if any(px >= frame.size.xy) { return; }
  let v = textureLoad(vis_tex, px, 0).xy;
  let center = LoftRay(frame.eye.xyz, loft_ray(vec2f(px) + 0.5), 0.0001, 1e6);
{{SHADE}}  textureStore(hdr, px, vec4f(color, 1.0));
}
{{TWIN_BINDINGS}}{{TWIN}}
