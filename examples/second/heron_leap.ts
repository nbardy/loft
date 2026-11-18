// The heron of heron.ts leaps a rail onto a deck: crouches, springs, flies, lands both feet on the deck at t = 1.2 s
// and stands. Nothing is keyed. The motion is `solve`d from what must hold (planted feet, a ballistic flight, legs
// within reach, every part clear of the rail, both feet on the deck at 1.2), the smoothest such motion (least jerk).
// The `moved` variant puts the deck farther and higher and solves again; heron_leap_refused.ts asks for a rail no
// 0.6 s flight can clear, and the local search is refused.
// Shows: `solve` (src/geom/solve.ts) driving a rig (`poseFrame` for the body, `chain` for the legs).
//
// Theory, before running. Takeoff at 0.6 s, landing at 1.2: a 0.6 s parabola rises g T^2 / 8 = 0.44 m above the mean
// of its end heights, so the body must leave the ground fast: from feet at (0, 0) to feet at (3.2, 0.7), with the
// hips at most 0.97 x 2.15 = 2.09 from the feet, the hips leave at about 1.9-2.0 m and land about 2.4-2.6 m, so
// vy0 = (y1 - y0) / T + g T / 2 = 3.4 to 3.8 m/s and vx = 2.3 to 3 m/s. Standing still at 1.9 m at t = 0 and leaving at
// about 1.95 m at 3.6 m/s in 0.6 s needs the body to go down first: predicted crouch (the anticipation nobody keyed)
// 0.4 to 0.6 m below standing, lowest near 0.3 s; apex hip height 2.5 to 2.8 m near 0.95 s; and a dip on landing
// (it arrives at about -2.3 m/s and must stop by 1.8 s). The rail at x = 1.7, 1.0 m high, is under the body near its
// apex, where the dangling feet would be at about 0.6 m: predicted, the feet tuck up behind (fx lags the body) to clear
// it. Moved deck (x 4.0, 1.0 high): vx up by 0.8 / 0.6 = 1.3 m/s, vy0 up by about 0.5 m/s. Solve time: under 2 s each.
//
// Scored: agent_notes/2026-11-18_solve/NOTE.md.
import {
  add, clamp, design, div, dot, length, lift, max, mix, mul, plane, pose, sin, smoothstep, poseFrame, scene, strand, sub, through, time, toon, vec2, vec3,
  type F, type P3,
} from "../../src/index.ts";
import { derivative } from "../../src/grad.ts";
import { chain } from "../../src/geom/limb.ts";
import { atLeast, equal, solve, spline } from "../../src/geom/solve.ts";
import { heron, hipOf, HOMES, inBody, LEGS } from "./lib/heron.ts";

export const NUMBERS = {
  /** The deck: its front and back edges (x), its top, and where the feet land on it. */
  deck: { from: 2.6, to: 4.4, top: 0.7, land: 3.2 },
  /** The rail: a bar along z at (x, y), its radius. */
  rail: { x: 1.7, y: 1.0, r: 0.06 },
};
type N = { readonly deck: { readonly from: number; readonly to: number; readonly top: number; readonly land: number }; readonly rail: { readonly x: number; readonly y: number; readonly r: number } };

const HIP = 1.9, SPAN = LEGS[0] + LEGS[1], T = { off: 0.6, land: 1.2, end: 1.8 }, G = 9.81;
const brown = [0.42, 0.3, 0.2] as const, steel = [0.35, 0.37, 0.4] as const;

/** Least distance in the side plane (x, y) from point r to the segment a-b. */
function apart(r: readonly [number, number], a: P3, b: P3): F {
  const A = vec2(a.x, a.y), B = vec2(b.x, b.y), R = vec2(r[0], r[1]), ab = sub(B, A);
  const s = clamp(div(dot(sub(R, A), ab), max(dot(ab, ab), 1e-9)), 0, 1);
  return length(sub(R, add(A, mul(ab, s))));
}

/** The bird for given curves of its body (x, y, pitch) and both feet (fx, fy): the body frame and the foot targets. */
function rig(c: { readonly x: F; readonly y: F; readonly pitch: F; readonly fx: F; readonly fy: F }) {
  const body = poseFrame(pose({ at: vec3(c.x, c.y, 0), heading: 0, pitch: c.pitch, heel: 0 }));
  const foot = (i: number): P3 => vec3(c.fx, c.fy, HOMES[i]![2]);
  return { body, foot };
}

/** Every hold of the leap, over curves of the body and feet: the lines a conflict is named by. */
function holds(n: N, c: Parameters<typeof rig>[0]) {
  const { body, foot } = rig(c);
  const at = (p: readonly [number, number, number]) => inBody(body, p);
  const velocity = vec3(derivative(c.x, time), derivative(c.y, time), derivative(c.pitch, time));
  const accel = vec2(derivative(derivative(c.x, time), time), derivative(derivative(c.y, time), time));
  const hip = at(hipOf(0)), reach = length(sub(hip, foot(0)));
  const leg = chain({ root: hip, target: foot(0), lengths: LEGS, bend: mul(body.x, -1) }).joints;
  const rail = [n.rail.x, n.rail.y] as const, pad = n.rail.r + 0.03;
  // Clearance of a part: one hold per station (distance to the rail's axis, less its own radius and the pad). One hold
  // per station, not the least of them: a `min` has corners where the nearest station changes, which stall the solve.
  const clear = (stations: readonly (readonly [P3, number])[]) => stations.map(([p, r]) => atLeast(apart(rail, p, p), r + pad));
  const along = (points: readonly (readonly [number, number, number])[], girth: (m: number) => number) =>
    [0, 0.25, 0.5, 0.75, 1].map((m) => [inBody(body, through(lift<"f32">(m), points)), girth(m)] as const);
  const torso = along([[0.55, 0.15, 0], [0, 0, 0], [-0.7, -0.05, 0], [-1.05, -0.2, 0]], (m) => 0.34 * (1 - 0.55 * m));
  const neck = along([[0.5, 0.2, 0], [0.75, 0.55, 0], [0.55, 0.95, 0], [0.75, 1.3, 0], [1.0, 1.38, 0]], (m) => 0.14 + (0.08 - 0.14) * m);
  const bill = [0, 0.5, 1].map((m) => [at([1.0 + 0.75 * m, 1.38 - 0.1 * m, 0]), 0.075] as const);
  return [
    equal(vec3(c.x, c.y, c.pitch), [0, HIP, 0], { at: 0 }),                                    // stands
    equal(velocity, [0, 0, 0], { at: 0 }),                                                     // still
    equal(vec2(c.fx, c.fy), [0, 0], { from: 0, to: T.off }),                                   // planted
    equal(accel, [0, -G], { from: T.off, to: T.land }),                                        // flies: a parabola
    equal(vec2(c.fx, c.fy), [n.deck.land, n.deck.top], { from: T.land, to: T.end }),           // lands on the deck at 1.2
    equal(vec3(c.x, c.y, c.pitch), [n.deck.land, n.deck.top + HIP, 0], { at: T.end }),         // stands on it
    equal(velocity, [0, 0, 0], { at: T.end }),                                                 // still
    atLeast(0.97 * SPAN, reach),                                                               // legs within reach
    atLeast(reach, 0.6),                                                                       // and not folded flat
    atLeast(c.fy, 0),                                                                          // feet above the ground
    ...clear(torso),                                                                           // clear of the rail
    ...clear(neck),
    ...clear(bill),
    atLeast(apart(rail, leg[0]!, leg[1]!), 0.045 + pad),
    atLeast(apart(rail, leg[1]!, leg[2]!), 0.045 + pad),
    atLeast(apart(rail, foot(0), add(foot(0), vec3(0.28, 0, 0))), 0.025 + pad),
  ];
}

/** The leap for these numbers: solved once, here, at build time. */
export function leap(n: N, budget?: { readonly ms: number }) {
  // The search starts from a rough leap that goes over the rail (a local search keeps the side it starts on: from
  // feet at rest it finds the feet passing under the rail and the legs through it). The answer does not lean on it.
  const flight = clamp(div(sub(time, T.off), T.land - T.off), 0, 1), arch = sin(mul(flight, Math.PI)), across = smoothstep(T.off, T.land, time);
  const solved = solve({
    over: [0, T.end], budget,
    // One number per 0.15 s, so takeoff (0.6) and landing (1.2) fall on knots: the flight's parabola starts and ends
    // exactly there, and the push before it is free to differ.
    vary: {
      x: spline(15, mul(across, n.deck.land)), y: spline(15, add(add(HIP, mul(across, n.deck.top)), mul(arch, 0.8))), pitch: spline(15),
      fx: spline(15, mul(across, n.deck.land)), fy: spline(15, add(mul(across, n.deck.top), mul(arch, 1.3))),
    },
    hold: (c) => holds(n, c),
    prefer: { jerk: 1 },
  });
  const { body, foot } = rig(solved.curves);
  const bird = heron(body, foot, { x: vec3(1, 0, 0), z: vec3(0, 0, 1) });
  return { solved, body, foot, bird };
}

function built(n: N) {
  const { bird, solved: leapMotion } = leap(n);
  const box = strand({
    name: "deck", spine: (m) => vec3(mix(n.deck.from, n.deck.to, m), n.deck.top / 2, 0), stations: 4, girth: 1.2,
    section: { aspect: n.deck.top / 2 / 1.2, squareness: 8 }, tips: [{ round: 0.04 }, { round: 0.04 }], paint: brown, detail: { along: 8, around: 32 }, up: [0, 1, 0],
  });
  const bar = (name: string, a: readonly [number, number, number], b: readonly [number, number, number], r: number) =>
    strand({ name, spine: (m) => mix(vec3(...a), vec3(...b), m), stations: 4, girth: r, tips: ["round", "round"], paint: steel, detail: { along: 12, around: 12 } });
  const { x, y, r } = n.rail;
  const things = [...bird.things, box, bar("rail", [x, y, -1.4], [x, y, 1.4], r), bar("postL", [x, 0, -1.4], [x, y, -1.4], r), bar("postR", [x, 0, 1.4], [x, y, 1.4], r)];
  const world = scene({
    ground: plane, things, style: toon({ bands: 3, outline: 0.03 }), sun: [-0.4, 0.8, 0.45], period: 2.4,
    cameras: {
      side: { kind: "perspective", eye: [0.5, 2.6, 10], target: [2.2, 1.4, 0], fovY: 0.62 },
      game: { kind: "perspective", eye: [7.5, 5.5, 7], target: [2.0, 1.3, 0], fovY: 0.6 },
    },
  });
  return { scene: world, checks: leapMotion.holds };
}

export const heronLeap = design({
  name: "heron_leap", numbers: NUMBERS, slots: {}, compare: ["side"],
  variants: { moved: { about: "the deck 0.8 farther and 0.3 higher, solved again", numbers: { deck: { from: 3.4, to: 5.2, top: 1.0, land: 4.0 } } } },
  build: (_, n) => built(n),
});
export { heronLeap as design };
export const sceneFor = (n: N) => built(n).scene;
