// Ranged checks on two closed-form motions that sampled frames get wrong (agent_notes/2026-11-18_ranged_checks.md):
// - a claw that sweeps through a shell between two 60 Hz frames (sampled frames pass; the ranged check fails);
// - a stone thrown at any speed in [4, 6] that touches down on a pad inside a 2 cm band no frame may hit (sampled
//   frames fail for most speeds; the ranged check holds).
//   bun src/cli/main.ts check examples/ranged.ts
import {
  abs, add, and, distance, ge, le, max, mul, param, plane, scene, smoothstep, sqrt, strand, sub, time, toon, vec3, type F,
} from "../src/index.ts";
import { always, eventually } from "../src/ranged.ts";

// ---- a claw through a shell -------------------------------------------------------------------------------------

/** The shell: a ball of radius R at (0, 1, 0). */
export const SHELL = { centre: [0, 1, 0] as const, radius: 0.35 };
/** The sweep starts 1/120 s after a frame, so it crosses the shell's centre half way between two frames at 60 Hz. */
const START = 0.4 + 1 / 120;
/** The claw's tip along x: from -4 to 4 in 0.2 s, at 60 units a second through the middle. */
const sweep = add(-4, mul(smoothstep(START, START + 0.2, time), 8));

const ball = (m: F, radius: number): F => mul(sqrt(max(sub(1, mul(sub(mul(m, 2), 1), sub(mul(m, 2), 1))), 0)), radius);

export const claw = strand({
  name: "claw",
  // A straight claw a unit long, its tip (m = 1) at x = sweep.
  spine: (m) => vec3(add(sweep, sub(m, 1)), 1, 0),
  stations: 8,
  girth: (m) => mul(sub(1.05, m), 0.12),
  paint: () => vec3(0.85, 0.3, 0.2),
});
const shell = strand({
  name: "shell",
  spine: (m) => vec3(mul(sub(m, 0.5), SHELL.radius * 2), SHELL.centre[1], 0),
  stations: 8,
  girth: (m) => ball(m, SHELL.radius),
  tips: "open",
  paint: () => vec3(0.3, 0.5, 0.8),
});

/** The claw's tip joint: its place and radius, as the skeleton measures it. */
const tip = claw.skeleton.joint(claw.skeleton.joints - 1);

// ---- a stone thrown onto a pad ----------------------------------------------------------------------------------

export const speed = param("stone.speed", 5, { min: 4, max: 6 });
const G = 9.81, AIM = Math.SQRT1_2;
/** The stone's middle: thrown from (0, 1, 0) at 45 degrees, ballistic. */
export const stone = vec3(mul(speed, time, AIM), sub(add(1, mul(speed, time, AIM)), mul(time, time, G / 2)), 0);
/** The pad: x from 2.3 to 4.5 on the ground. */
export const PAD = { middle: 3.4, half: 1.1 };

const pebble = strand({
  name: "stone",
  spine: (m) => add(stone, vec3(mul(sub(m, 0.5), 0.2), 0, 0)),
  stations: 6,
  girth: (m) => ball(m, 0.1),
  tips: "open",
  paint: () => vec3(0.5, 0.5, 0.52),
});
const pad = strand({
  name: "pad",
  spine: (m) => vec3(add(PAD.middle - PAD.half, mul(m, 2 * PAD.half)), 0.01, 0),
  stations: 4,
  girth: 0.6,
  section: { aspect: 0.05 },
  tips: "open",
  paint: () => vec3(0.9, 0.8, 0.3),
});

export default scene({
  ground: plane,
  things: [claw, shell, pebble, pad],
  style: toon({ bands: 3, outline: 0.04 }),
  sun: [-0.5, 0.8, 0.4],
  cameras: { three: { kind: "perspective", eye: [2, 4, 9], target: [1, 0.8, 0], fovY: 0.6 } },
});

export const checks = [
  // Fails: the tip passes through the shell's middle at t = 0.508333, between the frames at 0.5 and 0.516667.
  always([time, 0, 1], ge(distance(tip.p, SHELL.centre), add(SHELL.radius, tip.r)), { label: "claw tip stays out of the shell" }),
  // Holds: for every speed the stone comes down on the pad, within 2 cm of the ground at some instant.
  always([speed, 4, 6], eventually([time, 0, 2], and(le(abs(stone.y), 0.02), le(abs(sub(stone.x, PAD.middle)), PAD.half))), { label: "the stone lands on the pad at any speed" }),
];
