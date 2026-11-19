// The one Ray template (plan §1.5): every tracer is the library's hit function (o, d, span, width) -> (normal, t),
// t < 0 a miss, called on the ray clipped to the proxy box. One colour entry and one depth entry (shadow maps).
fn trace(px: vec2<f32>, id: u32) -> vec4<f32> {
  let r = ray_at(px);
  let b = proxy(id);
  let s = box_span(r.o, r.d, b[0], b[1]);
  let h = vec4<f32>(loft_hit(vec3<lf>(r.o), vec3<lf>(r.d), vec2<lf>(s), vec2<lf>(r.w), lf(f32(id))));
  if (s.x >= s.y || h.w < 0.0) { discard; }
  return h;
}

@fragment fn fs(in: Out) -> Lit {
  let h = trace(in.pos.xy, in.id);
  let r = ray_at(in.pos.xy);
  let p = r.o + r.d * h.w;
  let c = paint(p, normalize(h.xyz), vec2<f32>(0.0), in.id);
  return Lit(vec4<f32>(c.rgb * veil(p, -r.d, h.w), c.a), depth_of(p));
}

@fragment fn fs_depth(in: Out) -> @builtin(frag_depth) f32 {
  let h = trace(in.pos.xy, in.id);
  let r = ray_at(in.pos.xy);
  return depth_of(r.o + r.d * h.w);
}
