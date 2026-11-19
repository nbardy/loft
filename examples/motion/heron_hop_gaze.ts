// The heron's hop with a stabilised head and an IK neck (panel d of agent_notes/2026-11-19_motion.md). The body is
// panel c's solve (near the hand keys, the flight a parabola, feet planted) with the pitch bound relaxed to 34 degrees
// and no head unknown. The head is placed per frame, not solved:
//   - position: a critically damped follower (a `recur`, 120 steps/s, w = 2 pi 2 Hz) of the shoulder's path plus the
//     rest neck offset, fed the shoulder's velocity times LEAD / w (LEAD = 1: half the steady lag cancelled); during the run-up
//     (0 to 0.6 s) its target holds the shoulder's position at the end of each 0.2 s and then jumps: hold, then thrust.
//   - attitude: no roll, the bill toward a look point (the landing spot, then ahead), its pitch within -20..+12 degrees.
// The neck is a fixed-length S between the shoulder and the head: two circular arcs, each of chord d / 2 and length
// L / 2, bending opposite ways. The bend angle theta solves sin(theta) / theta = d / L by four Newton steps per frame.
import {
  add, atan2, clamp, cos, div, floor, length, lift, lt, max, min, mix, mul, normalize, recur, select, sin, smoothstep, sqrt, sub, time, vec3, vec4,
  type Check, type F, type Frame, type P3, type Probe,
} from "../../src/index.ts";
import { replace, type Expr } from "../../src/expr.ts";
import { derivative } from "../../src/grad.ts";
import { heron, inBody, NECK_BASE } from "../second/lib/heron.ts";
import { NUMBERS, stage } from "../second/heron_leap.ts";
import { hop, rig } from "./heron_hop_near.ts";

/** The neck's length (metres) and the head's rest offset from the shoulder (world, standing). */
export const NECK = { L: 1.55, rest: [0.5, 1.18, 0] as const, w: 2 * Math.PI * 2, bob: 0.2, runUp: 0.6, rate: 120, until: 2.4 };
export const solved = hop({ kind: "placed", pitch: 0.6 });
const { body, foot } = rig(solved.curves as never);
const shoulder = inBody(body, NECK_BASE);
const at = <T extends Expr>(e: T, when: F): T => replace([e], new Map([[time.id, when]]))[0] as T;
// Hold, then thrust: in the run-up the target is where the shoulder will be at the end of this 0.2 s.
const held = select(lt(time, NECK.runUp), min(mul(add(floor(div(time, NECK.bob)), 1), NECK.bob), NECK.runUp), time);
// Feed-forward of the shoulder velocity: LEAD = 2 cancels the steady lag but passes acceleration steps (landing) as
// velocity steps; LEAD = 1 (chosen) halves that and leaves a lag of v / w (the note scores both).
export const LEAD = Number(process.env.HERON_LEAD ?? 1);
const lead = mul(derivative(shoulder, time), LEAD / NECK.w);
const target = add(add(vec3(at(shoulder, held).x, shoulder.y, shoulder.z), vec3(...NECK.rest)), lead);

export const headPath = recur({
  name: "heronHead", size: 1, rate: NECK.rate, until: NECK.until,
  start: () => [vec4(add(shoulder, vec3(...NECK.rest)), 0), vec4(0)],
  step: (prev) => {
    const [p, v] = prev(0) as [ReturnType<typeof vec4>, ReturnType<typeof vec4>];
    // Semi-implicit Euler on p'' = w^2 (target - p) - 2 w p' (critically damped).
    const acc = sub(mul(sub(target, p.xyz), NECK.w * NECK.w), mul(v.xyz, 2 * NECK.w));
    const v2 = add(v.xyz, mul(acc, 1 / NECK.rate));
    return [vec4(add(p.xyz, mul(v2, 1 / NECK.rate)), 0), vec4(v2, 0)];
  },
});
const head: P3 = headPath.at(0)[0]!.xyz;

// The look point: the landing spot until about the top of the arc, then ahead.
const look = mix(vec3(NUMBERS.deck.land + 0.6, NUMBERS.deck.top, 0), add(head, vec3(6, 0, 0)), smoothstep(0.9, 1.4, time));
const toward = sub(look, head), pitch = clamp(atan2(toward.y, toward.x), -0.35, 0.2);
const turned = { x: vec3(cos(pitch), sin(pitch), 0), y: vec3(mul(sin(pitch), -1), cos(pitch), 0), z: lift<"vec3">([0, 0, 1]) };
// The head frame puts the bill's root (1.0, 1.38) of heron.ts's head coordinates at the head point.
const headFrame: Frame = { ...turned, o: sub(head, add(mul(turned.x, 1.0), mul(turned.y, 1.38))) };

// The neck: an S of two arcs of length L / 2 over chords of d / 2.
const chord = sub(head, shoulder), d = length(chord), dir = normalize(chord), side = vec3(mul(dir.y, -1), dir.x, 0);
const k = clamp(div(d, NECK.L), 0.05, 0.999);
let theta: F = min(sqrt(mul(sub(1, k), 6)), 2.5);
for (let n = 0; n < 4; n++) theta = sub(theta, div(sub(sin(theta), mul(k, theta)), sub(cos(theta), k)));
const arc = (a: P3, b: P3, bulge: P3, u: F): P3 => {
  const phi = mul(theta, sub(mul(u, 2), 1)), half = mul(length(sub(b, a)), 0.5), st = max(sin(theta), 1e-4);
  return add(add(mul(add(a, b), 0.5), mul(dir, mul(half, div(sin(phi), st)))), mul(bulge, mul(half, div(sub(cos(phi), cos(theta)), st))));
};
const middle = mul(add(shoulder, head), 0.5);
export const neckSpine = (m: F): P3 => select(lt(m, 0.5), arc(shoulder, middle, mul(side, -1), mul(m, 2)), arc(middle, head, side, sub(mul(m, 2), 1)));

export default stage(NUMBERS, heron(body, foot, { x: vec3(1, 0, 0), z: vec3(0, 0, 1) }, headFrame, neckSpine).things);

// Readouts on the baked head (loft check): head jerk, neck length, chord over length, body pitch, bill to rail.
export const checks: Check[] = [{
  name: "gaze readout",
  run: (probe: Probe) => {
    const h = 1 / NECK.rate, T = Array.from({ length: Math.round(1.8 / h) + 1 }, (_, n) => n * h);
    const P = T.map((t) => probe.eval([head], t)[0] as number[]);
    const jerk = (lo: number, hi: number) => {
      let m = 0;
      for (let n = 2; n < P.length - 1; n++) if (T[n]! >= lo && T[n]! <= hi) {
        const j = [0, 1, 2].map((c) => (P[n + 1]![c]! - 3 * P[n]![c]! + 3 * P[n - 1]![c]! - P[n - 2]![c]!) / h ** 3);
        m = Math.max(m, Math.hypot(...j));
      }
      return m;
    };
    const argmax = (lo: number, hi: number) => { let m = 0, at = lo; for (let n = 2; n < P.length - 1; n++) if (T[n]! >= lo && T[n]! <= hi) { const j = Math.hypot(...[0, 1, 2].map((c) => (P[n + 1]![c]! - 3 * P[n]![c]! + 3 * P[n - 1]![c]! - P[n - 2]![c]!) / h ** 3)); if (j > m) { m = j; at = T[n]!; } } return at; };
    const ms = Array.from({ length: 129 }, (_, i) => i / 128), spine = ms.map((m) => neckSpine(lift<"f32">(m)));
    let lenErr = 0, ratio = 0, rail = 9;
    const bill = inBody(headFrame, [1.75, 1.28, 0]);
    for (const t of T.filter((_, n) => n % 4 === 0)) {
      const pts = probe.eval(spine, t) as number[][];
      let len = 0; for (let i = 1; i < pts.length; i++) len += Math.hypot(...[0, 1, 2].map((c) => pts[i]![c]! - pts[i - 1]![c]!));
      lenErr = Math.max(lenErr, Math.abs(len - NECK.L)); ratio = Math.max(ratio, probe.scalar(d, t) / NECK.L);
      const [bx, by] = probe.eval([bill], t)[0] as number[], [rx, ry] = probe.eval([inBody(headFrame, [1.0, 1.38, 0])], t)[0] as number[];
      // Least distance from the rail's axis to the bill (root to tip), in the side plane.
      for (let q = 0; q <= 8; q++) { const x = rx! + (bx! - rx!) * q / 8, y = ry! + (by! - ry!) * q / 8; rail = Math.min(rail, Math.hypot(x - NUMBERS.rail.x, y - NUMBERS.rail.y)); }
    }
    const pitches = T.map((t) => probe.scalar(solved.curves.pitch!, t)), deg = (r: number) => (r * 180 / Math.PI).toFixed(1);
    return { name: "gaze", pass: true, value: lenErr,
      measured: `head jerk max ${jerk(0.05, 0.6).toFixed(0)} (run-up) / ${jerk(0.6, 1.75).toFixed(0)} (0.6..1.75) m/s^3 (0.8..1.75: ${jerk(0.8, 1.75).toFixed(0)}; argmax ${argmax(0.6, 1.75).toFixed(3)} s); neck length error ${lenErr.toExponential(2)} m (L ${NECK.L}); chord/L max ${ratio.toFixed(3)}; body pitch ${deg(Math.min(...pitches))}..${deg(Math.max(...pitches))} deg; bill to rail axis ${rail.toFixed(2)} m; solve ${solved.report.ms.toFixed(0)} ms`,
      limit: "readout" };
  },
}];

