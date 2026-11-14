// A long low walker at night whose stalk carries a lure: the lure lights its own face and the floor in front. No moon:
// the sun's amount is 0, so only the lure lights anything. The lure's place is written once; the flare shows it and
// the lamp lights from it (a glow lights nothing by itself).
import { add, flare, lamp, litBy, mul, plane, scene, sin, strand, time, toon, vec3, type F } from "../../src/index.ts";

/** The stalk, m 0 at the head to 1 at the lure: up and forward, bobbing. */
const stalk = (m: F) => vec3(add(0.3, mul(m, 1.4)), add(1.1, mul(sin(mul(m, 2.6)), 0.5)), mul(mul(m, m), mul(sin(mul(time, 1.7)), 0.3)));
const LURE = stalk(1 as unknown as F);

const body = strand({
  name: "body",
  spine: (m) => vec3(mul(m, -3), add(0.55, mul(sin(add(mul(m, 5), time)), 0.06)), 0),
  girth: (m) => add(0.08, mul(sin(mul(m, Math.PI)), 0.45)),
  tips: ["round", "point"],
  paint: [0.35, 0.3, 0.28],
});
export const lure = lamp(LURE, { colour: [0.5, 1.8, 1.3], reach: 1.2 });

export default scene({
  ground: plane,
  things: [
    body,
    strand({ name: "stalk", spine: stalk, girth: 0.03, tips: ["open", "point"], paint: [0.3, 0.27, 0.25] }),
    flare("glint", LURE, { size: 0.06, intensity: 3, rays: 0, halo: 5, colour: [0.3, 1, 0.75], core: [1, 1, 1], veil: { kind: "clear" } }),
  ],
  style: toon(),
  sun: { toward: [0.3, 0.8, -0.5], amount: [0, 0, 0] }, // no moon: only the lure lights anything
  lights: [lure],
  cameras: {
    whole: { kind: "perspective", eye: [5, 2.5, 6], target: [-0.5, 0.8, 0], fovY: 0.6 },
    face: { kind: "perspective", eye: [2.5, 1.2, 1.8], target: [0.3, 0.8, 0], fovY: 0.5 },
  },
});

// The head (m = 0, about 1.9 from the lure) receives at least 0.15 of the lure's light (luminance): predicted 0.31.
export const checks = [litBy(lure, body, { least: 0.15, from: 0, to: 0.1 })];
