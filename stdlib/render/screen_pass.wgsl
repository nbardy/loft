
@vertex fn vs_present(@builtin(vertex_index) vi: u32) -> @builtin(position) vec4<f32> {
  let uv = vec2<f32>(f32((vi << 1u) & 2u), f32(vi & 2u));
  return vec4<f32>(uv * 2.0 - 1.0, 0.0, 1.0);
}
@fragment fn fs_present(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<f32> {
  if frame.cold.x == LOFT_NONCE { discard; }
  let samples = max(frame.cold.w, 1u);
  var sum = vec3<f32>(0.0);
  for (var i = 0u; i < samples; i++) {
    let seed = path_hash(u32(pos.x) + u32(pos.y)*frame.size.x) ^ path_hash(i);
    let jitter = vec2<f32>(path_random(seed, 0xfffffff1u), path_random(seed, 0xfffffff2u)) - 0.5;
    let primary = path_camera(pos.xy + jitter, seed);
    sum += {{ENTRY}}(primary.o, primary.d, seed, {{BOUNCES}}u);
  }
  return vec4<f32>({{CONVERT}}, 1.0);
}
