
  if v.x == LOFT_EMPTY || (v.x >> 24u) != kp.kind_id { return; }
  let color = loft_shade_hit(LoftLeafRef(kp.kind_id, v.x & 0xffffffu), center, bitcast<f32>(v.y), -center.d, vec2f(px));
