// A Screen cover (post and full-screen passes): paint gets (uv, the colour under's alpha) as p and its rgb as n.
@vertex fn vs(@builtin(vertex_index) k: u32) -> Out { return screen_out(k); }

@fragment fn fs(in: Out) -> @location(0) vec4<f32> {
  let u = textureLoad(loft_scene_colour, vec2<i32>(in.pos.xy), 0);
  return paint(vec3<f32>(in.uv, u.a), u.rgb, 0u);
}
