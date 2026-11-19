
struct Ray { o: vec3<f32>, d: vec3<f32> };
fn camera_ray(px: vec2<f32>) -> Ray {
  let ndc = vec2<f32>(px.x * frame.viewport.z * 2.0 - 1.0, 1.0 - px.y * frame.viewport.w * 2.0);
  let near = frame.inv_view_proj * vec4<f32>(ndc, 0.0, 1.0);
  let far = frame.inv_view_proj * vec4<f32>(ndc, 1.0, 1.0);
  let o = near.xyz / near.w;
  return Ray(o, normalize(far.xyz / far.w - o));
}