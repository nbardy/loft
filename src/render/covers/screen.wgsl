@vertex fn screen(@builtin(vertex_index) i: u32) -> @builtin(position) vec4f {
  let uv=vec2f(f32((i<<1u)&2u),f32(i&2u));
  return vec4f(uv*vec2f(2,-2)+vec2f(-1,1),0,1);
}
