// The body path both scramble panels share (agent_notes/2026-11-19_motion.md, "Stepping from weight"): hand keys as
// closed forms of time. The crab moves to its right (+z) and then, before it has stopped, hard to its left. It leans
// into its acceleration, roll = atan(a / g), and its body lags the path by LAG * a (the centre of mass shifting).
import { add, clamp, div, lift, mul, sub, time, vec3, type F } from "../../../src/index.ts";
import { atan } from "../../../src/expr.ts";
import { derivative } from "../../../src/grad.ts";
import { bodyFrame, S } from "./crab.ts";

/** Smootherstep (C2): 0 to 1 as u goes 0 to 1, with zero speed and acceleration at both ends. */
const s5 = (u: F): F => { const c = clamp(u, 0, 1); return mul(mul(c, c), mul(c, add(10, mul(c, add(-15, mul(c, 6)))))); };
const G = 9.81, LAG = 0.08;
/** Where the body's ground point is along z: +1.8 over 0.2..1.9 s, then -3.2 over 1.7..3.5 s. */
export const z: F = sub(mul(s5(div(sub(time, 0.2), 1.7)), 1.8), mul(s5(div(sub(time, 1.7), 1.8)), 3.2));
export const v: F = derivative(z, time);
export const a: F = derivative(v, time);
export const lean: F = atan(div(a, G));
/** The body: over its ground point, lagging the path by LAG a, rolled into the acceleration. */
export const body = bodyFrame(vec3(0, S.ride, sub(z, mul(a, LAG))), lift<"f32">(0), lean);
export const SECONDS = 4;
