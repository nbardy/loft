
const INK_DIRS = array<vec2<f32>, 8>(vec2(1.0, 0.0), vec2(-1.0, 0.0), vec2(0.0, 1.0), vec2(0.0, -1.0),
  vec2(0.7071, 0.7071), vec2(-0.7071, -0.7071), vec2(0.7071, -0.7071), vec2(-0.7071, 0.7071));
const INK_RADII = array<f32, 6>(1.0, 2.0, 3.0, 4.5, 6.0, 8.0);
fn ink_view_depth(px: vec2<f32>) -> f32 { return dot(scene_position(px) - frame.eye.xyz, frame.forward.xyz); }
fn ink_decode(a: f32) -> f32 { return select((a - 0.51) / 0.49, a / 0.49, a < 0.5); }

@fragment fn fs(in: PostIn) -> @location(0) vec4<f32> {
  let px = in.px.xy;
  let here = post_color(in.uv);
  let z = select(1e30, ink_view_depth(px), scene_depth(px) < 1.0);
  var cover = 0.0;
  var colour = vec3<f32>(0.0);
  for (var r = 0u; r < 6u; r++) {
    let radius = INK_RADII[r];
    for (var k = 0u; k < 8u; k++) {
      let q = floor(px + INK_DIRS[k] * radius) + 0.5;
      if (scene_depth(q) >= 1.0) { continue; }
      let zq = ink_view_depth(q);
      let world = scene_position(q);
      let step = max(3.0 * outline_world(), 0.01 * zq);
      if (z - zq <= step) { continue; }
      let cq = textureLoad(loft_post_colour, vec2<i32>(q), 0);
      if (cq.a >= 0.5) { continue; }
      // The hull's width at q, in pixels, and how much of this pixel it covers.
      let width = min(ink_width(world) / pixel_world(world), 8.0);
      let c = clamp(width + 0.5 - radius, 0.0, 1.0);
      if (c > cover) {
        let depth = sea_height(world.xz) - world.y;
        colour = ink(cq.rgb, -frame.forward.xyz, -view_dir(world), world, depth, vec2<f32>(0.0), world, 0.0);
        cover = c;
      }
    }
  }
  return vec4<f32>(mix(here.rgb, colour, cover), ink_decode(here.a));
}