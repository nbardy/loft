  let t = bitcast<f32>({{DISTANCE_BITS}});
  let p = frame.eye.xyz + rd * t;
  let pl = loft_xf_pt(xf, p);
  // Normal from the field (tetrahedron differences, i < 4), then occlusion along it (i >= 4), in the kind's space.
  // One loop, one call site: Metal inlines every call site of the field, and compile time follows (phase1.md).
  let e = 5e-4 * max(1.0, 0.25 * t);
  var g = vec3<f32>(0.0);
  var nl = vec3<f32>(0.0, 1.0, 0.0);
  var occ = 0.0;
  var w = 1.0;
  for (var i = 0u; i < 9u; i++) {
    var q = pl + LOFT_TETRA[min(i, 3u)] * e;
    var h = 0.0;
    if i >= 4u {
      if i == 4u { nl = normalize(g); }
      h = 0.02 + 0.06 * f32(i - 4u);
      q = pl + nl * h;
    }
    let d = q_trace(row, q);
    if i < 4u { g += LOFT_TETRA[i] * d; } else { occ += (h - d) * w; w *= 0.8; }
  }
  let ao = clamp(1.0 - 2.0 * occ, 0.0, 1.0);
  let n = normalize(loft_xf_vec(pose, nl));
  let b = q_material(row, pl, nl);
