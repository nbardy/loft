{{HEAD}}{{WORLD}}
{{LIBRARY}}
fn loft_eval_hit(hit: loft_type_Hit, wo: vec3f, wi: vec3f) -> vec3f {
  let leaf = LoftLeafRef(hit.kind_id, hit.row_id);
  let n = hit.n;
  return world_eval(PathHit(hit.t, hit.p, n, hit.kind_id, world_at(leaf), true, hit.row_id), wo, wi);
}
fn loft_shade_hit(leaf: LoftLeafRef, ray: LoftRay, t: f32, wo: vec3f, px: vec2f) -> vec3f {
  let at = world_at(leaf);
  let p = ray.o + ray.d * t;
  var g = vec3f(0.0);
  // Keep both taps of each axis together for the primary surface normal.
  for (var axis = 0u; axis < 3u; axis++) {
    var e = vec3f(0.0);
    e[axis] = 0.0001;
    g[axis] = world_field(leaf.kind_id, at, p + e) - world_field(leaf.kind_id, at, p - e);
  }
  if !(dot(g, g) > 0.0) { return vec3f(0.0); }
  let n = normalize(select(-g, g, dot(g, ray.d) < 0.0));
  let hit = loft_type_Hit(t, p, n, vec2f(0.0), world_local(leaf.kind_id, at, p),
    leaf.kind_id, leaf.row_id, vec4f(0.0));
  return {{CALLBACK}}(0u, hit, wo, false, px);
}
