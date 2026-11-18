// A ring baked once into a 48³ distance texture by a compute pass, then drawn by `draw.field` reading that texture: the
// scene's camera, depth (the plane cuts it), light and outline. The bake is for speed; a check would read `shape.ring`,
// the source, which is still a graph function.
import { compute, draw, pbr, plane, scene, vec3, wgsl } from "../src/index.ts";

export const shape = wgsl`
fn ring(p: vec3f) -> f32 {
  let angle = atan2(p.z, p.x);
  let r = length(p.xz) - (0.51 + 0.07 * cos(5.0 * angle));
  return length(vec2(r, p.y + 0.10 * sin(3.0 * angle))) - 0.19;
}
@compute @workgroup_size(4, 4, 4)
fn fill(@builtin(global_invocation_id) id: vec3u) {
  // Texel centres: baked.sdf(uv) reads texel i at uv = (i + 0.5) / 48, so the field is stored there.
  textureStore(sdf, id, vec4(ring((vec3f(id) + 0.5) / 24.0 - 1.0), 0.0, 0.0, 1.0));
}`;

export const baked = compute("ring", {
  when: { kind: "build" },
  writes: { sdf: { kind: "texture", size: [48, 48, 48], format: "r32float" } },
  dispatch: [12, 12, 12],
  wgsl: shape,
});

const view = wgsl`fn field(p: vec3f) -> f32 { return ${baked.sdf}(p * 0.5 + 0.5).x; }`;

export default scene({
  ground: plane, style: pbr(), sun: [-1, 2, -1], compute: [baked],
  things: [draw.field("ring", view.field!, {
    bounds: { kind: "box", min: [-1, -1, -1], max: [1, 1, 1] }, steps: 64, safety: 0.8,
    paint: () => vec3(0.9, 0.45, 0.12), outline: "silhouette", schedule: "trace",
  })],
  cameras: { front: { kind: "perspective", eye: [0, 2, 5], target: [0, 0, 0], fovY: 38 * Math.PI / 180 } },
});
