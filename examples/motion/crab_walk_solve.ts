// The crab walking a turning arc, its body solved (panel b of agent_notes/2026-11-19_motion.md). No solved crab existed;
// this is one with the existing `solve` API. The footing is the gait's (the same planted feet and swings as
// crab_walk_gait.ts); the body's carriage (x, height y, z, yaw) is four B-splines over the 4 s that meet: the course's
// pose and speed at 0 s and at 4 s, and every leg within 0.5 to 0.95 of its span of its foot at every time; with the
// least jerk. No bob or roll is keyed or asked for: whatever the body does between the ends is the solve's.
import { add, always, equal, ge, length, plane, scene, solve, spline, sub, time, toon, vec3, vec4 } from "../../src/index.ts";
import { derivative } from "../../src/grad.ts";
import { rotate } from "../../src/map/chart.ts";
import { bodyFrame, CAMERAS, crab, legRoot, onCourse, S, SPAN, walk } from "./lib/crab.ts";

const T = S.seconds, a = onCourse(0), b = onCourse(T);
const solved = solve({
  over: [0, T],
  vary: { x: spline(14, 0), y: spline(14, S.ride), z: spline(14, 0), yaw: spline(14, 0) },
  hold: (c) => {
    const body = bodyFrame(vec3(c.x, c.y, c.z), c.yaw);
    const speed = vec4(derivative(c.x, time), derivative(c.y, time), derivative(c.z, time), derivative(c.yaw, time));
    const d = (k: number) => length(sub(walk.foot(k), add(body.o, rotate(body, vec3(...legRoot(k))))));
    return [
      always([time, 0, 0], equal(vec4(c.x, c.y, c.z, c.yaw), [a.x, S.ride, a.z, a.yaw])),
      always([time, 0, 0], equal(speed, [a.vx, 0, a.vz, a.vyaw])),
      always([time, T, T], equal(vec4(c.x, c.y, c.z, c.yaw), [b.x, S.ride, b.z, b.yaw])),
      always([time, T, T], equal(speed, [b.vx, 0, b.vz, b.vyaw])),
      ...[0, 1, 2, 3, 4, 5, 6, 7].flatMap((k) => [ge(0.95 * SPAN, d(k)), ge(d(k), 0.5 * SPAN)]),
    ];
  },
  prefer: { jerk: 1 },
});
console.error(`crab solve: ${solved.report.ms.toFixed(0)} ms, ${solved.report.steps} steps, ${solved.report.rounds} rounds`);
const c = solved.curves;
export default scene({ ground: plane, things: crab(bodyFrame(vec3(c.x, c.y, c.z), c.yaw), walk.foot), style: toon({ bands: 3, outline: 0.04 }), sun: [-0.45, 0.75, 0.5], cameras: CAMERAS });
export { solved };
