
/// A surface sample in the vertex stage: world position, unit normal, surface coordinates.
struct SurfV { pos: vec3<f32>, normal: vec3<f32>, uv: vec2<f32>, anchor: vec3<f32> };
/// A stable identity in [0, 1) for copy i of a drawable whose own is id (golden-ratio steps, so neighbours differ).
fn copy_id(id: f32, i: u32) -> f32 { return fract(id + f32(i) * 0.6180340); }
/// Grid corner offsets of the six vertices of a quad: triangles (0,0)(1,0)(1,1) and (0,0)(1,1)(0,1).
fn quad_corner(c: u32) -> vec2<u32> {
  return vec2<u32>(select(0u, 1u, c == 1u || c == 2u || c == 4u), select(0u, 1u, c == 2u || c == 4u || c == 5u));
}
/// World size of one pixel at p, across the view (exact for perspective and orthographic cameras alike).
fn pixel_world(p: vec3<f32>) -> f32 {
  let clip = frame.view_proj * vec4<f32>(p, 1.0);
  // Row 1 of view_proj is the projection's y scale times a unit row of the view rotation (perspective and
  // orthographic alike), so its length is that scale. One pixel at depth w is then 2 w / (height * scale).
  let fy = length(vec3<f32>(frame.view_proj[0][1], frame.view_proj[1][1], frame.view_proj[2][1]));
  return 2.0 * clip.w / (frame.viewport.y * fy);
}
/// The outline's offset: the style's world width, but never thinner than 1.5 pixels (so far views keep their ink); none
/// for a style whose width is 0. Friction F12 (2026-11-16): the 1.5 px floor applied at width 0 too, so papercraft (and
/// any spread with outline 0) put a 1.5 px rim on each side of every body, and a strand of 0.004 girth (under a pixel)
/// was drawn as a rod three or four pixels wide.
fn ink_width(p: vec3<f32>) -> f32 {
  return select(max(outline_world(), 1.5 * pixel_world(p)), 0.0, outline_world() <= 0.0);
}
/// Longest extent of one pixel's footprint on the sea at p: a pixel stretched by the grazing angle. Analytic, so it is
/// smooth across the sea grid's triangles. (It was fwidth of the interpolated position, which jumps from one large far
/// triangle to the next and drew their outlines into the water as light wedges.)
fn sea_footprint(p: vec3<f32>) -> f32 {
  return pixel_world(p) / max(abs(view_dir(p).y), 0.02);
}