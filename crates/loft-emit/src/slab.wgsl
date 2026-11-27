fn loft_box_span(o: vec3f, inv_d: vec3f, parallel: vec3<bool>, lo: vec3f, hi: vec3f, interval: vec2f) -> vec2f {
  if any(select(vec3<bool>(false), o < lo, parallel)) || any(select(vec3<bool>(false), o > hi, parallel)) {
    return vec2f(1.0, -1.0);
  }
  let a = (lo - o) * inv_d;
  let b = (hi - o) * inv_d;
  let near = select(min(a, b), vec3f(interval.x), parallel);
  let far = select(max(a, b), vec3f(interval.y), parallel);
  return vec2f(max(interval.x, max(near.x, max(near.y, near.z))),
               min(interval.y, min(far.x, min(far.y, far.z))));
}
