// Afloat: a rigid placement on the water whose height, pitch and roll come from the water under the hull, so a ship
// rides the swell (or a whirlpool's funnel wall) with no animation code of its own.
//
// The water is sampled at four points: bow and stern (half the length either side of the centre, along the heading)
// and port and starboard (half the beam either side). The origin sits at the mean of the four heights; the bow axis
// climbs by the bow-to-stern difference over the length (pitch), the starboard axis by the starboard-to-port
// difference over the beam (roll). A wind on the sails heels the ship further, toward leeward, in proportion to how
// much of the wind comes across it.
//
// Local coordinates of the placed thing: x toward the bow, y up, z to starboard (= x cross y), origin on the waterline
// amidships.

import { add, cos, cross, dot, lift, mul, normalize, sin, sub, substitute, time, vec2, vec3, type Expr, type In } from "../expr.ts";
import { rotate } from "../map/chart.ts";
import type { Chart, F, Frame, P3 } from "../map/types.ts";

type V2 = Expr<"vec2">;

/**
 * Water something can float on: the sea, or a deformation of it in its own coordinates (`pool.in(sea)`). `carry` takes
 * a point given in the water's own plane coordinates to the sea plane now, with how far a thing there has been turned;
 * `height` is the surface height at a sea-plane point.
 */
export interface Waters {
  carry(at: V2): { readonly xz: V2; readonly turn: F };
  height(xz: V2): F;
}

export type AfloatSpec = {
  /** Where, in the water's own plane coordinates (x, z). */
  readonly at: In<"vec2">;
  /** Heading of the bow, radians from +x toward +z, in the water's coordinates (it turns with the water). */
  readonly heading: In<"f32">;
  /** The span the water is sampled over: length (bow to stern) and beam. */
  readonly length: number;
  readonly beam: number;
  /** How high the local origin sits above the mean water level. Default 0. */
  readonly lift?: number;
  /**
   * Wind on the sails: the direction it blows toward (x, z) and the heel, radians, it gives when it comes straight
   * across. The heel is toward leeward and breathes a little with gusts. Default no wind.
   */
  readonly wind?: { readonly direction: readonly [number, number]; readonly heel: number };
};

/** The placement's frame, as expressions: origin on the waterline amidships, x the bow, y up, z starboard. */
export function afloatFrame(w: Waters, spec: AfloatSpec): Frame {
  if (!(spec.length > 0 && spec.beam > 0)) throw new Error(`afloat: length and beam must be positive, got ${spec.length}, ${spec.beam}`);
  const c = w.carry(lift<"vec2">(spec.at));
  const h = add(lift<"f32">(spec.heading), c.turn);
  const fwd = vec2(cos(h), sin(h)), stbd = vec2(mul(sin(h), -1), cos(h));
  const at = (d: V2, k: number): F => w.height(add(c.xz, mul(d, k)));
  const bow = at(fwd, spec.length / 2), stern = at(fwd, -spec.length / 2);
  const star = at(stbd, spec.beam / 2), port = at(stbd, -spec.beam / 2);
  const level = mul(add(add(bow, stern), add(star, port)), 0.25);
  const x = normalize(vec3(fwd.x, div0(sub(bow, stern), spec.length), fwd.y));
  const side = normalize(vec3(stbd.x, div0(sub(star, port), spec.beam), stbd.y));
  // Orthonormal, x exact: z is the starboard direction made perpendicular to the bow, y = z cross x.
  const z0 = normalize(sub(side, mul(x, dot(side, x))));
  const y0 = cross(z0, x);
  const lean = heelOf(spec, stbd);
  const y = add(mul(y0, cos(lean)), mul(z0, sin(lean))), z = sub(mul(z0, cos(lean)), mul(y0, sin(lean)));
  return { o: vec3(c.xz.x, add(level, spec.lift ?? 0), c.xz.y), x, y, z };
}

const div0 = (a: F, k: number): F => mul(a, 1 / k);

/** No wind: nothing heels the hull (the direction is arbitrary). */
const CALM = { direction: [1, 0], heel: 0 } as const;

/** Heel to starboard, radians: the wind's component toward starboard times the heel, with a slow gust. */
function heelOf(spec: AfloatSpec, stbd: V2): F {
  const wind = spec.wind ?? CALM;
  const l = Math.hypot(wind.direction[0], wind.direction[1]);
  if (!(l > 0)) throw new Error("afloat: the wind needs a direction");
  const across = add(mul(stbd.x, wind.direction[0] / l), mul(stbd.y, wind.direction[1] / l));
  const gust = add(1, mul(add(sin(mul(time, 0.53)), mul(sin(mul(time, 1.37)), 0.5)), 0.12));
  return mul(mul(across, wind.heel), gust);
}

/** A rigid chart in the frame g: local l goes to o + l.x X + l.y Y + l.z Z; a slice substitutes time in the frame. */
export function rigid(g: Frame): Chart {
  return {
    frame: (l) => ({ o: add(g.o, rotate(g, l)), x: g.x, y: g.y, z: g.z }),
    at: (t) => {
      const [o, x, y, z] = substitute([g.o, g.x, g.y, g.z], time, t) as P3[];
      return rigid({ o, x, y, z });
    },
  };
}

/** Float something on the water: a rigid placement that rides the surface under it. */
export function afloat(w: Waters, spec: AfloatSpec): Chart {
  return rigid(afloatFrame(w, spec));
}
