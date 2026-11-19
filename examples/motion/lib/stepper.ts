// Feet that step from the body's weight (agent_notes/2026-11-19_motion.md, "Stepping from weight"): a `recur` over the
// crab's 8 feet. Equations in crab_scramble_steps.ts. `Swing` says how long a swing takes and how high it lifts:
// fixed numbers, or set at lift-off from the step's length and its urgency.
import { add, and, clamp, eq, f, gt, ge, length, lt, max, mix, mod, mul, not, or, pick, recur, select, sin, smoothstep, sub, vec2, vec3, vec4, type Check, type F, type Probe } from "../../../src/index.ts";
import { S } from "./crab.ts";
import { lean, SECONDS, v, z } from "./scramble.ts";

/**
 * How a swing is shaped. `fixed`: every swing takes `seconds` and lifts `lift`. `varied`: set at lift-off from the step
 * length l (metres, foot to target) and the urgency u in [0, 1] (how far the capture point is from the planted feet's
 * centroid, past 0.5 m, over 2 m): seconds = clamp(0.30 + 0.03 l - 0.30 u, 0.12, 0.40), lift = clamp(0.7 + 0.25 l -
 * 1.2 u, 0.25, 1.6). Urgent steps are short and low, relaxed ones longer and higher.
 */
export type Swing = { readonly kind: "fixed"; readonly seconds: number; readonly lift: number } | { readonly kind: "varied" };

export const STEPPER = { rate: 120, R: 1.3, Tc: Math.sqrt(S.ride / 9.81), Tl: 0.25, Tmin: 0.12 };
const P = STEPPER, dt = 1 / P.rate;
const homeX = [-1, 1].flatMap(() => S.legFeet.map(([x]) => x));
const homeZ = [-1, 1].flatMap((s) => S.legFeet.map(([, fz]) => s * fz));
const home = (j: F) => vec3(pick(j, homeX), 0, add(z, pick(j, homeZ)));

export function stepper(name: string, swing: Swing) {
  return recur({
    name, size: 8, rate: P.rate, until: SECONDS,
    // Lanes: (position, swinging 0/1), (swing start, swing share s), (seconds planted, steps taken, swing seconds, lift).
    start: (i) => [vec4(home(i), 0), vec4(home(i), 0), vec4(P.Tmin, 0, 0.24, 1)],
    step: (prev, i) => {
      const strain = (j: F) => length(sub(prev(j)[0]!.xyz, add(home(j), vec3(0, 0, mul(v, P.Tc)))).xz);
      const swinging = (j: F) => gt(prev(j)[0]!.w, 0.5);
      const wants = (j: F) => and(and(not(swinging(j)), gt(strain(j), P.R)), ge(prev(j)[2]!.x, P.Tmin));
      const k = mod(i, 4), mine = strain(i);
      // A neighbour n blocks i when it swings, or wants to lift with more strain (ties go to the lower index).
      const blocks = (n: F) => or(swinging(n), and(wants(n), or(gt(strain(n), mine), and(eq(strain(n), mine), lt(n, i)))));
      // Same-side neighbours exist only inside the side's row of four (k = i mod 4); the opposite foot always does.
      const blocked = or(or(and(gt(k, 0.5), blocks(sub(i, 1))), and(lt(k, 2.5), blocks(add(i, 1)))), blocks(mod(add(i, 4), 8)));
      const lifts = and(wants(i), not(blocked));
      const [pw, fs, st] = prev(i) as [ReturnType<typeof vec4>, ReturnType<typeof vec4>, ReturnType<typeof vec4>];
      const target = add(home(i), vec3(0, 0, mul(v, P.Tl)));
      const shape = shapeOf(swing, length(sub(target, pw.xyz).xz), urgency(prev));
      const swingAt = (from: ReturnType<typeof vec3>, s: F, lift: F) => add(mix(from, target, smoothstep(0, 1, s)), vec3(0, mul(sin(mul(s, Math.PI)), lift), 0));
      // Swinging: advance s by this swing's own duration; planted again at the target when it reaches 1.
      const s2 = add(fs.w, div1(st.z)), done = ge(s2, 1);
      const swingNext = [vec4(select(done, target, swingAt(fs.xyz, s2, st.w)), select(done, 0, 1)), vec4(fs.xyz, s2), vec4(0, st.y, st.z, st.w)];
      // Planted: stay put (copied exactly), or lift now from here with the swing's duration and height fixed for it.
      const s1 = div1(shape.seconds);
      const liftNext = [vec4(swingAt(pw.xyz, s1, shape.lift), 1), vec4(pw.xyz, s1), vec4(0, add(st.y, 1), shape.seconds, shape.lift)];
      const stayNext = [pw, fs, vec4(add(st.x, dt), st.y, st.z, st.w)];
      const isSwinging = gt(pw.w, 0.5);
      return [0, 1, 2].map((l) => select(isSwinging, swingNext[l]!, select(lifts, liftNext[l]!, stayNext[l]!)));
    },
  });
}
/** dt over a swing's seconds: the share of the swing one step takes. */
const div1 = (seconds: F): F => mul(dt, f(1).div(seconds));

/** Urgency u in [0, 1]: the capture point's distance from the planted feet's centroid, past 0.5 m, over 2 m. */
function urgency(prev: (j: F | number) => readonly ReturnType<typeof vec4>[]): F {
  let sum: ReturnType<typeof vec2> = vec2(0, 0), n: F = f(0);
  for (let j = 0; j < 8; j++) { const w = sub(1, prev(j)[0]!.w); sum = add(sum, mul(prev(j)[0]!.xz, w)); n = add(n, w); }
  const centroid = mul(sum, f(1).div(max(n, 1))), capture = vec2(0, add(z, mul(v, P.Tc)));
  return clamp(mul(sub(length(sub(capture, centroid)), 0.5), 0.5), 0, 1);
}

/** The swing's seconds and lift: one handler per kind. */
function shapeOf(swing: Swing, l: F, u: F): { readonly seconds: F; readonly lift: F } {
  switch (swing.kind) {
    case "fixed": return { seconds: f(swing.seconds), lift: f(swing.lift) };
    case "varied": return {
      seconds: clamp(sub(add(0.30, mul(l, 0.03)), mul(u, 0.30)), 0.12, 0.40),
      lift: clamp(sub(add(0.7, mul(l, 0.25)), mul(u, 1.2)), 0.25, 1.6),
    };
  }
}

// Measured on the baked state (loft check): steps, slide while planted, neighbours swinging together, swing shapes.
const T = Array.from({ length: SECONDS * P.rate + 1 }, (_, n) => n / P.rate);
export function readout(feet: ReturnType<typeof stepper>): Check[] {
  return [{
  name: "stepper readout",
  run: (probe: Probe) => {
    const lanes = [0, 1, 2, 3, 4, 5, 6, 7].flatMap((k) => [feet.at(k)[0]!, feet.at(k)[2]!]);
    const rows = T.map((t) => probe.eval(lanes, t) as number[][]);
    let slide = 0, together = 0;
    const nbr = (a: number, b: number) => Math.abs(a - b) === 4 || (Math.floor(a / 4) === Math.floor(b / 4) && Math.abs(a - b) === 1);
    for (let n = 1; n < rows.length; n++) for (let k = 0; k < 8; k++) {
      const [p0, p1] = [rows[n - 1]![2 * k]!, rows[n]![2 * k]!];
      // Exactly planted at both reads: a read between a landing step and the next mixes the two lanes (`at` is linear).
      if (p0[3]! === 0 && p1[3]! === 0) slide = Math.max(slide, Math.hypot(p1[0]! - p0[0]!, p1[1]! - p0[1]!, p1[2]! - p0[2]!));
      for (let j = k + 1; j < 8; j++) if (nbr(k, j) && rows[n]![2 * k]![3]! > 0.5 && rows[n]![2 * j]![3]! > 0.5) together++;
    }
    const steps = rows.at(-1)!.filter((_, c) => c % 2 === 1).map((l) => l[1]!);
    const total = steps.reduce((a, b) => a + b, 0);
    // Each swing's seconds and lift, read at its lift-off.
    const shapes: number[][] = [];
    for (let n = 1; n < rows.length; n++) for (let k = 0; k < 8; k++) if (rows[n]![2 * k]![3]! > 0.5 && rows[n - 1]![2 * k]![3]! < 0.5) shapes.push([rows[n]![2 * k + 1]![2]!, rows[n]![2 * k + 1]![3]!]);
    const range = (c: number) => `${Math.min(...shapes.map((x) => x[c]!)).toFixed(3)}..${Math.max(...shapes.map((x) => x[c]!)).toFixed(3)}`;
    const firsts = [0, 1, 2, 3, 4, 5, 6, 7].map((k) => T.find((_, n) => rows[n]![2 * k]![3]! > 0.5) ?? -1);
    // The first lift after the body's velocity turns negative (the reversal).
    const vs = T.map((t) => probe.scalar(v, t)), turn = T.find((_, n) => vs[n]! < 0) ?? -1;
    const after = T.find((t, n) => t > turn && [0, 1, 2, 3, 4, 5, 6, 7].some((k) => rows[n]![2 * k]![3]! > 0.5 && rows[n - 1]![2 * k]![3]! < 0.5)) ?? -1;
    const leans = T.map((t) => probe.scalar(lean, t)), most = leans.reduce((m, x, n) => (Math.abs(x) > Math.abs(m.x) ? { x, t: T[n]! } : m), { x: 0, t: 0 });
    return { name: "stepper", pass: slide === 0 && together === 0, value: total,
      measured: `swing s ${range(0)}, lift ${range(1)}; max lean ${(most.x * 180 / Math.PI).toFixed(1)} deg at ${most.t.toFixed(2)}; v < 0 from ${turn.toFixed(3)}, next lift ${after.toFixed(3)}; steps ${total} (per foot ${steps.join(",")}); first lifts ${firsts.map((x) => x.toFixed(2)).join(",")}; planted slide ${slide}; neighbour pairs swinging together ${together}`, limit: "slide 0, no neighbours together" };
  },
}];
}
