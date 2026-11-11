// A ring wave: what a slam leaves on the water (docs/throw_and_shove.md). One crest and the trough behind it leave
// the edge of a disc at an instant and run outward at a fixed speed; ahead of the front the water is exactly as it
// was. It is a deformation of the sea (`sea({ deform: [ring] })`), so everything that reads the sea's height rides it:
// a ship placed with `afloat` rises over the crest and heels on its faces with no animation of its own. Things afloat
// are also shoved outward. What the wave gives a thing is an impulse when its front arrives (`impulse`: a change of
// velocity, units per second, outward); `pushed` is Loft's stand-in for a game's boat taking that impulse under a
// linear drag, a closed form of the clock and the thing's own place.
//
// Three facts the formulas are built on:
//   1. A ring's energy is spread round a growing circle, so its height falls as 1 / sqrt(r).
//   2. In water shallower than the pulse is long the wave does not disperse and runs at sqrt(g depth): the pulse keeps
//      its width, and the front that left radius R at the slam is at R + c t. Nothing moves before it arrives.
//   3. A floating thing cannot be pushed faster than the wave that pushes it: the impulse at the disc's edge is at
//      most the wave's speed (checked when the ring is made).

import { add, and, atan2, clamp, div, exp, gt, length, lift, lt, max, mul, select, sin, smoothstep, sqrt, sub, time, type Expr, type In } from "../expr.ts";
import type { SeaDeform } from "./sea.ts";

type F = Expr<"f32">;
type V2 = Expr<"vec2">;

export type RingSpec = {
  /** The disc the wave leaves: its centre on the sea plane (x, z) and its radius. */
  readonly centre: In<"vec2">;
  readonly radius: In<"f32">;
  /** Seconds since the slam at clock t (negative before it). */
  readonly since: (t: In<"f32">) => F;
  /** Speed of the front, world units per second (shallow water: sqrt(g depth)). */
  readonly speed: number;
  /** Height of the crest at the disc's edge; it falls as 1 / sqrt(r) beyond. */
  readonly height: number;
  /** Length of the pulse from its front to the end of its trough, world units. */
  readonly length: number;
  /** Distance from the centre at which the wave is gone (it fades over the last quarter of the way). */
  readonly reach: number;
  /** How far a thing afloat at the disc's edge ends up shoved outward; it falls as 1 / sqrt(r) too. */
  readonly shove: number;
  /**
   * Seconds for the water to take off the speed the front gave (1 / e): the stand-in's linear drag. The impulse at the
   * disc's edge is `shove / drag` units per second.
   */
  readonly drag: number;
};

/** The crest's share of the pulse's length, and the trough's depth as a share of the crest's height. */
const CREST = 0.5, TROUGH = 0.3;

export class RingWave implements SeaDeform {
  readonly rise: number;
  private readonly centre: V2;
  private readonly radius: F;

  constructor(readonly spec: RingSpec) {
    const { speed, height, length: len, reach, shove, drag } = spec;
    for (const [k, v] of Object.entries({ speed, height, length: len, reach, drag })) if (!(v > 0 && Number.isFinite(v))) throw new Error(`ringWave: ${k} must be a positive, finite number; got ${v}`);
    if (!(shove >= 0 && Number.isFinite(shove))) throw new Error(`ringWave: shove must be a finite distance, at least 0; got ${shove}`);
    // Fact 3. The impulse is greatest at the disc's edge (it falls with distance as the offset does).
    const fastest = shove / drag;
    if (!(fastest <= speed)) {
      throw new Error(`ringWave: a thing at the disc's edge would be given ${fastest.toFixed(2)} units/s, faster than the wave (${speed.toFixed(2)}): shove at most ${(speed * drag).toFixed(2)}, or drag at least ${(shove / speed).toFixed(2)} s`);
    }
    this.rise = height;
    this.centre = lift<"vec2">(spec.centre);
    this.radius = lift<"f32">(spec.radius);
  }

  /** Where a sea-plane point is in the pulse at time t: its distance out, how far behind the front (0..1 of the pulse), and whether the pulse is on it. */
  private at(xz: V2, t: In<"f32">): { readonly r: F; readonly xi: F; readonly on: Expr<"bool"> } {
    const r = length(sub(xz, this.centre)), tau = this.spec.since(t);
    const xi = div(sub(add(this.radius, mul(tau, this.spec.speed)), r), this.spec.length);
    return { r, xi, on: and(and(gt(tau, 0), lt(r, this.spec.reach)), and(gt(xi, 0), lt(xi, 1))) };
  }

  /** The share of the edge's height (and shove) left at distance r: 1 / sqrt(r) from the disc's edge, to nothing at `reach`. */
  private spread(r: F): F {
    const fade = sub(1, smoothstep(0.75 * this.spec.reach, this.spec.reach, r));
    return mul(sqrt(div(this.radius, max(r, this.radius))), fade);
  }

  /** The pulse's shape at xi in (0, 1) behind the front: a crest of height 1, then a shallower trough; flat at both ends and between. */
  private shape(xi: F): F {
    const x = clamp(xi, 0, 1);
    const crest = sin(mul(x, Math.PI / CREST)), trough = sin(mul(sub(x, CREST), Math.PI / (1 - CREST)));
    return select(lt(x, CREST), mul(crest, crest), mul(mul(trough, trough), -TROUGH));
  }

  /** How far the wave lifts the surface at sea-plane point xz at time t (below 0 in its trough; exactly 0 off the pulse). */
  lifts(xz: In<"vec2">, t: In<"f32">): F {
    const p = lift<"vec2">(xz), w = this.at(p, t);
    return select(w.on, mul(mul(this.shape(w.xi), this.spread(w.r)), this.spec.height), 0);
  }

  /** Seconds after the slam that the front reaches sea-plane point xz (0 inside the disc). */
  arrival(xz: In<"vec2">): F {
    return div(max(sub(length(sub(lift<"vec2">(xz), this.centre)), this.radius), 0), this.spec.speed);
  }

  /**
   * What the front gives a thing afloat at `place` when it arrives there: a change of velocity on the sea plane (x, z),
   * world units per second, outward. `shove / drag` at the disc's edge, falling as 1 / sqrt(r), nothing past `reach`.
   * A game adds it to its boat's velocity at `arrival(place)` after the slam and lets its own drag do the rest.
   */
  impulse(place: In<"vec2">): V2 {
    const p = lift<"vec2">(place), d = sub(p, this.centre), r = length(d);
    return mul(div(d, max(r, 1e-4)), mul(this.spread(r), this.spec.shove / this.spec.drag));
  }

  /**
   * Loft's stand-in for a boat that takes the impulse: how far a thing afloat whose own place is `place` has been
   * shoved at time t, on the sea plane (x, z). It is the integral of `impulse(place) exp(-s / drag)` from the front's
   * arrival: `impulse drag (1 - exp(-s / drag))`, s seconds after it. Exactly 0 until then.
   */
  pushed(place: In<"vec2">, t: In<"f32">): V2 {
    const s = max(sub(this.spec.since(t), this.arrival(place)), 0);
    return mul(this.impulse(place), mul(sub(1, exp(mul(s, -1 / this.spec.drag))), this.spec.drag));
  }

  /** Where the stand-in ends: `impulse(place) drag`. */
  shoved(place: In<"vec2">): V2 { return mul(this.impulse(place), this.spec.drag); }

  // ---- as a deformation of the sea: it moves no water sideways and calms nothing; it lifts the surface and marks it ----

  toLocal(xz: V2): V2 { return xz; }
  calm(): F { return lift<"f32">(1); }
  dip(xz: V2, t: In<"f32">): F { return mul(this.lifts(xz, t), -1); }

  /**
   * Foam along the crest's front face, broken into dashes round the ring and thinning as the wave weakens; the crest
   * shaded lighter and the trough darker (in the swell's crest units).
   */
  look(xz: V2, t: In<"f32">): { readonly foam: F; readonly shade: F; readonly lean: V2 } {
    const w = this.at(xz, t), d = sub(xz, this.centre), strength = this.spread(w.r);
    const band = mul(smoothstep(0.03, 0.1, w.xi), sub(1, smoothstep(0.2, 0.3, w.xi)));
    const angle = atan2(d.y, d.x);
    const dash = add(0.5, mul(0.5, sin(add(mul(angle, 23), mul(3, sin(add(mul(angle, 5), mul(w.xi, 9))))))));
    const foam = mul(mul(band, add(0.4, mul(dash, 0.6))), sqrt(strength));
    // `lean`: the crest breaks outward (an added readout for a style that prints the foam as curled heads).
    return { foam: select(w.on, foam, 0), shade: select(w.on, mul(mul(this.shape(w.xi), strength), 1.6), 0), lean: div(d, max(w.r, 1e-4)) };
  }

  /** On the pulse: after the slam, between the front and the end of the trough, inside `reach`. Off it every part is exactly nothing. */
  reaches(xz: V2): Expr<"bool"> { return this.at(xz, time).on; }
}

/** A ring wave (see `RingWave`). */
export function ringWave(spec: RingSpec): RingWave {
  return new RingWave(spec);
}
