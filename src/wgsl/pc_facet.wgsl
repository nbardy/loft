
fn pc_hash3(c: vec3<i32>) -> vec3<f32> {
  var h = (bitcast<u32>(c.x) * 73856093u) ^ (bitcast<u32>(c.y) * 19349663u) ^ (bitcast<u32>(c.z) * 83492791u);
  h = (h ^ (h >> 16u)) * 2246822519u;
  let a = (h ^ (h >> 13u)) * 3266489917u;
  let b = (a ^ (a >> 16u)) * 2654435761u;
  let d = (b ^ (b >> 15u)) * 2246822519u;
  return vec3<f32>(f32(a >> 8u), f32(b >> 8u), f32(d >> 8u)) / 16777216.0;
}
/// Screen offsets (a, b) with a dpx + b dpy nearest to r, in least squares (0 where the pixel's footprint is degenerate).
fn pc_screen(r: vec3<f32>, dpx: vec3<f32>, dpy: vec3<f32>) -> vec2<f32> {
  let g = vec3<f32>(dot(dpx, dpx), dot(dpx, dpy), dot(dpy, dpy));
  let det = g.x * g.z - g.y * g.y;
  let rhs = vec2<f32>(dot(r, dpx), dot(r, dpy));
  return select(vec2<f32>(0.0), clamp(vec2<f32>(g.z * rhs.x - g.y * rhs.y, g.x * rhs.y - g.y * rhs.x) / det, vec2<f32>(-400.0), vec2<f32>(400.0)), abs(det) > 1e-20);
}
/// The lattice lives in the surface's anchor space a (so the facets ride a moving body); directions found there are
/// carried to the world through the screen: the same screen offset applied to the world point's derivatives.
fn pc_facet(n: vec3<f32>, p: vec3<f32>, a: vec3<f32>) -> vec4<f32> {
  let dpx = dpdx(p);
  let dpy = dpdy(p);
  let dax = dpdx(a);
  let day = dpdy(a);
  let dnx = dpdx(n);
  let dny = dpdy(n);
  let q = a / 0.55;
  let base = vec3<i32>(floor(q));
  var d1 = 1e9;
  var d2 = 1e9;
  var d3 = 1e9;
  var s1 = vec3<f32>(0.0);
  var s2 = vec3<f32>(0.0);
  var s3 = vec3<f32>(0.0);
  var id1 = vec3<i32>(0);
  var id2 = vec3<i32>(0);
  for (var k = -1; k <= 1; k++) {
    for (var j = -1; j <= 1; j++) {
      for (var i = -1; i <= 1; i++) {
        let c = base + vec3<i32>(i, j, k);
        let s = vec3<f32>(c) + 0.15 + 0.7 * pc_hash3(c);
        let d = dot(s - q, s - q);
        if (d < d1) { d3 = d2; s3 = s2; d2 = d1; s2 = s1; id2 = id1; d1 = d; s1 = s; id1 = c; }
        else if (d < d2) { d3 = d2; s3 = s2; d2 = d; s2 = s; id2 = c; }
        else if (d < d3) { d3 = d; s3 = s; }
      }
    }
  }
  // Distances to the bisectors (|q - b|^2 - |q - a|^2) / (2 |b - a|): the cell wall (sites 1, 2) and the spoke
  // between this fan triangle and the next (sites 2, 3).
  let wall = (d2 - d1) / (2.0 * max(length(s2 - s1), 1e-4)) * 0.55;
  let spoke = (d3 - d2) / (2.0 * max(length(s3 - s2), 1e-4)) * 0.55;
  // The surface normal carried to the site: the screen offset (a, b) that reaches it, then n + a dnx + b dny.
  let ab = pc_screen(s1 * 0.55 - a, dax, day);
  let carried = n + ab.x * dnx + ab.y * dny;
  // A carried normal that turned past the surface's own (a cell across a sharp rim) keeps the surface's.
  let flat = select(n, normalize(carried), dot(carried, n) > 0.3 * length(carried));
  // The fan triangle slopes toward its edge of the cell: up for a raised cell, down for a sunk one.
  let w = pc_screen(s2 - s1, dax, day);
  let edge = w.x * dpx + w.y * dpy;
  let toward = edge - flat * dot(edge, flat);
  let raised = select(-1.0, 1.0, pc_hash3(id1).x > 0.35);
  let slope = toward / max(length(toward), 1e-5) * (0.24 * raised);
  let jitter = (pc_hash3(id1 * 3 + id2 + vec3<i32>(17, 5, 11)) - 0.5) * 0.24;
  return vec4<f32>(normalize(flat + slope + jitter), min(wall, spoke));
}