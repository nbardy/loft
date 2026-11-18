// A Mesh cover: a triangle list in a row resource, two vec4 lanes per vertex (position, normal), read by `mesh_row`.
@vertex fn vs(@builtin(vertex_index) k: u32, @builtin(instance_index) i: u32) -> Out {
  let p = mesh_row(2u * k).xyz;
  return Out(to_clip(p), p, mesh_row(2u * k + 1u).xyz, vec2<f32>(0.0), i);
}
