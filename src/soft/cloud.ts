// Clouds: masses of Gaussian puffs that boil and drift, in closed form of time. `cloud` places them in a shape of
// revolution from a handful of numbers; `billows` boils and drifts any puffs you place yourself.

import { add, mul, time, vec3, type Expr } from "../expr.ts";
import { noise3 } from "../lib/noise.ts";
import { blob, type Box, type Puff } from "./medium.ts";

export type Rgb = readonly [number, number, number];

export type CloudSpec = {
  /** Centre of the cloud's base (its lowest level). */
  readonly at: readonly [number, number, number];
  /** Width (x), height (y), depth (z) of the mass. */
  readonly size: readonly [number, number, number];
  /**
   * Half-width as a share of size, against height as a share (0 base, 1 top): keys [height, share], piecewise linear,
   * or a function of (height, bearing in radians round the axis), so the mass need not be round. A cumulus bulges in
   * the middle; a thunderhead is a narrow tower under a wide anvil.
   */
  readonly profile: readonly (readonly [number, number])[] | ((h: number, bearing: number) => number);
  /** Number of puffs. Cost per pixel grows with it (see DESIGN.md 8). */
  readonly blobs: number;
  /** Boiling: how far puffs wander, as a share of their radius (0 still, 0.5 lively). */
  readonly turbulence: number;
  /** How fast the boiling changes, noise cells per second. */
  readonly billow: number;
  /** World units per second. */
  readonly drift: readonly [number, number, number];
  /** Peak extinction of the core puffs, per world unit. */
  readonly density: number;
  readonly colour: Rgb;
  readonly seed: number;
  /** The bounds cover the drift for this many seconds from time 0. */
  readonly seconds: number;
};

/** What a cloud builder returns: its puffs, and the box they stay inside for `seconds`. */
export type Cloud = { readonly puffs: readonly Puff[]; readonly bounds: Box };

/** A hash of (seed, i, k) to [0, 1): integer arithmetic only (pure, the same everywhere). */
export function hashUnit(seed: number, i: number, k: number): number {
  let h = Math.imul(seed ^ 0x9e3779b9, 0x85ebca6b) ^ Math.imul(i + 0x632be5ab, 0xc2b2ae35) ^ Math.imul(k + 0x27d4eb2f, 0x165667b1);
  h = Math.imul(h ^ (h >>> 15), 0x2c1b3c6d);
  h = Math.imul(h ^ (h >>> 12), 0x297a2d39);
  return ((h ^ (h >>> 15)) >>> 8) / 16777216;
}

function profileAt(profile: readonly (readonly [number, number])[], h: number): number {
  if (h <= profile[0][0]) return profile[0][1];
  for (let k = 1; k < profile.length; k++) {
    const [h1, r1] = profile[k], [h0, r0] = profile[k - 1];
    if (h <= h1) return r0 + ((r1 - r0) * (h - h0)) / (h1 - h0);
  }
  return profile[profile.length - 1][1];
}

/** The profile as a function of (height, bearing), classified once. */
function profileOf(p: CloudSpec["profile"]): (h: number, bearing: number) => number {
  if (typeof p === "function") return p;
  if (p.length < 2) throw new Error("cloud: the profile needs at least two keys");
  for (let k = 1; k < p.length; k++) if (!(p[k][0] > p[k - 1][0])) throw new Error("cloud: profile heights must increase");
  return (h) => profileAt(p, h);
}

/** Tiers: share of the puffs, radius as a share of the cloud's half-width, how far out they sit, density share. */
// Radii are shares of the local half-width (the profile at the puff's height), so a narrow tower gets small puffs.
// The surface tier is the densest: a Gaussian's visible edge is sharper the larger its peak optical depth, so dense
// small puffs on the outside make a crisp, lumpy silhouette, while a thinner core only has to fill the inside.
const TIER_BLOBS = 260;
const TIERS = [
  { share: 0.1, radius: 0.34, out: [0.0, 0.4], density: 0.6 },
  { share: 0.3, radius: 0.2, out: [0.4, 0.78], density: 1.0 },
  { share: 0.6, radius: 0.11, out: [0.76, 1.0], density: 2.6 },
] as const;

/**
 * A billowing mass of puffs: a cumulus, a thunderhead, a bank of smoke or steam.
 *
 * The shape and its law. `blobs` Gaussian puffs fill a shape of revolution about a vertical axis: at height h (a share
 * of `size`'s height) the mass reaches `profile(h)` of its half-width. The puffs come in three tiers, a few big ones in
 * the core, more middling ones in the lobes, many small dense ones on the surface (the cauliflower look), stratified by
 * height; puffs at the base and in a wide anvil are flattened, and none goes below the base. Each puff then boils in a
 * noise field of its own and the whole drifts (`billows`). Placement is a pure function of `seed` (an integer hash).
 *
 * Parameters: all numbers, fixed at build, and `profile` (keys, or a function of height and bearing). The motion is a
 * closed form of time; nothing else moves.
 *
 * What it cannot do, because of the law:
 * - a mass that is not a star-shaped solid about one vertical axis: no hole (a smoke ring), no arch, no overhang
 *   under a lobe, no second tower.
 * - follow a curve: no trail, no column bent by wind, no plume from a chimney.
 * - change shape in time beyond boiling in place: it does not grow, tower up or tear apart.
 * - tiers or flattening of your own: the cumulus recipe is fixed.
 *
 * Going past it is one step down: `billows(places, motion)` boils and drifts any list of puffs, each a place, radii and
 * a density, so the shape is whatever produces the list. Relatives:
 * - a smoke ring: places round a circle in a vertical plane (a torus of puffs), drifting along its axis.
 * - a trail or plume: places along any curve (a ship's smoke, a whale's spout bent by wind), radii growing along it.
 * - puffs on a surface: places at points of a body (steam off a back, spray on a crest).
 * - a different law entirely: `blob` is one puff and `medium` integrates any list of them; layers and banks
 *   (`layer`, `bank`) are the flat relatives.
 */
export function cloud(spec: CloudSpec): Cloud {
  if (!(Number.isInteger(spec.blobs) && spec.blobs >= 1)) throw new Error(`cloud: blobs must be a positive integer, got ${spec.blobs}`);
  const profile = profileOf(spec.profile);
  const [W, H, Dp] = spec.size;
  const half = Math.max(W, Dp) / 2;
  // The tiers' radii are calibrated for TIER_BLOBS puffs; with fewer (cheaper: cost per pixel and the light pass both
  // grow with the count) each is larger by the cube root, so the puffs fill the same volume, and thinner by as much, so
  // a puff's optical depth through its middle (density times radius), and with it the cloud's shading, stays the same.
  // Thinner still (keeping the cloud's mass) went pale and see-through; checked in tests/soft.storm.test.ts.
  const fill = Math.cbrt(TIER_BLOBS / spec.blobs);
  const places: PuffPlace[] = [];
  let start = 0;
  TIERS.forEach((tier, ti) => {
    const n = ti === TIERS.length - 1 ? spec.blobs - start : Math.max(1, Math.round(spec.blobs * tier.share));
    for (let j = 0; j < n; j++) {
      const i = start + j;
      const u = (k: number) => hashUnit(spec.seed, i, k);
      // Height: stratified within the tier so every level gets puffs; the core stays off the very top and bottom.
      const h = ti === 0 ? 0.15 + 0.7 * ((j + u(0)) / n) : (j + u(0)) / n;
      const theta = u(1) * Math.PI * 2;
      const R = profile(h, theta);
      const rho = tier.out[0] + (tier.out[1] - tier.out[0]) * Math.sqrt(u(2));
      const r = R * half * tier.radius * fill * (0.75 + 0.5 * u(3));
      // Flatter toward the base (a level, dark bottom) and in the anvil (a wide, thin sheet).
      const flat = h < 0.12 ? 0.55 : R > 0.8 ? 0.6 : 1;
      const ry = Math.min(r * flat, H * 0.3);
      const x = spec.at[0] + Math.cos(theta) * rho * R * (W / 2);
      const z = spec.at[2] + Math.sin(theta) * rho * R * (Dp / 2);
      // Never below the base: the base is a level line.
      const y = Math.max(spec.at[1] + h * H, spec.at[1] + ry * 0.9);
      places.push({ at: [x, y, z], radii: [r, ry, r], density: (spec.density * tier.density) / fill });
    }
    start += n;
  });
  return billows(places, spec);
}

/** One puff before it moves: its centre, its 1/e radii per axis and its peak extinction per world unit. */
export type PuffPlace = { readonly at: V3; readonly radii: V3; readonly density: number };
/** How puffs boil and drift: `CloudSpec`'s motion and colour. */
export type BillowSpec = Pick<CloudSpec, "turbulence" | "billow" | "drift" | "colour" | "seed" | "seconds">;
type V3 = readonly [number, number, number];

/**
 * Puffs that boil and drift, from places you choose: `cloud`'s motion without its shape (see `cloud` for the relatives).
 * Puff i wanders through a noise field of its own (seeded by `seed` and i) by `turbulence` of its radius, breathes, is
 * tinted a little, and drifts with the rest; `bounds` covers them for `seconds`.
 */
export function billows(places: readonly PuffPlace[], motion: BillowSpec): Cloud {
  const puffs: Puff[] = [];
  const lo = [Infinity, Infinity, Infinity], hi = [-Infinity, -Infinity, -Infinity];
  places.forEach((p, i) => {
    const u = (k: number) => hashUnit(motion.seed, i, k);
    const base = p.at, [r] = p.radii;
    // Boiling: each puff wanders through a noise field of its own; radii breathe with another.
    const seedOff = [u(4) * 97, u(5) * 89, u(6) * 83];
    const clock = mul(time, motion.billow);
    const wander = (axis: number): Expr<"f32"> => noise3(vec3(seedOff[axis], add(clock, axis * 13.7), seedOff[(axis + 1) % 3]));
    const reach = motion.turbulence * r;
    const centre = add(add(vec3(...base), vec3(mul(wander(0), reach), mul(wander(1), reach * 0.6), mul(wander(2), reach))), mul(vec3(...motion.drift), time));
    const breathe = add(1, mul(noise3(vec3(seedOff[2], add(clock, 41.3), seedOff[0])), 0.25 * motion.turbulence));
    const tint = 0.92 + 0.08 * u(7);
    puffs.push(blob({ centre, radii: mul(vec3(...p.radii), breathe), density: p.density, colour: motion.colour.map((c) => c * tint) as [number, number, number] }));
    const m = p.radii.map((x) => 3.2 * x * (1 + 0.25 * motion.turbulence) + reach);
    for (let a = 0; a < 3; a++) {
      const end = base[a] + motion.drift[a] * motion.seconds;
      lo[a] = Math.min(lo[a], base[a] - m[a], end - m[a]);
      hi[a] = Math.max(hi[a], base[a] + m[a], end + m[a]);
    }
  });
  return { puffs, bounds: { min: lo as unknown as [number, number, number], max: hi as unknown as [number, number, number] } };
}

export type ThunderheadSpec = {
  /** Centre of the base. */
  readonly at: readonly [number, number, number];
  /** From base to the top of the anvil. */
  readonly height: number;
  /** Width of the anvil. */
  readonly width: number;
  readonly blobs: number;
  readonly seed: number;
  readonly drift: readonly [number, number, number];
  readonly seconds: number;
};

/** A thunderhead: a broad dark-based tower under a wide anvil, boiling slowly. */
export function thunderhead(t: ThunderheadSpec): Cloud {
  return cloud({
    at: t.at, size: [t.width, t.height, t.width * 0.8],
    profile: [[0, 0.5], [0.12, 0.6], [0.45, 0.44], [0.7, 0.42], [0.84, 0.72], [0.93, 1.0], [1, 0.8]],
    blobs: t.blobs, turbulence: 0.35, billow: 0.12, drift: t.drift, density: 0.9, colour: [1, 1, 1], seed: t.seed, seconds: t.seconds,
  });
}
