
@group(2) @binding(0) var loft_scene_depth: texture_2d<f32>;

/// Opaque scene depth at pixel px: 0 near to 1 far; 1 where no opaque surface is.
fn scene_depth(px: vec2<f32>) -> f32 {
  let size = vec2<i32>(textureDimensions(loft_scene_depth));
  return textureLoad(loft_scene_depth, clamp(vec2<i32>(floor(px)), vec2<i32>(0), size - 1), 0).x;
}
/// World position of the opaque surface seen at pixel px (on the far plane where there is none).
fn scene_position(px: vec2<f32>) -> vec3<f32> {
  let ndc = vec2<f32>(px.x * frame.viewport.z * 2.0 - 1.0, 1.0 - px.y * frame.viewport.w * 2.0);
  let h = frame.inv_view_proj * vec4<f32>(ndc, scene_depth(px), 1.0);
  return h.xyz / h.w;
}
