// The crab's scramble with feet that step from the body's weight (panel b of agent_notes/2026-11-19_motion.md,
// "Stepping from weight"). Same body path as crab_scramble_gait.ts. No gait clock: each foot is planted or swinging, a
// `recur` over the 8 feet steps 120 times a second, and a planted foot lifts only when its strain says so.
//
// Per foot i, with h_i(t) its home under the body's ground point, v the body's velocity and p_i its position:
//   strain_i  = |(p_i - (h_i + v Tc)).xz|, Tc = sqrt(H / g): the distance from where an inverted pendulum of the body's
//               height would need the foot (the capture point carried to the foot's home).
//   wants_i   = planted, strain_i > R and planted for at least Tmin.
//   lifts_i   = wants_i, no neighbour (same side i +- 1, and the opposite foot) swinging, and no neighbour that wants
//               with more strain (ties to the lower index): at most alternating neighbours swing.
//   target_i  = h_i + v Tl (its home ahead by about half a step), re-aimed every step of the swing.
//   swing     = s += dt / Tsw; p = mix(from, target, smoothstep(s)) + up L sin(pi s); planted at target when s >= 1.
// A planted foot's position is copied from step to step: it cannot slide.
import { add, and, eq, f, gt, ge, length, lt, mix, mod, mul, not, or, pick, plane, recur, scene, select, sin, smoothstep, sub, toon, vec3, vec4, type Check, type F, type Probe } from "../../src/index.ts";
import { CAMERAS, crab, S } from "./lib/crab.ts";
import { body, lean, v, z } from "./lib/scramble.ts";
import { SECONDS } from "./lib/scramble.ts";

export const STEPPER = { rate: 120, R: 1.3, Tc: Math.sqrt(S.ride / 9.81), Tl: 0.25, Tsw: 0.24, Tmin: 0.12, L: 1.0 };
const P = STEPPER, dt = 1 / P.rate;
const homeX = [-1, 1].flatMap(() => S.legFeet.map(([x]) => x));
const homeZ = [-1, 1].flatMap((s) => S.legFeet.map(([, fz]) => s * fz));
const home = (j: F) => vec3(pick(j, homeX), 0, add(z, pick(j, homeZ)));

export const feet = recur({
  name: "feet", size: 8, rate: P.rate, until: SECONDS,
  // Lanes: (position, swinging 0/1), (swing start, swing share s), (seconds planted, steps taken, 0, 0).
  start: (i) => [vec4(home(i), 0), vec4(home(i), 0), vec4(P.Tmin, 0, 0, 0)],
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
    const swingAt = (from: ReturnType<typeof vec3>, s: F) => add(mix(from, target, smoothstep(0, 1, s)), vec3(0, mul(sin(mul(s, Math.PI)), P.L), 0));
    // Swinging: advance s; planted again at the target when it reaches 1.
    const s2 = add(fs.w, dt / P.Tsw), done = ge(s2, 1);
    const swingNext = [vec4(select(done, target, swingAt(fs.xyz, s2)), select(done, 0, 1)), vec4(fs.xyz, s2), vec4(0, st.y, 0, 0)];
    // Planted: stay put (copied exactly), or lift now from here.
    const liftNext = [vec4(swingAt(pw.xyz, f(dt / P.Tsw)), 1), vec4(pw.xyz, dt / P.Tsw), vec4(0, add(st.y, 1), 0, 0)];
    const stayNext = [pw, fs, vec4(add(st.x, dt), st.y, 0, 0)];
    const isSwinging = gt(pw.w, 0.5);
    return [0, 1, 2].map((l) => select(isSwinging, swingNext[l]!, select(lifts, liftNext[l]!, stayNext[l]!)));
  },
});

const foot = (k: number) => feet.at(k)[0]!.xyz;
export default scene({ ground: plane, things: crab(body, foot), style: toon({ bands: 3, outline: 0.04 }), sun: [-0.45, 0.75, 0.5], cameras: CAMERAS });

// Measured on the baked state (loft check): steps, slide while planted, neighbours swinging together, leg stretch.
const T = Array.from({ length: SECONDS * P.rate + 1 }, (_, n) => n / P.rate);
export const checks: Check[] = [{
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
    const firsts = [0, 1, 2, 3, 4, 5, 6, 7].map((k) => T.find((_, n) => rows[n]![2 * k]![3]! > 0.5) ?? -1);
    // The first lift after the body's velocity turns negative (the reversal).
    const vs = T.map((t) => probe.scalar(v, t)), turn = T.find((_, n) => vs[n]! < 0) ?? -1;
    const after = T.find((t, n) => t > turn && [0, 1, 2, 3, 4, 5, 6, 7].some((k) => rows[n]![2 * k]![3]! > 0.5 && rows[n - 1]![2 * k]![3]! < 0.5)) ?? -1;
    const leans = T.map((t) => probe.scalar(lean, t)), most = leans.reduce((m, x, n) => (Math.abs(x) > Math.abs(m.x) ? { x, t: T[n]! } : m), { x: 0, t: 0 });
    return { name: "stepper", pass: slide === 0 && together === 0, value: total,
      measured: `max lean ${(most.x * 180 / Math.PI).toFixed(1)} deg at ${most.t.toFixed(2)}; v < 0 from ${turn.toFixed(3)}, next lift ${after.toFixed(3)}; steps ${total} (per foot ${steps.join(",")}); first lifts ${firsts.map((x) => x.toFixed(2)).join(",")}; planted slide ${slide}; neighbour pairs swinging together ${together}`, limit: "slide 0, no neighbours together" };
  },
}];
