let sun = normalize(frame.sun.xyz);
  if hdr_px.a <= 0.0 { c = {{SKY}}(loft_ray(pos.xy), sun); }
  let code = abs(hdr_px.a) - 2.0;
  if code >= 0.0 {
    var steps = array(vec2<i32>(1, 0), vec2<i32>(-1, 0), vec2<i32>(0, 1), vec2<i32>(0, -1));
    let q = clamp(vec2<i32>(px) + steps[min(u32(code), 3u)], vec2<i32>(0), vec2<i32>(frame.size.xy) - 1);
    let other = textureLoad(hdr_tex, q, 0);
    var b = other.rgb;
    if other.a <= 0.0 { b = {{SKY}}(loft_ray(vec2<f32>(q) + 0.5), sun); }
    c = mix(c, b, fract(code));
  }
  return vec4<f32>({{FILM}}(c), 1.0);