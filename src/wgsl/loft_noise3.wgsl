fn loft_noise3(p: vec3<f32>) -> f32 {
  let i = vec3<u32>(loft_lat(p.x), loft_lat(p.y), loft_lat(p.z));
  let f = p - floor(p);
  let u = f * f * f * (f * (f * 6.0 - 15.0) + 10.0);
  let c000 = loft_corner3(i, f);
  let c100 = loft_corner3(i + vec3<u32>(1u, 0u, 0u), f - vec3<f32>(1.0, 0.0, 0.0));
  let c010 = loft_corner3(i + vec3<u32>(0u, 1u, 0u), f - vec3<f32>(0.0, 1.0, 0.0));
  let c110 = loft_corner3(i + vec3<u32>(1u, 1u, 0u), f - vec3<f32>(1.0, 1.0, 0.0));
  let c001 = loft_corner3(i + vec3<u32>(0u, 0u, 1u), f - vec3<f32>(0.0, 0.0, 1.0));
  let c101 = loft_corner3(i + vec3<u32>(1u, 0u, 1u), f - vec3<f32>(1.0, 0.0, 1.0));
  let c011 = loft_corner3(i + vec3<u32>(0u, 1u, 1u), f - vec3<f32>(0.0, 1.0, 1.0));
  let c111 = loft_corner3(i + vec3<u32>(1u, 1u, 1u), f - vec3<f32>(1.0, 1.0, 1.0));
  let z0 = mix(mix(c000, c100, u.x), mix(c010, c110, u.x), u.y);
  let z1 = mix(mix(c001, c101, u.x), mix(c011, c111, u.x), u.y);
  return mix(z0, z1, u.z) * 1.1547005383792517;
}