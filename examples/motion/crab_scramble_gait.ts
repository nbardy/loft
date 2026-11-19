// The crab's scramble with today's gait (panel a of agent_notes/2026-11-19_motion.md, "Stepping from weight"): the body
// path of lib/scramble.ts, the feet from `gait` paced by the distance the body has moved along z. When the body
// reverses, the distance counts back and the gait's clock runs backward: the feet replay their steps in reverse.
import { course, gait, plane, scene, toon, vec3 } from "../../src/index.ts";
import { CAMERAS, crab, S } from "./lib/crab.ts";
import { body, z } from "./lib/scramble.ts";

const sides = [-1, 1] as const;
const walk = gait({
  feet: sides.flatMap((s) => S.legFeet.map(([x, fz]) => vec3(x, 0, s * fz))),
  phase: sides.flatMap((s) => S.legFeet.map((_, i) => ((i % 2 === 0) === (s < 0) ? 0 : 0.5) + 0.07 * i)),
  course: course.line([0, 0, 0], -Math.PI / 2, Math.PI / 2), stride: S.stride, pace: { kind: "distance", travelled: z }, duty: S.duty, lift: S.lift,
});
export default scene({ ground: plane, things: crab(body, walk.foot), style: toon({ bands: 3, outline: 0.04 }), sun: [-0.45, 0.75, 0.5], cameras: CAMERAS });
