// A Grid cover: GRID_U x GRID_V cells of the place function (uv in [0, 1]^2, instance) -> position, two triangles per
// cell, the normal by central differences a quarter cell apart.
fn grid_at(uv: vec2<f32>, i: u32) -> vec3<f32> { return vec3<f32>(loft_place(vec2<lf>(uv), lf(f32(i)))); }

@vertex fn vs(@builtin(vertex_index) k: u32, @builtin(instance_index) i: u32) -> Out {
  let cell = k / 6u;
  let c = k % 6u; // (0,0) (1,0) (1,1) (0,0) (1,1) (0,1)
  let du = select(0u, 1u, c == 1u || c == 2u || c == 4u);
  let dv = select(0u, 1u, c == 2u || c == 4u || c == 5u);
  let uv = vec2<f32>(f32(cell % GRID_U + du), f32(cell / GRID_U + dv)) / vec2<f32>(f32(GRID_U), f32(GRID_V));
  let e = 0.25 / f32(max(GRID_U, GRID_V));
  let p = grid_at(uv, i);
  let tu = grid_at(uv + vec2<f32>(e, 0.0), i) - grid_at(uv - vec2<f32>(e, 0.0), i);
  let tv = grid_at(uv + vec2<f32>(0.0, e), i) - grid_at(uv - vec2<f32>(0.0, e), i);
  return Out(to_clip(p), p, normalize(cross(tu, tv)), vec2<f32>(0.0), i);
}
