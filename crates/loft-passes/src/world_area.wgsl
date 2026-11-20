fn world_sphere_light(id: u32, point: vec3f, xi: vec2f) -> PathLight {
  let lamp = world_lamps[id];
  let towards = lamp.position.xyz - point;
  let radius = -lamp.position.w;
  let sample = loft_sample_sphere(towards, radius, xi);
  let wi = sample.xyz;
  let along = dot(towards, wi);
  let perpendicular = cross(towards, wi);
  let root = sqrt(max(0.0, radius * radius - dot(perpendicular, perpendicular)));
  let inside = dot(towards, towards) <= radius * radius;
  let distance = select(along - root, along + root, inside);
  if distance <= 0.0002 { return PathLight(wi, vec3f(0.0), 0.0); }
  return PathLight(wi, lamp.intensity.xyz * sample.w, distance);
}
