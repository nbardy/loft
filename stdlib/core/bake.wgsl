
@group(3) @binding(0) var baked: texture_storage_{{STORAGE}}<{{FORMAT_NAME}}, write>;
@compute @workgroup_size({{WX}}, {{WY}}, {{WZ}})
fn bake(@builtin(global_invocation_id) gid: vec3<u32>) {
  let extent = vec3<u32>({{X}}u, {{Y}}u, {{Z}}u);
{{INVOCATION}}
  let texel = texel_value({{ARG}}{{COLUMN_ARG}});
  textureStore(baked, {{INDEX}}, {{ENCODING}});
{{END}}
}
