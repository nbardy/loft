fn world_direct(h: PathHit, wo: vec3<f32>, seed: u32, bounce: u32) -> vec3<f32> {
  var r = vec4<f32>(0.0);
  let lights = arrayLength(&world_lamps);
  // Split independent proposals into adjacent reservoirs. Retargeting uses the
  // selected target and count, so completing their merge preserves total mass.
  let count = min(8u, lights);
  var previous = vec4<f32>(0.0);
  for (var i = 0u; i < count; i++) {
    let j = bounce*16u + 2u + i;
    let id = min(u32(path_random(seed ^ 0x80c910a9u, j)*f32(lights)), lights-1u);
    let lt = world_light(id, h.p);
    let value = world_eval(h, wo, lt.wi) * lt.l;
    let target_value = max(dot(value, vec3<f32>(0.2126,0.7152,0.0722)),0.0);
    let xi = path_random(seed ^ 0x71a77fc1u, j);
    if (i & 1u)==0u {
      r = {{RESERVOIR_ADD}}(r, id, target_value, target_value*f32(lights), 1.0, xi);
    } else {
      previous = {{RESERVOIR_ADD}}(previous, id, target_value, target_value*f32(lights), 1.0, xi);
    }
  }
  r = {{RESERVOIR_REUSE}}(r, previous, previous.y, path_random(seed ^ 0x0f6d2b79u, bounce));
  let scale = {{RESERVOIR_SCALE}}(r);
  if scale == 0.0 { return vec3<f32>(0.0); }
  let lt = world_light(u32(r.w), h.p);
  let start = h.p+h.n*0.0002;
  let vis = world_visibility(0u, LoftRay(start, lt.wi, 0.0001, lt.t-0.0002));
  return world_eval(h, wo, lt.wi) * lt.l * scale * vis;
}

fn path_aces(x: vec3<f32>) -> vec3<f32> {
  return clamp((x*(2.51*x+0.03))/(x*(2.43*x+0.59)+0.14), vec3<f32>(0.0),vec3<f32>(1.0));
}
