// Restless motion: what a twitchy creature does between its actions (docs/animation_principles.md, "Temperament").
//
// `alive` is a slow breath: it never arrives anywhere. A nervous thing does the opposite: it is still, then somewhere
// else, at intervals that are never quite even, and it overshoots when it gets there. `dart` is that as a closed form
// of the clock: which hold it is in, which it came from, and how far the jump between them has gone. `tremor` is the
// shake to lay over it. Neither keeps state between frames.
//
// Mine: the uneven clock (a whole-number count of holds per loop on a clock warped by one slow sine, so the holds are
// uneven and the sequence still loops) and the choice to return indices, not a value: the caller picks what each
// hold means (a place to look, a pose), so two parts can share one rhythm or have one each by the seed.

import { add, clamp, floor, fract, lift, mod, mul, sin, sub, vec3, type Expr, type In } from "../expr.ts";
import { hash3 } from "../lib/noise.ts";
import { overshot } from "./lively.ts";

type F = Expr<"f32">;
const TAU = Math.PI * 2;

export type DartSpec = {
  /** Seconds a hold lasts on average, and seconds a jump takes (less than `every`). */
  readonly every: number; readonly snap: number;
  /** Seconds after which the whole sequence repeats: a whole number of holds is fitted into it. */
  readonly period: number;
  /** Which sequence: parts with different seeds jump at different instants to different things. */
  readonly seed: number;
  /** How uneven the holds are: 0 a metronome, 0.4 (the default) some twice as long as others. Below 0.45. */
  readonly uneven?: number;
  /** How far past its mark a jump runs before settling (`overshot`'s back). Default 2.2. */
  readonly past?: number;
};

export type Dart = {
  /** This hold's number and the last one's: whole numbers in [0, holds). Hash them (`chosen`) to choose what each means. */
  readonly now: F; readonly before: F;
  /** How far the jump from `before` to `now` has gone: 0 at the jump, past 1 and back to 1 by `snap` seconds on. */
  readonly arrive: F;
  /** Seconds since the jump (about: the clock is warped). */
  readonly since: F;
  /** How many holds one period has. */
  readonly holds: number;
};

export class RestlessError extends Error {}

export function dart(t: In<"f32">, o: DartSpec): Dart {
  const uneven = o.uneven ?? 0.4;
  if (!(o.every > 0 && o.snap > 0 && o.snap < o.every)) throw new RestlessError(`dart: need 0 < snap < every (seconds), got snap ${o.snap}, every ${o.every}`);
  if (!(o.period >= o.every)) throw new RestlessError(`dart: period (${o.period}) must be at least one hold (${o.every})`);
  if (!(uneven >= 0 && uneven < 0.45)) throw new RestlessError(`dart: uneven must be in [0, 0.45) (at 0.5 the clock stops), got ${uneven}`);
  const holds = Math.max(2, Math.round(o.period / o.every));
  // The warp: one sine of about three holds' length, a whole number of turns per period. The clock's rate is
  // (holds / period) (1 + uneven 2 pi turns / holds cos(...)): positive while uneven 2 pi turns / holds < 1, and
  // turns / holds is about 1 / 3, so uneven < 0.45 keeps it running forward.
  const turns = Math.max(1, Math.round(holds / 3.1));
  if (uneven * TAU * turns / holds >= 0.95) throw new RestlessError(`dart: with ${holds} holds a period, uneven ${uneven} would stop the clock; use at most ${(0.9 * holds / (TAU * turns)).toFixed(2)}`);
  const c = add(mul(lift<"f32">(t), holds / o.period), mul(sin(add(mul(lift<"f32">(t), (TAU * turns) / o.period), o.seed * 1.7)), uneven));
  const k = floor(c);
  const since = mul(fract(c), o.period / holds);
  return { now: mod(k, holds), before: mod(sub(k, 1), holds), arrive: overshot(clamp(mul(since, 1 / o.snap), 0, 1), o.past ?? 2.2), since, holds };
}

/** A number in [0, 1) for a hold, fixed for that hold: hash a dart's `now` or `before` with a seed of your own. */
export const chosen = (hold: In<"f32">, seed: number): F => hash3(vec3(hold, seed, 5.3));

/** A shake: two fast sines that never line up, in -1..1, so it shivers instead of humming. Scale it by a weight. */
export const tremor = (t: In<"f32">, hz: number, seed = 0): F =>
  add(mul(sin(add(mul(lift<"f32">(t), hz * TAU), seed * 2.1)), 0.6), mul(sin(add(mul(lift<"f32">(t), hz * 1.37 * TAU), seed * 3.3 + 1)), 0.4));
