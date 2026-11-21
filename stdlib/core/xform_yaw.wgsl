fn loft_xform_yaw(o: vec3<f32>, yaw: f32) -> LoftXform {
  let s = sin(yaw);
  let c = cos(yaw);
  return LoftXform(mat3x3<f32>(vec3<f32>(c, 0.0, -s), vec3<f32>(0.0, 1.0, 0.0), vec3<f32>(s, 0.0, c)), o);
}
