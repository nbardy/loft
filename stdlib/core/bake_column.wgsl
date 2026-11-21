  let x = (f32(gid.x) + 0.5) / f32(extent.x);
  let column = loft_bake_column(vec2f(x, 0.0));
  for (var dy = 0u; dy < {{ROWS_PER_INVOCATION}}u; dy++) {
    let id = vec3(gid.x, gid.y * {{ROWS_PER_INVOCATION}}u + dy, gid.z);
    if any(id >= extent) { break; }
    let u = vec3(x, (f32(id.y) + 0.5) / f32(extent.y), 0.5);
