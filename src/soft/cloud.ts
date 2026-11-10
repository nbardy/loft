// Clouds from a handful of numbers: a billowing mass of Gaussian puffs in a shape of revolution, three tiers of puff
// size (a few big ones fill the core, medium ones make the lobes, many small ones roughen the surface: the cauliflower
// look of cumulus), a flattened base, boiling from noise and a drift. Placement is a pure function of the seed (an
// integer hash, no randomness), and the motion is a closed-form expression of time.

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
   * Half-width as a share of size, against height as a share (0 base, 1 top), piecewise linear. A cumulus bulges in the
   * middle; a thunderhead is a narrow tower under a wide anvil.
   */
  readonly profile: readonly (readonly [number, number])[];
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

/** A billowing mass of puffs. */
export function cloud(spec: CloudSpec): Cloud {
  if (!(Number.isInteger(spec.blobs) && spec.blobs >= 1)) throw new Error(`cloud: blobs must be a positive integer, got ${spec.blobs}`);
  if (spec.profile.length < 2) throw new Error("cloud: the profile needs at least two keys");
  for (let k = 1; k < spec.profile.length; k++) if (!(spec.profile[k][0] > spec.profile[k - 1][0])) throw new Error("cloud: profile heights must increase");
  const [W, H, Dp] = spec.size;
  const half = Math.max(W, Dp) / 2;
  // The tiers' radii are calibrated for TIER_BLOBS puffs; with fewer (cheaper: cost per pixel and the light pass both
  // grow with the count) each is larger by the cube root, so the puffs fill the same volume, and thinner by as much, so
  // a puff's optical depth through its middle (density times radius), and with it the cloud's shading, stays the same.
  // Thinner still (keeping the cloud's mass) went pale and see-through; checked in tests/soft.storm.test.ts.
  const fill = Math.cbrt(TIER_BLOBS / spec.blobs);
  const puffs: Puff[] = [];
  let lo = [Infinity, Infinity, Infinity], hi = [-Infinity, -Infinity, -Infinity];
  let start = 0;
  TIERS.forEach((tier, ti) => {
    const n = ti === TIERS.length - 1 ? spec.blobs - start : Math.max(1, Math.round(spec.blobs * tier.share));
    for (let j = 0; j < n; j++) {
      const i = start + j;
      const u = (k: number) => hashUnit(spec.seed, i, k);
      // Height: stratified within the tier so every level gets puffs; the core stays off the very top and bottom.
      const h = ti === 0 ? 0.15 + 0.7 * ((j + u(0)) / n) : (j + u(0)) / n;
      const R = profileAt(spec.profile, h);
      const theta = u(1) * Math.PI * 2;
      const rho = tier.out[0] + (tier.out[1] - tier.out[0]) * Math.sqrt(u(2));
      const r = R * half * tier.radius * fill * (0.75 + 0.5 * u(3));
      // Flatter toward the base (a level, dark bottom) and in the anvil (a wide, thin sheet).
      const flat = h < 0.12 ? 0.55 : R > 0.8 ? 0.6 : 1;
      const ry = Math.min(r * flat, H * 0.3);
      const x = spec.at[0] + Math.cos(theta) * rho * R * (W / 2);
      const z = spec.at[2] + Math.sin(theta) * rho * R * (Dp / 2);
      // Never below the base: the base is a level line.
      const y = Math.max(spec.at[1] + h * H, spec.at[1] + ry * 0.9);
      const base = [x, y, z];
      // Boiling: each puff wanders through a noise field of its own; radii breathe with another.
      const seedOff = [u(4) * 97, u(5) * 89, u(6) * 83];
      const clock = mul(time, spec.billow);
      const wander = (axis: number): Expr<"f32"> => noise3(vec3(seedOff[axis], add(clock, axis * 13.7), seedOff[(axis + 1) % 3]));
      const reach = spec.turbulence * r;
      const centre = add(add(vec3(...base), vec3(mul(wander(0), reach), mul(wander(1), reach * 0.6), mul(wander(2), reach))), mul(vec3(...spec.drift), time));
      const breathe = add(1, mul(noise3(vec3(seedOff[2], add(clock, 41.3), seedOff[0])), 0.25 * spec.turbulence));
      const tint = 0.92 + 0.08 * u(7);
      puffs.push(blob({ centre, radii: mul(vec3(r, ry, r), breathe), density: (spec.density * tier.density) / fill, colour: spec.colour.map((c) => c * tint) as [number, number, number] }));
      const m = [r, ry, r].map((x) => 3.2 * x * (1 + 0.25 * spec.turbulence) + reach);
      for (let a = 0; a < 3; a++) {
        const end = base[a] + spec.drift[a] * spec.seconds;
        lo[a] = Math.min(lo[a], base[a] - m[a], end - m[a]);
        hi[a] = Math.max(hi[a], base[a] + m[a], end + m[a]);
      }
    }
    start += n;
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
