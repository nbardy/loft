import { scene, thing, type Scene, type Thing } from "../../src/render/record.ts";
import { plane, capsule, box, bead, sphere } from "../../src/kinds/shapes.ts";
import { matte, metal, glow } from "../../src/kinds/materials.ts";
import { realtime } from "../../src/engines/realtime.ts";

export function slice(n: number, withBead = true): Scene {
  const ivory = matte([0.84, 0.77, 0.6]),
    copper = metal([0.92, 0.52, 0.26], 0.42);
  const things: Thing[] = [thing(plane(40), matte([0.27, 0.32, 0.34]))];
  for (let i = 0; i < 4; i++) things.push(thing(capsule(0.14, 0.37), ivory, [-1.6 + i * 0.65, 0.51, 1.2]));
  for (let i = 0; i < 2; i++)
    things.push(thing(box([0.38, 0.4 + 0.15 * i, 0.42]), ivory, [-1.2 + 2.5 * i, 0.4 + i * 0.15, -0.4]));
  if (withBead) things.push(thing(bead(0.68, 0.24, 0.03), copper, [0, 0.72, -0.3]));
  things.push(thing(sphere(0.23), glow([18, 11, 5]), [1.2, 2, 0]));
  for (let i = things.length; i < n; i++) {
    const k = i - 9,
      a = k * 2.39996323,
      r = 2.1 + Math.sqrt(Math.max(k, 0)) * 0.27;
    things.push(thing(sphere(0.16), ivory, [r * Math.cos(a), 0.16, r * Math.sin(a)]));
  }
  return {
    things,
    cameras: { front: { kind: "perspective", eye: [3, 2.5, 4.3], target: [0, 0.65, 0], fovY: 0.78 } },
    engine: realtime(),
  };
}
export default scene(slice(10));
