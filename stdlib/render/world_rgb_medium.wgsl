
fn loft_medium(world: u32, p: vec3<f32>) -> loft_type_Sigma {
  let sigma = world_medium(world, p);
  return loft_type_Sigma(sigma.s, sigma.t, sigma.g, sigma.emission);
}
fn loft_authored_volume(world: u32, ray: loft_type_Ray, limit: f32, steps: u32, rng: array<u32, 4>) -> loft_type_Vol {
  let vol = world_volume(world, LoftRay(ray.o, ray.d, ray.t0, min(ray.t1, limit)), steps, true);
  return loft_type_Vol(vol.l, vol.t);
}
