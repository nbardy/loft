// The pass composer's ink (TS src/draw/ink.ts INK_POST), the only place outline logic lives (plan §1.5). A pixel is ink
// where a surface q that asked for ink (alpha below 0.5) lies within q's ink width and nearer by more than a step.
// The graph function `loft_outline(colour at q, q) -> (ink rgb, ink width in world units)` is the style's.
@vertex fn vs(@builtin(vertex_index) k: u32) -> Out { return screen_out(k); }

fn ink_texel(px: vec2<f32>) -> vec2<i32> { return vec2<i32>(clamp(px, vec2<f32>(0.5), view.size.xy - 0.5)); }

@fragment fn fs(in: Out) -> @location(0) vec4<f32> {
  var dirs = array<vec2<f32>, 8>(vec2<f32>(1.0, 0.0), vec2<f32>(-1.0, 0.0), vec2<f32>(0.0, 1.0), vec2<f32>(0.0, -1.0),
    vec2<f32>(0.7071, 0.7071), vec2<f32>(-0.7071, -0.7071), vec2<f32>(0.7071, -0.7071), vec2<f32>(-0.7071, 0.7071));
  var radii = array<f32, 6>(1.0, 2.0, 3.0, 4.5, 6.0, 8.0);
  let px = in.pos.xy;
  let here = textureLoad(scene_colour, ink_texel(px), 0);
  let r = ray_at(px);
  let zh = textureLoad(scene_depth, ink_texel(px), 0);
  let z = select(3.4e38, dot(world_at(px, zh) - r.o, r.d), zh < 1.0);
  var cover = 0.0;
  var colour = vec3<f32>(0.0);
  for (var j = 0u; j < 6u; j++) {
    for (var k = 0u; k < 8u; k++) {
      let q = floor(px + dirs[k] * radii[j]) + 0.5;
      let dq = textureLoad(scene_depth, ink_texel(q), 0);
      let cq = textureLoad(scene_colour, ink_texel(q), 0);
      if (dq >= 1.0 || cq.a >= 0.5) { continue; }
      let rq = ray_at(q);
      let world = world_at(q, dq);
      let zq = dot(world - rq.o, rq.d);
      let ink = vec4<f32>(loft_outline(vec3<lf>(cq.rgb), vec3<lf>(world)));
      if (z - zq <= max(3.0 * ink.w, 0.01 * zq)) { continue; }
      let c = clamp(min(ink.w / (rq.w.x + rq.w.y * zq), 8.0) + 0.5 - radii[j], 0.0, 1.0);
      if (c > cover) {
        colour = ink.rgb;
        cover = c;
      }
    }
  }
  return vec4<f32>(mix(here.rgb, colour, cover), select((here.a - 0.51) / 0.49, here.a / 0.49, here.a < 0.5));
}
