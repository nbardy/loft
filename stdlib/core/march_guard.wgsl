  // Restrict only the exit. The old raster interpolant and t sequence determine the hit bits.
  if frame.cold.y == 0u && t <= end {
    let g = bitcast<u32>(luts[{{HEADER}}u + kp.kind_id]) + 10u * v.row;
    let center = vec3f(luts[g+6u], luts[g+7u], luts[g+8u]);
    let rel = lro - center;
    let reach = luts[g+9u];
    let slack = 0.0001 + 0.000004 * (abs(end) + length(lro) + length(center));
    let last = rel + lrd * end;
    if length(rel) <= {{EYE_REACH}} && dot(lrd,lrd) >= 0.999 && all(abs(rel + lrd*t) < vec3f(reach - slack))
        && all(abs(last) < vec3f(reach - slack)) {
      let lo = vec3f(luts[g], luts[g+1u], luts[g+2u]) - slack;
      let hi = vec3f(luts[g+3u], luts[g+4u], luts[g+5u]) + slack;
      // Ignoring a parallel slab only retains extra work. Avoid 0 * infinity on its face.
      let parallel = abs(lrd) < vec3f(1e-15);
      let dir = select(lrd, vec3f(1.0), parallel);
      let a = (lo - lro) / dir;
      let b = (hi - lro) / dir;
      let n = select(min(a,b), vec3f(-1e30), parallel);
      let f = select(max(a,b), vec3f(1e30), parallel);
      end = min(end, min(f.x, min(f.y, f.z)) + slack);
      if max(t, max(n.x, max(n.y, n.z)) - slack) > end { discard; }
    }
  }
