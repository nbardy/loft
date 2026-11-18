// A heron wading: two long legs, ankles bent backward, walking a slow circle with its neck drawn into an S.
// Shows: `gait` (two feet, half a cycle apart), `limb` (two segments by inverse kinematics, bending backward),
// `carriage` and `footing` (src/geom/stance.ts: the body carried over planted feet).
// Ground: a calm sea stands in for the mud flat (`FLOOR` below is the one line to change when a scene has a floor).
//
// Theory, before running. A biped's duty is above a half, so both feet are down for (2 duty - 1) = 24% of a cycle and
// the body bobs twice a cycle: `carriage`'s walk (bob at 2u, sway at u) fits a biped as written. `limb` bends toward
// any side, so a heron's backward ankle is `bend: -forward`: fits. `gait` takes two feet and two phases: fits.
// Predicted fights: `footing` makes the re-plants (`replant`) and the stamps required, so a creature that only walks
// must invent an idle schedule it never uses; and nothing turns a `Carriage` into the body's frame, so every user
// writes the six rotations by hand (the crab's rig does, in rig.ts).
//
// Scored. gait, limb and carriage's walk fit as predicted (the leg check measured 0.849 of the span, predicted 0.85).
// footing and the carriage fought as predicted, and were opened (below). Not fixed: a heron that walks and then stands
// would hand its feet from the walk to `homes`, which are fixed numbers in the ground frame, not where the walk last
// planted them, so the feet would slide as `walk.on` falls; `homes` as expressions would let a caller pass the last plants.
import { add, carriage, carriedFrame, course, footing, gait, lift, mul, scene, plane, toon, vec3, type Frame } from "../../src/index.ts";
import { limbsReach, tolerance } from "../../src/check.ts";
import { heron, HOMES, LEGS } from "./lib/heron.ts";

const FLOOR = plane; // a mud flat
/** The ground frame raised by h: where the hips ride. */
const lifted = (g: Frame, h: number): Frame => ({ ...g, o: add(g.o, mul(g.y, h)) });
const H = { hip: 1.9, legs: LEGS, stride: 0.9, period: 1.6, duty: 0.62, lift: 0.28, radius: 3.2 };

// ---- the walk: two feet, half a cycle apart, on a circle ---------------------------------------------------------------

const walk = gait({
  feet: HOMES.map((h) => vec3(...h)), phase: [0, 0.5], course: course.arc([0, 0, 0], 0, H.radius),
  stride: H.stride, pace: { kind: "time", period: H.period }, duty: H.duty, lift: H.lift,
});
// The fight (before src/geom/stance.ts was opened): `footing` required `replant` and `stamps`, so this walker had to
// write an idle schedule it never runs:
//   stamps: [], replant: { schedule: [], loop: 8, step: { lead: 0.2, seconds: 0.4, settle: 0.3 }, height: 0,
//                          busy: () => lift(1), idleBelow: 0, unload: 0, dip: 0 },
// and turn the carriage into a frame with six `rotateAbout`s copied from the crab's rig. Both sections are now optional
// and `carriedFrame` is the six rotations, with the signs `Carriage` states.
const feet = footing({ ground: walk.body, homes: HOMES, walk: { gait: walk, on: lift<"f32">(1), secondsPerCycle: H.period } });
const C = carriage([
  { kind: "idle", loop: 8, seed: 3, breath: { rise: 0.02, period: 4 }, shift: { side: 0.02, fore: 0.02, hz: 0.3 }, tilt: { roll: 0.2, pitch: 0.2 }, turn: 0.03, unload: feet.unload },
  { kind: "walk", on: lift<"f32">(0.85), gait: walk, size: { bob: 0.06, sway: 0.05, roll: 0.05, yaw: 0.06 } },
]);
const body = carriedFrame(lifted(walk.body, H.hip), C);
// The bird (a teardrop body, an S neck, a dagger bill, legs by IK bent backward, toes) is in lib/heron.ts, shared with
// heron_leap.ts.
const bird = heron(body, (i) => feet.foot(i), walk.body);

export default scene({
  ground: FLOOR,
  things: bird.things,
  style: toon({ bands: 3, outline: 0.03 }),
  sun: [-0.4, 0.8, 0.45],
  cameras: {
    game: { kind: "perspective", eye: [5.5, 6.5, 5], target: [0, 0.9, -3.2], fovY: 0.6 },
    side: { kind: "perspective", eye: [0, 2.2, 9], target: [0, 1.6, -3.2], fovY: 0.6 },
  },
});

// Predicted: the most stretched leg is (1.75 + 0.06 bob) over a 0.28 half-stance, about 0.85 of its 2.15 span.
export const checks = [
  limbsReach(bird.legs.map((l, i) => ({ name: `leg${i}`, limb: l, outward: mul(body.x, -1) })), { period: H.period * 2, slack: tolerance.share(0.03), fold: tolerance.share(0.1) }),
];
