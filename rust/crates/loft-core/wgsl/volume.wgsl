// A Ray cover that finds a medium: the view ray clipped to the proxy box and to the opaque scene's depth (a body inside
// the medium ends the integral at its skin), integrated by the medium (`scatter`, premultiplied rgb and alpha).
@fragment fn fs(in: Out) -> @location(0) vec4<f32> {
  let r = ray_at(in.pos.xy);
  let b = proxy(in.id);
  var s = box_span(r.o, r.d, b[0], b[1]);
  s.y = min(s.y, dot(world_at(in.pos.xy, scene_depth(in.pos.xy)) - r.o, r.d));
  if (s.x >= s.y) { discard; }
  return scatter(r, s);
}
