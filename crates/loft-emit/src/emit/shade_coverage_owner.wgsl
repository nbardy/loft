  if v.x != LOFT_EMPTY && (v.x >> 24u) != kp.kind_id { return; }
  // Halo ownership as for taps: exactly one kind writes each pixel, sky included.
  let near = loft_vis_neighborhood(px, v.x);
  if near.x == LOFT_EMPTY || (near.x >> 24u) != kp.kind_id { return; }
  let loft_alpha = loft_coverage_alpha(px, v, near.y != 0u);
  if v.x == LOFT_EMPTY {
    if loft_alpha != 0.0 { textureStore(hdr, px, vec4<f32>(0.0, 0.0, 0.0, loft_alpha)); }
    return;
  }
