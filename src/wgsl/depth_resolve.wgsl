
@group(0) @binding(0) var depth_x4: texture_depth_multisampled_2d;
@vertex fn vs(@builtin(vertex_index) vi: u32) -> @builtin(position) vec4<f32> {
  let uv = vec2<f32>(f32((vi << 1u) & 2u), f32(vi & 2u));
  return vec4<f32>(uv.x * 2.0 - 1.0, 1.0 - uv.y * 2.0, 0.0, 1.0);
}
@fragment fn fs(@builtin(position) p: vec4<f32>) -> @location(0) vec4<f32> {
  let c = vec2<i32>(floor(p.xy));
  var d = textureLoad(depth_x4, c, 0);
  for (var i = 1; i < 4; i++) { d = min(d, textureLoad(depth_x4, c, i)); }
  return vec4<f32>(d, 0.0, 0.0, 1.0);
}
