fn loft_long_dir(p: vec3<f32>, n: vec3<f32>, s: f32) -> vec3<f32> {
  let px = dpdx(p);
  let py = dpdy(p);
  let det = dot(n, cross(px, py));
  let g = (cross(py, n) * dpdx(s) + cross(n, px) * dpdy(s)) / select(1.0, det, abs(det) > 1e-12);
  let l = length(g);
  // Its direction, weighted by how much of a shaft this is: the gradient's size wanders with every bulge of the skin
  // (and from one triangle to the next), and a threshold scaled by it stepped along the grid.
  return select(vec3<f32>(0.0), g / max(l, 1e-6) * smoothstep(0.25, 0.6, l), abs(det) > 1e-12 && l < 4.0);
}