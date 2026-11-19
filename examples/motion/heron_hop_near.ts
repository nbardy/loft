// The heron's hop, solved with the hand keys as the prior (panel c of agent_notes/2026-11-19_motion.md). Existing
// `solve` API only: `prefer.near` the keys of heron_hop_keys.ts plus a whisper of jerk, so the keys say how it moves and
// the holds add only what they need (the flight's parabola, planted feet, reach, rail clearance). The head has its own
// unknown (the neck's turn, `headFrame`) held level in the world, and the body's pitch is bounded to 20 degrees.
import { add, always, clamp, div, dot, ge, le, length, lift, max, mul, poseFrame, pose, solve, spline, sub, through, time, vec2, vec3, type F, type P3 } from "../../src/index.ts";
import { derivative } from "../../src/grad.ts";
import { chain } from "../../src/geom/limb.ts";
import { equal, type Family } from "../../src/geom/solve.ts";
import { headFrame, heron, hipOf, HOMES, inBody, LEGS } from "../second/lib/heron.ts";
import { NUMBERS, stage } from "../second/heron_leap.ts";
import { KEYS } from "./heron_hop_keys.ts";

const HIP = 1.9, SPAN = LEGS[0] + LEGS[1], T = { off: 0.6, land: 1.2, end: 1.8 }, G = 9.81, n = NUMBERS;
/** Weights: near the keys 1, jerk JERK (crossover (1 / JERK)^(1/6) rad/s). */
export const JERK = Number(process.env.HERON_JERK ?? 1e-7);
type C = { readonly x: F; readonly y: F; readonly pitch: F; readonly fx: F; readonly fy: F; readonly head?: F };
/**
 * Where the head comes from. `solved` (panel c): an unknown, the neck's turn, held level, the neck and bill held clear
 * of the rail, the body's pitch within 20 degrees. `placed` (panel d, heron_hop_gaze.ts): no head unknown (the head is
 * placed per frame after the solve), the pitch bound relaxed to `pitch` radians, and only the body and legs held clear.
 */
export type Head = { readonly kind: "solved" } | { readonly kind: "placed"; readonly pitch: number };

export function rig(c: C) {
  const body = poseFrame(pose({ at: vec3(c.x, c.y, 0), heading: 0, pitch: c.pitch, heel: 0 }));
  return { body, head: headFrame(body, c.head ?? lift<"f32">(0)), foot: (i: number): P3 => vec3(c.fx, c.fy, HOMES[i]![2]) };
}
function apart(r: readonly [number, number], a: P3, b: P3): F {
  const A = vec2(a.x, a.y), B = vec2(b.x, b.y), R = vec2(r[0], r[1]), ab = sub(B, A);
  const s = clamp(div(dot(sub(R, A), ab), max(dot(ab, ab), 1e-9)), 0, 1);
  return length(sub(R, add(A, mul(ab, s))));
}

export function hop(how: Head) {
  const solvesHead = how.kind === "solved", bound = how.kind === "solved" ? 0.35 : how.pitch;
  return solve({
  over: [0, T.end],
  // spline(27): a knot every 0.075 s, so 0.6 and 1.2 stay knots and acceleration can ramp within 0.075 s.
  vary: { x: spline(27, KEYS.x), y: spline(27, KEYS.y), pitch: spline(27, KEYS.pitch), fx: spline(27, KEYS.fx), fy: spline(27, KEYS.fy), ...(solvesHead ? { head: spline(27, mul(KEYS.pitch, -1)) } : {}) } as Readonly<Record<string, Family>>,
  hold: (cs) => {
    const c = cs as unknown as C;
    const { body, head, foot } = rig(c);
    const d = (e: F) => derivative(e, time);
    const velocity = vec3(d(c.x), d(c.y), d(c.pitch)), accel3 = vec3(d(d(c.x)), d(d(c.y)), d(d(c.pitch)));
    const accel = vec2(d(d(c.x)), d(d(c.y)));
    const hip = inBody(body, hipOf(0)), reach = length(sub(hip, foot(0)));
    const leg = chain({ root: hip, target: foot(0), lengths: LEGS, bend: mul(body.x, -1) }).joints;
    const rail = [n.rail.x, n.rail.y] as const, pad = n.rail.r + 0.03;
    const clear = (stations: readonly (readonly [P3, number])[]) => stations.map(([p, r]) => ge(length(sub(p.xy, rail)), r + pad));
    const along = (f: typeof body, points: readonly (readonly [number, number, number])[], girth: (m: number) => number) =>
      [0, 0.25, 0.5, 0.75, 1].map((m) => [inBody(f, through(lift<"f32">(m), points)), girth(m)] as const);
    const torso = along(body, [[0.55, 0.15, 0], [0, 0, 0], [-0.7, -0.05, 0], [-1.05, -0.2, 0]], (m) => 0.34 * (1 - 0.55 * m));
    const neck = along(head, [[0.5, 0.2, 0], [0.75, 0.55, 0], [0.55, 0.95, 0], [0.75, 1.3, 0], [1.0, 1.38, 0]], (m) => 0.14 + (0.08 - 0.14) * m);
    const bill = [0, 0.5, 1].map((m) => [inBody(head, [1.0 + 0.75 * m, 1.38 - 0.1 * m, 0]), 0.075] as const);
    return [
      always([time, 0, 0], equal(vec3(c.x, c.y, c.pitch), [0, HIP, 0])),
      always([time, 0, 0], equal(velocity, [0, 0, 0])),
      always([time, 0, 0], equal(accel3, [0, 0, 0], 0.001)),                                     // starts from rest: no snap
      always([time, 0, T.off], equal(vec2(c.fx, c.fy), [0, 0])),
      always([time, T.off, T.land], equal(accel, [0, -G], 0.001)),
      always([time, T.land, T.end], equal(vec2(c.fx, c.fy), [n.deck.land, n.deck.top])),
      always([time, T.end, T.end], equal(vec3(c.x, c.y, c.pitch), [n.deck.land, n.deck.top + HIP, 0])),
      always([time, T.end, T.end], equal(velocity, [0, 0, 0])),
      always([time, T.end, T.end], equal(accel3, [0, 0, 0], 0.001)),                             // and stops without a snap
      ge(c.pitch, -bound), le(c.pitch, bound),                                                  // |pitch| bounded (20 degrees for c)
      ...(solvesHead ? [equal(add(c.pitch, c.head!), 0, 0.03)] : []),                          // the head level in the world
      ge(0.97 * SPAN, reach), ge(reach, 0.6), ge(c.fy, 0),
      ...clear(torso), ...(solvesHead ? [...clear(neck), ...clear(bill)] : []),
      ge(apart(rail, leg[0]!, leg[1]!), 0.045 + pad),
      ge(apart(rail, leg[1]!, leg[2]!), 0.045 + pad),
      ge(apart(rail, foot(0), add(foot(0), vec3(0.28, 0, 0))), 0.025 + pad),
    ];
  },
  prefer: { near: { to: solvesHead ? { ...KEYS, head: mul(KEYS.pitch, -1) } : KEYS, weight: 1 }, jerk: JERK },
  });
}
export const solved = hop({ kind: "solved" });

console.error(`heron near: ${solved.report.ms.toFixed(0)} ms, ${solved.report.steps} steps, ${solved.report.rounds} rounds (jerk ${JERK})`);
const { body, head, foot } = rig(solved.curves as C);
export default stage(n, heron(body, foot, { x: vec3(1, 0, 0), z: vec3(0, 0, 1) }, head).things);
