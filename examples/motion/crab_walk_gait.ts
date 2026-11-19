// The crab walking a turning arc by hand (panel a of agent_notes/2026-11-19_motion.md): islandback.ts's method. The
// gait plants the feet; the body rides the course frame at a fixed height, with a keyed bob (twice a cycle, as each
// tetrapod takes the weight) and roll (once a cycle) written as sines of the clock.
import { add, cos, mul, plane, scene, sin, time, toon } from "../../src/index.ts";
import { bodyFrame, CAMERAS, crab, S, walk } from "./lib/crab.ts";

const g = walk.body;
const bob = mul(cos(mul(time, (4 * Math.PI) / S.period)), 0.09), roll = mul(sin(mul(time, (2 * Math.PI) / S.period)), 0.04);
const yaw = mul(time, S.stride / S.period / S.radius);
export default scene({ ground: plane, things: crab(bodyFrame(add(g.o, mul(g.y, add(S.ride, bob))), yaw, roll), walk.foot), style: toon({ bands: 3, outline: 0.04 }), sun: [-0.45, 0.75, 0.5], cameras: CAMERAS });
