// Motion helpers, closed form in time (or in a 0..1 progress input): no state, so any of them can be read at another
// time for a telegraph or a smear, and superpose.

import { add, atan2, clamp, cos, dot, length, lift, mul, sin, smoothstep, sub, TAU, time, type In } from "../expr.ts";
import type { F, Frame, P3 } from "../map/types.ts";
import { unit } from "./limb.ts";

export type LookSpec = {
  /** The frame looking: its x is straight ahead, y up, z to the side. */
  readonly from: Frame;
  readonly target: In<"vec3">;
  /** Largest turn either way, radians: sideways (about y) and up or down (about z). */
  readonly yaw: number;
  readonly pitch: number;
};

/** The direction toward a target as yaw and pitch, each held within its limit, and the unit direction they give. */
export function lookAt(spec: LookSpec): { readonly yaw: F; readonly pitch: F; readonly dir: P3 } {
  const f = spec.from, d = sub(lift<"vec3">(spec.target), f.o);
  const ahead = dot(d, f.x), side = dot(d, f.z), up = dot(d, f.y);
  const yaw = clamp(atan2(side, ahead), -spec.yaw, spec.yaw);
  const flat = length(add(mul(f.x, ahead), mul(f.z, side)));
  const pitch = clamp(atan2(up, flat), -spec.pitch, spec.pitch);
  const dir = add(mul(f.y, sin(pitch)), mul(add(mul(f.x, cos(yaw)), mul(f.z, sin(yaw))), cos(pitch)));
  return { yaw, pitch, dir: unit(dir) };
}

export type BreathSpec = { readonly rate: In<"f32">; readonly depth: In<"f32">; readonly phase?: In<"f32">; readonly t?: In<"f32"> };

/**
 * A breathing scale, 1 at rest out to 1 + depth: the breath in is quicker than the breath out (the phase is skewed,
 * theta + 0.4 sin theta), so it reads as breathing and not as a pulse.
 */
export function breathe(spec: BreathSpec): F {
  const th = mul(add(mul(spec.t ?? time, spec.rate), spec.phase ?? 0), TAU);
  const skewed = add(th, mul(sin(th), 0.4));
  return add(1, mul(spec.depth, mul(sub(1, cos(skewed)), 0.5)));
}

export type BeatSpec = {
  /** Beats per second. */
  readonly rate: In<"f32">;
  /** Peak angle either side of `bias`, radians. */
  readonly amplitude: In<"f32">;
  /** Seconds the tip (r = 1) runs behind the root (r = 0). */
  readonly lag: In<"f32">;
  /** Mean angle, radians. Default 0. */
  readonly bias?: In<"f32">;
  /** About the share of the cycle the downstroke takes (0.5 symmetric; below, a quick downstroke and a slow recovery), 0.25 to 0.75. Default 0.5. */
  readonly down?: number;
  readonly t?: In<"f32">;
};

/**
 * A wing beat: the angle at r in [0, 1] along the wing. Each point reads the beat on a clock delayed by `lag * r`
 * (as `retarded` does along a body), so the tip trails the root and the wing flexes through each stroke. +angle is up.
 */
export function beat(r: In<"f32">, spec: BeatSpec): F {
  const down = spec.down ?? 0.5;
  if (!(down >= 0.25 && down <= 0.75)) throw new Error(`beat: down must be in [0.25, 0.75], got ${down}`);
  const th = mul(sub(spec.t ?? time, mul(spec.lag, r)), mul(lift<"f32">(spec.rate), TAU));
  // A phase warp psi = theta + c sin(theta) moves the top (psi = pi / 2) earlier by about c and the bottom later by
  // about c, so the downstroke takes about (pi + 2c) / 2 pi of the cycle; |c| < 1 keeps the warp monotone.
  const c = (down - 0.5) * Math.PI;
  return add(spec.bias ?? 0, mul(spec.amplitude, sin(add(th, mul(sin(th), c)))));
}

export type StrikeSpec = {
  /** How far the wind-up goes the wrong way, as a share of the strike. */
  readonly anticipation: In<"f32">;
  /** How far past the hit the follow-through carries, as a share of the strike. */
  readonly follow: In<"f32">;
  /** Progress at which the wind-up is full, the hit lands, the follow-through peaks and the recovery starts. */
  readonly timing?: readonly [number, number, number, number];
};

/**
 * A strike envelope over its progress s in [0, 1]: 0 at rest, down to -anticipation (the wind-up), up to 1 (the hit),
 * on to 1 + follow (follow-through), then back to 0 by s = 1. Driven by an input from 0 to 1, so the game decides when
 * and how fast; every phase is a smoothstep, so the pose eases in and out of each.
 */
export function strike(s: In<"f32">, spec: StrikeSpec): F {
  const [a, i, f, r] = spec.timing ?? [0.35, 0.5, 0.6, 0.75];
  if (!(0 < a && a < i && i < f && f < r && r < 1)) throw new Error(`strike: timing must increase within (0, 1), got [${a}, ${i}, ${f}, ${r}]`);
  const x = lift<"f32">(s), A = lift<"f32">(spec.anticipation);
  const windup = mul(A, smoothstep(0, a, x));
  const hit = mul(add(A, 1), smoothstep(a, i, x));
  const over = mul(spec.follow, sub(smoothstep(i, f, x), smoothstep(f, r, x)));
  return sub(add(sub(hit, windup), over), smoothstep(r, 1, x));
}
