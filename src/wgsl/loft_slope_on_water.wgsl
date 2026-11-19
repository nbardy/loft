
@diagnostic(off, derivative_uniformity)
fn loft_slope_on_water(m: f32, xz: vec2<f32>) -> vec2<f32> {
  let a = dpdx(xz); let b = dpdy(xz);
  let det = a.x * b.y - a.y * b.x;
  let dm = vec2<f32>(dpdx(m), dpdy(m));
  return select(vec2<f32>(0.0), vec2<f32>(dm.x * b.y - dm.y * a.y, dm.y * a.x - dm.x * b.x) / det, abs(det) > 1e-12);
}