  let id = gid;
  if any(id >= extent) { return; }
  let u = (vec3<f32>(id) + 0.5) / vec3<f32>(extent);
