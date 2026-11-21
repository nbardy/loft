
  let center = textureLoad(vis_tex, px, 0).x;
  if center == LOFT_EMPTY { c = {{SKY}}(loft_ray(pos.xy), normalize(frame.sun.xyz)); }
  let size = vec2<i32>(textureDimensions(vis_tex));
  var line = 0.0;
  for (var i=0u; i<4u; i++) {
    let sign = select(-1, 1, (i & 1u) == 0u);
    let step = select(vec2<i32>(0,sign),vec2<i32>(sign,0),i<2u);
    let q = clamp(vec2<i32>(px)+step,vec2<i32>(0),size-1);
    if textureLoad(vis_tex,q,0).x!=center { line += 0.33; }
  }
  let hatch = smoothstep(0.74,0.98,cos((pos.x+pos.y)*1.256637)) * (1.0-smoothstep(0.08,0.32,dot(c,vec3<f32>(0.2126,0.7152,0.0722))));
  c = mix(c, vec3<f32>(0.025,0.02,0.035), min(line + 0.42*hatch,1.0));
  return vec4<f32>({{PAPER}}(loft_aces(c),pos.xy,0.04,0.03),1.0);
