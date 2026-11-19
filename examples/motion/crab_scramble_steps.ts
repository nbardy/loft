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
import { plane, scene, toon } from "../../src/index.ts";
import { CAMERAS, crab } from "./lib/crab.ts";
import { body } from "./lib/scramble.ts";
import { readout, stepper } from "./lib/stepper.ts";

export const feet = stepper("feet", { kind: "fixed", seconds: 0.24, lift: 1.0 });
export default scene({ ground: plane, things: crab(body, (k) => feet.at(k)[0]!.xyz), style: toon({ bands: 3, outline: 0.04 }), sun: [-0.45, 0.75, 0.5], cameras: CAMERAS });
export const checks = readout(feet);
