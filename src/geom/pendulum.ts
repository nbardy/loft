// A hanging thing's swing, in closed form (docs/animation_principles.md, "Stalks and hanging things").
//
// A bob on an arm of length L under gravity g, lightly damped, hung from a pivot that something else moves. For small
// swings its angle from plumb, psi (radians, positive the way the pivot's positive displacement points), obeys
//
//   psi'' + 2 zeta w0 psi' + w0^2 psi = -x_p'' / L        w0 = sqrt(g / L)
//
// which is linear, so its answer to several drives is the sum of its answers to each, and each is a closed form of the
// clock (no state between frames):
//
//   sway   the pivot moves A sin(w t + phase), for ever (a walk's sway). The bob has settled into
//          psi = G sin(w t + phase - lag), with G = (A w^2 / L) / sqrt((w0^2 - w^2)^2 + (2 zeta w0 w)^2) and
//          lag = atan2(2 zeta w0 w, w0^2 - w^2). Near w = w0 the swing is large and a quarter turn late.
//   jolt   the pivot's velocity changes by v at an instant (a slam, a snatch, a throw's release). The bob is left
//          behind: psi = -(v / (L wd)) exp(-zeta w0 s) sin(wd s), s seconds later; wd = w0 sqrt(1 - zeta^2).
//
// Nothing here is chosen but the damping: how fast it swings follows from its length and the world's gravity, how far
// from how hard its pivot was moved. Two bobs of different lengths drift in and out of step by themselves.
//
// Inherited: the damped driven oscillator, a first-year result. Mine: using its steady state and impulse response as
// the animation itself, so a hanging part's swing is derived from the rig's own sway and attack clocks rather than keyed.

import { add, exp, lift, lt, max, mul, select, sin, type Expr, type In } from "../expr.ts";

type F = Expr<"f32">;

export type PendulumSpec = {
  /** The arm's length, world units, and the world's gravity, units / s^2. */
  readonly length: number; readonly gravity: number;
  /** Damping ratio, 0 (swings for ever) to below 1; 0.1 dies away over about three swings. */
  readonly damping: number;
};

/** What moves the pivot. One dispatcher reads the kind (`answer`). */
export type Drive =
  /** The pivot sways `amplitude` sin(2 pi t / period + phase), world units, and has for a long time. */
  | { readonly kind: "sway"; readonly amplitude: In<"f32">; readonly period: number; readonly phase?: number }
  /** The pivot's velocity changed by `speed` (units / s) when `after` (seconds) passed 0; nothing before. */
  | { readonly kind: "jolt"; readonly after: In<"f32">; readonly speed: In<"f32"> };

export const sway = (amplitude: In<"f32">, period: number, phase = 0): Drive => ({ kind: "sway", amplitude, period, phase });
export const jolt = (after: In<"f32">, speed: In<"f32">): Drive => ({ kind: "jolt", after, speed });

export class PendulumError extends Error {}

export type Pendulum = {
  /** Its own swings per second, and seconds per swing. */
  readonly hz: number; readonly period: number;
  /** A sway's steady answer in plain numbers: radians of swing per unit of the pivot's sway, and seconds late. */
  readonly gain: (period: number) => { readonly perUnit: number; readonly late: number };
  /** The angle from plumb at clock t, radians: the sum of its answers to the drives. */
  readonly answer: (t: In<"f32">, drives: readonly Drive[]) => F;
};

/**
 * A hanging thing's swing: a bob on an arm under gravity, lightly damped, hung from a pivot something else moves.
 *
 * The shape and its law. The small-swing damped driven oscillator above: psi'' + 2 zeta w0 psi' + w0^2 psi = -x_p'' / L.
 * Its answer is linear in the drives, so `answer(t, drives)` sums the closed-form answer to each: a sway's steady state,
 * a jolt's ringing. How fast it swings follows from length and gravity; how far from how hard the pivot moves.
 *
 * Parameters: `length`, `gravity`, `damping` are fixed numbers (they decide the natural frequency on the CPU); a sway's
 * amplitude and a jolt's time and speed are expressions, so the drives can come from any clock.
 *
 * What it cannot do, because of the law:
 * - large swings: the law is linearised, so a swing past about half a radian is too regular and never goes over the top.
 * - a pivot path that is neither a steady sway nor a sudden change of speed (a sway that starts, a smooth push).
 * - more than one degree of freedom: no swing in two planes at once (a conical swing), no double pendulum, no chain.
 * - a bob that hits something, or a length that changes.
 *
 * Going past it: the drives are a sum type summed by `answer`, so a new motion of the pivot is a sum of the two kinds:
 * a pivot path that is piecewise linear in time is exactly a jolt at each key (the change of speed there), so any keyed
 * path is a list of `jolt`s; a sway that fades in is a sway times a ramp plus the jolt of its start. Relatives: two
 * planes are two pendulums on the same pivot, one per axis (exact for small swings); a stalk that bends by a pose is
 * `bent`, whose `bow` and `crook` can be driven by a pendulum's angle each, the crook later than the bow (whip).
 */
export function pendulum(spec: PendulumSpec): Pendulum {
  const { length: L, gravity: g, damping: zeta } = spec;
  if (!(L > 0 && Number.isFinite(L))) throw new PendulumError(`pendulum: length must be a positive number, got ${L}`);
  if (!(g > 0 && Number.isFinite(g))) throw new PendulumError(`pendulum: gravity must be a positive number, got ${g}`);
  if (!(zeta >= 0 && zeta < 1)) throw new PendulumError(`pendulum: damping must be in [0, 1) (at 1 and over it does not swing), got ${zeta}`);
  const w0 = Math.sqrt(g / L), wd = w0 * Math.sqrt(1 - zeta * zeta);
  const gain = (period: number) => {
    if (!(period > 0 && Number.isFinite(period))) throw new PendulumError(`pendulum: a sway's period must be a positive number of seconds, got ${period}`);
    const w = (2 * Math.PI) / period, re = w0 * w0 - w * w, im = 2 * zeta * w0 * w;
    return { perUnit: (w * w) / L / Math.hypot(re, im), late: Math.atan2(im, re) / w };
  };
  const answer = (t: In<"f32">, drives: readonly Drive[]): F => drives.map((d): F => {
    // The one place a drive's kind is read.
    switch (d.kind) {
      case "sway": {
        const k = gain(d.period), w = (2 * Math.PI) / d.period;
        return mul(mul(d.amplitude, k.perUnit), sin(add(mul(lift<"f32">(t), w), (d.phase ?? 0) - k.late * w)));
      }
      case "jolt": {
        const s = max(d.after, 0);
        const ring = mul(exp(mul(s, -zeta * w0)), sin(mul(s, wd)));
        return mul(mul(mul(d.speed, -1 / (L * wd)), ring), select(lt(lift<"f32">(d.after), 0), 0, 1));
      }
    }
  }).reduce((acc, x) => add(acc, x), lift<"f32">(0));
  return { hz: w0 / (2 * Math.PI), period: (2 * Math.PI) / w0, gain, answer };
}
