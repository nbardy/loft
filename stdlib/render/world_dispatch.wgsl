
fn world_local(kind: u32, at: u32, point: vec3<f32>) -> vec3<f32> {
  switch kind { {{LOCAL}} default: { return point; } }
}
fn world_field(kind: u32, at: u32, point: vec3<f32>) -> f32 {
  switch kind { {{FIELD}} default: { return 1e6; } }
}
fn loft_bvh_intersect(leaf: LoftLeafRef, ray: LoftRay) -> f32 {
  let at = world_at(leaf);
  let b = at + bitcast<u32>(world_word(2u + 2u*leaf.kind_id)) - 7u;
  let lo = vec3(world_word(b),world_word(b+1u),world_word(b+2u));
  let hi = vec3(world_word(b+3u),world_word(b+4u),world_word(b+5u));
  let span = loft_bvh_span(ray, lo, hi);
  if span.x > span.y { return -1.0; }
  switch leaf.kind_id { {{INTERSECT}} default: { return -2.0; } }
}
fn world_eval(h: PathHit, wo: vec3<f32>, wi: vec3<f32>) -> vec3<f32> {
  switch h.kind { {{EVAL}} default: { return vec3<f32>(0.0); } }
}
