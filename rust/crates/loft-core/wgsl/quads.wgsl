// A Quads cover: instance i is the place function (i) -> (centre, radius), a view-facing disc (specks, rain, glow,
// posed points). The view's axes are the clip matrix's first two rows, so perspective and orthographic are one case.
@vertex fn vs(@builtin(vertex_index) k: u32, @builtin(instance_index) i: u32) -> Out {
  var corners = array<vec2<f32>, 6>(vec2<f32>(-1.0, -1.0), vec2<f32>(1.0, -1.0), vec2<f32>(1.0, 1.0),
                                    vec2<f32>(-1.0, -1.0), vec2<f32>(1.0, 1.0), vec2<f32>(-1.0, 1.0));
  let c = corners[k];
  let q = vec4<f32>(loft_place(lf(f32(i))));
  let m = frame.view_proj;
  let right = normalize(vec3<f32>(m[0].x, m[1].x, m[2].x));
  let up = normalize(vec3<f32>(m[0].y, m[1].y, m[2].y));
  let p = q.xyz + (right * c.x + up * c.y) * q.w;
  return Out(to_clip(p), q.xyz, normalize(cross(right, up)), c, i);
}
