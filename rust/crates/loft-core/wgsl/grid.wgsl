// A Grid cover: copy i is `loft_cells(i)` cells of the place function (uv in [0, 1]^2, instance) -> position, two
// triangles per cell (TS surface.ts's order: u major), its normal the library's own function (a surface's closed form:
// central differences gave NaN at a tube's closed tips). The draw issues GRID_U x GRID_V cells, the most any level has;
// a vertex past the chosen level's cells (or of a hidden copy, 0 cells) is put outside the clip volume.
@vertex fn vs(@builtin(vertex_index) k: u32, @builtin(instance_index) i: u32) -> Out {
  let n = vec2<u32>(vec2<f32>(loft_cells(lf(f32(i)))));
  let cell = k / 6u;
  if (cell >= n.x * n.y) { return Out(vec4<f32>(0.0, 0.0, 2.0, 1.0), vec3<f32>(0.0), vec3<f32>(0.0, 1.0, 0.0), vec2<f32>(0.0), i); }
  let c = k % 6u; // (0,0) (1,0) (1,1) (0,0) (1,1) (0,1)
  let ij = vec2<u32>(cell / n.y, cell % n.y) + quad_corner(c);
  let uv = vec2<f32>(ij) / vec2<f32>(n);
  let p = vec3<f32>(loft_place(vec2<lf>(uv), lf(f32(i))));
  let nn = vec3<f32>(loft_normal(vec2<lf>(uv), lf(f32(i))));
  return Out(to_clip(p), p, nn, uv, i);
}
