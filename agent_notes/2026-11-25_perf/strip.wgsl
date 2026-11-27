// The screen projection of a segment hull lies between the projected supports
// of its endpoint balls. Reuse the hardware's two clip-depth planes for that
// strip. The fragment still writes the actual hit depth.
fn loft_thin_position({{ROW}}: Row, clip: vec4f) -> vec4f {
  let row = {{ROW}};
  if frame.cold.y != 0u { return clip; }
  let vp = frame.view_proj;
  let mx = vec3f(vp[0].x, vp[1].x, vp[2].x);
  let my = vec3f(vp[0].y, vp[1].y, vp[2].y);
  let mz = vec3f(vp[0].z, vp[1].z, vp[2].z);
  let mw = vec3f(vp[0].w, vp[1].w, vp[2].w);
  let extent = 0.5 * (row.loft_hi - row.loft_lo);
  let center = row.loft_lo + extent;
  let c = vp * vec4f(center, 1.0);
  // Every corner must already satisfy the near and far planes we replace.
  let eps = 0.00001 * (1.0 + abs(c.w) + length(center) + length(extent));
  if c.z <= dot(abs(mz), extent) + eps || c.w - c.z <= dot(abs(mw-mz), extent) + eps { return clip; }
  let pose = loft_pose(row);
  let a = loft_xf_pt(pose, {{A}});
  let b = loft_xf_pt(pose, {{B}});
  let ra = {{RA}};
  let rb = {{RB}};
  let ac = vp * vec4f(a, 1.0);
  let bc = vp * vec4f(b, 1.0);
  let wa = ac.w - ra * length(mw);
  let wb = bc.w - rb * length(mw);
  if min(ra, rb) < 0.0 || min(wa, wb) <= eps { return clip; }
  let d = bc.xy / bc.w - ac.xy / ac.w;
  let n = select(vec2f(1, 0), vec2f(-d.y, d.x) / max(length(d), 1e-12), length(d) > 1e-8);
  let na = dot(ac.xy, n) / ac.w;
  let nb = dot(bc.xy, n) / bc.w;
  let axis = n.x * mx + n.y * my;
  // For a displacement u inside a ball, delta(NDC) = dot(axis - NDC*mw, u) / (w + dot(mw,u)).
  let pa = (ra + eps) * length(axis - na * mw) / wa;
  let pb = (rb + eps) * length(axis - nb * mw) / wb;
  let pad = dot(abs(n), 4.0 / vec2f(frame.size.xy)) + 0.0001;
  let lo = min(na - pa, nb - pb) - pad;
  let hi = max(na + pa, nb + pb) + pad;
  let z = (dot(n, clip.xy) - lo * clip.w) / (hi - lo);
  return vec4f(clip.xy, z, clip.w);
}

