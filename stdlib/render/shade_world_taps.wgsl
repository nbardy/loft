
  if v.x != LOFT_EMPTY && (v.x >> 24u) != kp.kind_id { return; }
  let near = loft_vis_neighborhood(px, v.x);
  if near.x == LOFT_EMPTY || (near.x >> 24u) != kp.kind_id { return; }
  let color = loft_shade_samples(center, vec2f(px), v, near.y != 0u);
