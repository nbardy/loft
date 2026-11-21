
  let delta = max(4.0 * e, 64.0 * 1.1920929e-7 * length(pl));
  var grad = vec3<f32>(0.0);
  for (var j = 0u; j < 6u; j++) {
    let axis = j / 2u;
    var q = pl;
    let side = select(-1.0, 1.0, (j & 1u) == 0u);
    q[axis] += side * delta;
    grad[axis] += side * q_trace(row, q);
  }
  let offset = grad * (delta * inverseSqrt(max(dot(grad, grad), 1e-30)));
  let a = pl - offset;
  let b0 = pl + offset;
  let fa = q_trace(row, a);
  let fb = q_trace(row, b0);
  let chord = distance(a, b0);
  // Subtract the endpoint uncertainty in field units. Compare the resulting
  // lower estimate at the original threshold; zero-length chords prove nothing.
  let extent = length(row.loft_hi - row.loft_lo);
  let roundoff = 8.0 * 1.1920929e-7 * (abs(fa) + abs(fb) + row.loft_lip * max(extent, length(pl)));
  let ratio = select(0.0, max(0.0, abs(fa - fb) - roundoff) / max(chord * row.loft_lip, 1e-30), chord > 0.0);
