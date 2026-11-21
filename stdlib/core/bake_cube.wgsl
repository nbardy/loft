
fn bake_direction(face: u32, uv: vec2<f32>) -> vec3<f32> {
  let h = uv * 2.0 - 1.0;
  var d: vec3<f32>;
  switch face {
    case 0u: { d = vec3<f32>( 1.0, -h.y, -h.x); }
    case 1u: { d = vec3<f32>(-1.0, -h.y,  h.x); }
    case 2u: { d = vec3<f32>( h.x,  1.0,  h.y); }
    case 3u: { d = vec3<f32>( h.x, -1.0, -h.y); }
    case 4u: { d = vec3<f32>( h.x, -h.y,  1.0); }
    default: { d = vec3<f32>(-h.x, -h.y, -1.0); }
  }
  return normalize(d);
}
