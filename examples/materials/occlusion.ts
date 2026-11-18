// A ceramic relief with two channels cut into its face. The cuts and their lips belong to one distance field.
// Swap the occlusion slot for () => f(1) to see the unchanged direct light and the open sky in the channels.
import { bound, budget, draw, max, min, nothing, pbr, scene, sub, vec3, type P3, type SceneSpec } from "../../src/index.ts";
import { sdBox3 } from "../../src/lib/index.ts";

const relief = (p: P3) => {
  const slab = sub(sdBox3(p, vec3(1.12, 0.7, 0.22)), 0.08);
  const cut = (x: number) => sub(sdBox3(sub(p, vec3(x, 0, 0.26)), vec3(0.14, 0.45, 0.19)), 0.025);
  return max(slab, sub(0, min(cut(-0.46), cut(0.46))));
};

export const spec = {
  ground: nothing,
  things: [draw.field("relief", relief, {
    bounds: bound.box([-1.22, -0.8, -0.32], [1.22, 0.8, 0.32]),
    paint: () => ({ base: vec3(0.89, 0.77, 0.64), rough: 0.82 }),
    steps: 76, outline: "none", schedule: "trace",
  })],
  sun: [-0.2, 0.8, -0.5],
  style: pbr(),
  cameras: {
    front: { kind: "orthographic", eye: [0, 0, 4], target: [0, 0, 0], height: 2.8 },
    three: { kind: "orthographic", eye: [1.9, 1.35, 5], target: [0, 0, 0], height: 2.6 },
  },
} satisfies SceneSpec;

export default scene(spec);
export const checks = [budget({ triangles: 13 })];
