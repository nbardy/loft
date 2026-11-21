
const LENS_RADIUS = 0.0;
const FOCUS = 1.0;
fn loft_tiled_node(id: u32) -> LoftBvhNode {
  let i = bitcast<u32>(luts[1]) + 8u * id;
  return LoftBvhNode(vec3f(luts[i], luts[i+1u], luts[i+2u]), bitcast<u32>(luts[i+3u]),
    vec3f(luts[i+4u], luts[i+5u], luts[i+6u]), bitcast<u32>(luts[i+7u]));
}
fn loft_tiled_leaf(id: u32) -> LoftLeafRef {
  let i = bitcast<u32>(luts[2]) + 2u * id;
  return LoftLeafRef(bitcast<u32>(luts[i]), bitcast<u32>(luts[i+1u]));
}
fn loft_tiled_lamp(id: u32) -> PathLamp {
  let i = bitcast<u32>(luts[3]) + 8u * id;
  return PathLamp(vec4f(luts[i], luts[i+1u], luts[i+2u], luts[i+3u]),
    vec4f(luts[i+4u], luts[i+5u], luts[i+6u], luts[i+7u]));
}
{{SOURCE}}
