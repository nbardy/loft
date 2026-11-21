
    // Only border lanes read the halo; each corner also reads its diagonal.
    let local = g.xy & vec2u(7);
    let step = vec2i(select(select(0, 1, local.x == 7u), -1, local.x == 0u),
                     select(select(0, 1, local.y == 7u), -1, local.y == 0u));
    for (var i = 1u; i <= 3u; i++) {
      let delta = step * vec2i(i32(i & 1u), i32(i >> 1u));
      let q = vec2i(g.xy) + delta;
      if any(delta != vec2i(0)) && all(q >= vec2i(0)) && all(q < vec2i(frame.size.xy)) {
        let neighbor = textureLoad(vis_tex, q, 0).x;
        if neighbor != LOFT_EMPTY { atomicOr(&mask, 1u << (neighbor >> 24u)); }
      }
    }
