// Lively motion: the twelve principles of animation (Johnston and Thomas, The Illusion of Life, 1981) as formulas.
// docs/animation_principles.md says what each principle is and which helper serves it. Everything here is a pure
// function of a clock, so it runs wherever pose runs and keeps no state between frames.
//
// What was already in the library and is used as is: `response` (a damped spring's answer to a step: follow through,
// overshoot, and anticipation with r < 0), `retarded` (the clock a part further along a body reads: drag), `pulse`,
// `ease`, `breathe` (a moving hold), `strike`, `lookAt`.

import { add, clamp, cos, div, exp, lift, max, min, mix, mul, pow, select, lt, sin, sqrt, sub, vec3, type Expr, type In } from "../expr.ts";
import { hash3 } from "../lib/noise.ts";
import { size } from "../map/chart.ts";
import type { Size } from "../map/types.ts";

type F = Expr<"f32">;
type P3 = Expr<"vec3">;
const TAU = Math.PI * 2;

// ---- timing: an action is three beats -------------------------------------------------------------------------------

/** How long each beat of an action lasts, seconds. An action with no wind-up or no settle gives that beat 0. */
export type Beats = { readonly anticipate: number; readonly act: number; readonly settle: number };

/** Where a clock is in each beat: 0 before it, 1 after it, rising across it. `since` is seconds since the action began. */
export type Beat = {
  /** The wind-up: the audience is told what is coming. */
  readonly anticipate: F;
  /** The action itself. */
  readonly act: F;
  /** After the action: loose parts catch up and come to rest. */
  readonly settle: F;
  /** Seconds since the action ended its `act` beat (negative before). Feed it to `response` for follow through. */
  readonly after: F;
};

/**
 * Timing. Split one clock into wind-up, action and settle. Fewer seconds in `act` reads as fast and light, more as
 * slow and heavy; a long `anticipate` before a short `act` is what makes a hit read as a hit.
 */
export function beats(since: In<"f32">, b: Beats): Beat {
  for (const [k, v] of Object.entries(b)) if (!(v >= 0 && Number.isFinite(v))) throw new Error(`beats: ${k} must be a finite number of seconds, at least 0; got ${v}`);
  if (!(b.act > 0)) throw new Error(`beats: act must be positive (an action takes time); got ${b.act}`);
  const t = lift<"f32">(since);
  const span = (from: number, len: number): F => (len > 0 ? clamp(div(sub(t, from), len), 0, 1) : select(lt(t, from), 0, 1));
  return {
    anticipate: span(0, b.anticipate),
    act: span(b.anticipate, b.act),
    settle: span(b.anticipate + b.act, b.settle),
    after: sub(t, b.anticipate + b.act),
  };
}

// ---- slow in and slow out, anticipation, overshoot: shapes of 0..1 -------------------------------------------------------

/** Slow in and slow out. 0..1 with zero speed at both ends; `power` 2 is gentle, 4 holds the extremes longer. */
export function slowInOut(u: In<"f32">, power = 2): F {
  if (!(power >= 1)) throw new Error(`slowInOut: power must be at least 1, got ${power}`);
  const x = clamp(u, 0, 1);
  const rise = mul(0.5, pow(mul(x, 2), power)), fall = sub(1, mul(0.5, pow(mul(sub(1, x), 2), power)));
  return select(lt(x, 0.5), rise, fall);
}

/** Slow out only: starts at speed, arrives gently. For something already moving that comes to rest. */
export const slowOut = (u: In<"f32">, power = 2): F => sub(1, pow(sub(1, clamp(u, 0, 1)), power));
/** Slow in only: leaves gently, arrives at speed. For a blow that lands. */
export const slowIn = (u: In<"f32">, power = 2): F => pow(clamp(u, 0, 1), power);

/**
 * Anticipation. 0..1 that first goes the wrong way: it dips by about `back * 0.06` (for back = 1.7, to -0.1), then
 * drives through to 1. Use it for the action itself when the wind-up and the action are one move.
 */
export function anticipated(u: In<"f32">, back = 1.7): F {
  const x = clamp(u, 0, 1);
  return mul(mul(x, x), sub(mul(back + 1, x), back));
}

/** Overshoot. 0..1 that runs past 1 (to about 1 + back * 0.06) and comes back. For an arrival with weight behind it. */
export function overshot(u: In<"f32">, back = 1.7): F {
  const y = sub(clamp(u, 0, 1), 1);
  return add(1, mul(mul(y, y), add(mul(back + 1, y), back)));
}

// ---- squash and stretch ---------------------------------------------------------------------------------------------

/**
 * Squash and stretch, with volume kept. `along` is the length factor on the copy's y axis: above 1 stretches, below 1
 * squashes; the other two axes take 1 / sqrt(along), so the volume does not change.
 */
export function squash(along: In<"f32">): Size {
  const s = max(along, 1e-3), across = div(1, sqrt(s));
  return size.axes(vec3(across, s, across));
}

/**
 * The length factor a moving thing should wear: 1 at rest, longer the faster it goes, never beyond `most`. `speed` is
 * in the thing's own lengths per second. Pair it with `squash` and an impact squash at the stop.
 */
export const stretchOf = (speed: In<"f32">, per = 0.15, most = 1.6): F => min(add(1, mul(per, speed)), most);

/** The squash at an impact: `depth` at the moment of contact (`after` = 0), sprung back out by `after` seconds later. */
export function impact(after: In<"f32">, depth = 0.35, hz = 3, damping = 0.35): F {
  const t = max(after, 0), w = hz * TAU;
  const ring = mul(exp(mul(t, -damping * w)), cos(mul(t, w * Math.sqrt(Math.max(1 - damping * damping, 1e-4)))));
  return sub(1, mul(mul(depth, ring), select(lt(lift<"f32">(after), 0), 0, 1)));
}

// ---- arcs ---------------------------------------------------------------------------------------------------------------

/** Arc. From a to b along an arch, not a line: `lift` is how far the middle stands off along `up`. */
export function arc(a: In<"vec3">, b: In<"vec3">, u: In<"f32">, lift_: In<"f32">, up: In<"vec3"> = [0, 1, 0]): P3 {
  const x = clamp(u, 0, 1);
  return add(mix(lift<"vec3">(a), lift<"vec3">(b), x), mul(lift<"vec3">(up), mul(lift_, mul(4, mul(x, sub(1, x))))));
}

// ---- follow through, overlap, drag, secondary action ----------------------------------------------------------------------

/**
 * Follow through. A loose part's swing after its parent stops: 0 before `after` = 0, then a decaying swing of
 * `amount` that dies away. Add it to the part's angle or offset. Heavier, floppier parts take a lower `hz`.
 */
export function swing(after: In<"f32">, amount: In<"f32">, hz = 1.4, damping = 0.25): F {
  const t = max(after, 0), w = hz * TAU, wd = w * Math.sqrt(Math.max(1 - damping * damping, 1e-4));
  return mul(mul(amount, mul(exp(mul(t, -damping * w)), sin(mul(t, wd)))), select(lt(lift<"f32">(after), 0), 0, 1));
}

/**
 * Overlap and drag. The clock a part reads when it trails the lead by `lag` seconds per unit of `along` (distance
 * down a tail, index down a chain). The same motion formula read on this clock arrives later the further along.
 */
export const dragged = (t: In<"f32">, along: In<"f32">, lag: In<"f32">): F => sub(t, mul(along, lag));

/** Secondary action. A smaller, later echo of a main motion `f` of the clock: `f(t - delay) * gain`. */
export const echo = (f: (t: F) => F, t: In<"f32">, delay: In<"f32">, gain: In<"f32">): F => mul(f(sub(t, delay)), gain);

// ---- the moving hold, exaggeration, twins ----------------------------------------------------------------------------------

/**
 * The moving hold. A still pose kept alive: two slow swings that never line up, in -1..1 times `amount`. `seed` gives
 * each part its own, so nothing breathes in step.
 */
export function alive(t: In<"f32">, amount: In<"f32">, seed: In<"f32"> = 0, hz = 0.23): F {
  const s = lift<"f32">(seed), ph = mul(hash3(vec3(s, 17.3, 4.1)), TAU);
  const a = sin(add(mul(t, hz * TAU), ph)), b = sin(add(mul(t, hz * TAU * 1.618), mul(ph, 2.3)));
  return mul(amount, add(mul(a, 0.62), mul(b, 0.38)));
}

/** Exaggeration. Push a value away from its rest by `by`: 1 leaves it, 2 doubles the departure. */
export const exaggerated = (x: In<"f32">, rest: In<"f32">, by: In<"f32">): F => add(rest, mul(sub(x, rest), by));

/**
 * Against twins. A factor near 1 that differs per part (and so per side): multiply a size, an angle or a clock rate by
 * it and the left and right of a pair stop mirroring each other. `spread` 0.15 gives 0.85..1.15.
 */
export const uneven = (part: In<"f32">, spread = 0.15): F => add(1, mul(sub(mul(hash3(vec3(part, 3.7, 9.2)), 2), 1), spread));
