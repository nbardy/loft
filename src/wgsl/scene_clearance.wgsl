
@group(2) @binding(1) var loft_scene_clearance: texture_2d<f32>;

/// Lower bound on the distance in pixels from pixel px to any opaque pixel (at most 49.0).
fn scene_clearance(px: vec2<f32>) -> f32 {
  let size = vec2<i32>(textureDimensions(loft_scene_depth));
  let c = textureLoad(loft_scene_clearance, clamp(vec2<i32>(floor(px)), vec2<i32>(0), size - 1) / 8, 0).x;
  return c * 64.0 - 0.13049019607843138;
}
