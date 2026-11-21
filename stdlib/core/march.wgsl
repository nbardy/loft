  // Sphere trace with the row's derived Lipschitz bound (hidden column `loft_lip`).
  var t = max(span.x, 0.0);
  let step = 1.0 / row.loft_lip;
  const march_limit = 0u;
  const march_hit = 1u;
  const march_interval = 2u;
  var march_exit = march_limit;
{{STEPS_INIT}}  for (var i = 0u; i < 24u; i++) {
{{STEPS_4}}    let d = q_trace(row, lro + lrd * t);
    if d < 1e-4 + 2e-4 * t { march_exit = march_hit; break; }
    t += d * step;
    if t > span.y { march_exit = march_interval; break; }
  }
  if march_exit == march_limit {
    for (var i = 0u; i < 42u; i++) {
      {
{{STEPS_8}}        let d = q_trace(row, lro + lrd * t);
        if d < 1e-4 + 2e-4 * t { march_exit = march_hit; break; }
        t += d * step;
        if t > span.y { march_exit = march_interval; break; }
      }
      {
{{STEPS_8}}        let d = q_trace(row, lro + lrd * t);
        if d < 1e-4 + 2e-4 * t { march_exit = march_hit; break; }
        t += d * step;
        if t > span.y { march_exit = march_interval; break; }
      }
      {
{{STEPS_8}}        let d = q_trace(row, lro + lrd * t);
        if d < 1e-4 + 2e-4 * t { march_exit = march_hit; break; }
        t += d * step;
        if t > span.y { march_exit = march_interval; break; }
      }
      {
{{STEPS_8}}        let d = q_trace(row, lro + lrd * t);
        if d < 1e-4 + 2e-4 * t { march_exit = march_hit; break; }
        t += d * step;
        if t > span.y { march_exit = march_interval; break; }
      }
    }
  }
  if march_exit != march_hit { discard; }
