
// A halo pixel belongs to the first covered neighbor. Exactly one kind writes
// any pixel, including where different kinds meet or a tile crosses the edge.
fn loft_vis_neighborhood(px: vec2u, center: u32) -> vec2u {
  let limit = vec2i(frame.size.xy) - 1;
  var owner = center;
  var edge = 0u;
  for (var y = -1; y <= 1; y++) {
    for (var x = -1; x <= 1; x++) {
      let key = textureLoad(vis_tex, clamp(vec2i(px) + vec2i(x, y), vec2i(0), limit), 0).x;
      if key != center { edge = 1u; }
      if owner == LOFT_EMPTY { owner = key; }
    }
  }
  return vec2u(owner, edge);
}
