
@compute @workgroup_size({{X}}, {{Y}}, {{Z}})
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
  loft_gid = gid;
  loft_kernel({{ARGS}});
}
