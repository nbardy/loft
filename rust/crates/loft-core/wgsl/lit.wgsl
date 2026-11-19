// The fragment stage of every Raster cover. `uv` is a quad's disc coordinate (its edge fades, DISC) or a grid's (u, v).
@fragment fn fs(in: Out, @builtin(front_facing) front: bool) -> @location(0) vec4<f32> {
  let r = ray_at(in.pos.xy);
  let n = select(1.0, -1.0, TWO_SIDED && !front) * normalize(in.normal);
  let c = paint(in.world, n, in.uv, in.id);
  let edge = select(1.0, 1.0 - smoothstep(0.9, 1.0, length(in.uv)), DISC);
  return vec4<f32>(c.rgb * veil(in.world, -r.d, dot(in.world - r.o, r.d)), c.a * edge);
}
