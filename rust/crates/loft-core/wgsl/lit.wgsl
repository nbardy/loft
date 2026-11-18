// The fragment stage of every Raster cover. `uv` is the disc coordinate of a quad and 0 elsewhere, so the disc's edge
// is 1 on grids and meshes.
@fragment fn fs(in: Out) -> @location(0) vec4<f32> {
  let r = ray_at(in.pos.xy);
  let c = paint(in.world, normalize(in.normal), in.id);
  let edge = 1.0 - smoothstep(0.9, 1.0, length(in.uv));
  return vec4<f32>(c.rgb * veil(in.world, -r.d, dot(in.world - r.o, r.d)), c.a * edge);
}
