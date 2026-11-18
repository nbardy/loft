// A matte ceramic disc reflects an amber sky as the sun moves left, and a blue sky as it moves right.
// The sun stays behind the face, so the disc's colour comes from the projected environment.
import { add, bound, budget, cos, draw, mix, mul, nothing, pbr, saturate, scene, sub, time, vec3, type P3, type SceneSpec } from "../../src/index.ts";
import { sdCylinder } from "../../src/lib/index.ts";

const amber = vec3(0.56, 0.27, 0.09), blue = vec3(0.10, 0.24, 0.58);
const sky = (dir: P3, sun: P3) => mul(mix(amber, blue, saturate(add(0.5, sun.x))), add(0.7, mul(dir.z, 0.3)));

export const spec = {
  ground: nothing,
  things: [draw.field("disc", (p) => sub(sdCylinder(p, 0.88, 0.12), 0.08), {
    bounds: bound.box([-1.1, -1.1, -0.22], [1.1, 1.1, 0.22]),
    paint: () => ({ base: vec3(0.95, 0.95, 0.95), rough: 1 }),
    steps: 72, outline: "none", schedule: "trace",
  })],
  sun: vec3(mul(cos(mul(time, Math.PI)), -0.6), 0.65, -0.65),
  style: pbr({ sky, tone: (c) => c }),
  cameras: { front: { kind: "orthographic", eye: [0, 0, 4], target: [0, 0, 0], height: 2.5 } },
  period: 2,
} satisfies SceneSpec;

export default scene(spec);
export const checks = [budget({ triangles: 13 })];
