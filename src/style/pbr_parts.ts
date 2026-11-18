// The parts a physically based look is built from (`pbr`, src/style/pbr.ts): each a plain function over expressions,
// so it runs on the GPU and in the JS twin alike and ports to another engine as the maths it is. Any one can be handed
// to `pbr({ ... })` in place of the default, or called from a `shade` of your own.
//
// Conventions shared by every part:
// - Directions are unit vectors; cosines (n_l, n_v, n_h, v_h) are clamped to [0, 1] by the caller.
// - `rough` is perceptual roughness as a paint gives it, 0 (mirror) to 1; a part squares it (alpha = rough^2) itself,
//   floored at rough 0.05 so the GGX peak stays finite in f32.
// - Light: an arrival's `amount` 1 is the sun at noon, the irradiance under which a white Lambert surface facing it
//   has radiance 1. So a term is radiance per unit amount, the BRDF times pi times n.l.
// - Colours are linear until `tone`, which makes them the picture's display values.
//
// Inherited: GGX (Walter et al. 2007), height-correlated Smith visibility (Heitz 2014), Schlick's Fresnel, the analytic
// split-sum DFG (Karis 2014), the Narkowicz fit of ACES, irradiance from L2 spherical harmonics (Ramamoorthi and
// Hanrahan 2001). New here: the environment from a closed-form sky projected once per scene, with the reflection's
// sharp part a direct tap of the same sky, blurred by roughness through per-band attenuation (`skyLight`).

import { add, compileJs, div, dot, evaluate, exp, f, max, mix, mul, normalize, pow, saturate, smoothstep, sqrt, sub, variable, vec3 } from "../expr.ts";
import type { F, P3 } from "../map/types.ts";

/** alpha = rough^2, rough floored at 0.05. */
const alphaOf = (rough: F): F => { const r = max(rough, 0.05); return mul(r, r); };

/**
 * The GGX (Trowbridge-Reitz) distribution of microfacet normals: D = alpha^2 / (pi ((n.h)^2 (alpha^2 - 1) + 1)^2).
 * Integrates to 1 against n.h over the hemisphere. Cannot: anisotropy (one alpha; brushed metal wants two, along the
 * body's own m and around it). Relatives: Beckmann (a shorter tail); a sum of two GGX lobes (a clear coat's haze).
 */
export function ggx(n_h: F, rough: F): F {
  const a = alphaOf(rough), a2 = mul(a, a);
  const d = add(mul(mul(n_h, n_h), sub(a2, 1)), 1);
  return div(a2, mul(Math.PI, mul(d, d)));
}

/**
 * Smith's visibility for GGX, height-correlated: Vis = G / (4 n.l n.v) =
 * 0.5 / (n.l sqrt(n.v^2 (1 - alpha^2) + alpha^2) + n.v sqrt(n.l^2 (1 - alpha^2) + alpha^2)).
 * Cannot: account for light that scatters between microfacets more than once (energy is lost at high roughness: the
 * lobe's albedo face on is 1.00 at rough 0.25, 0.92 at 0.5, 0.31 at 1; agent_notes/2026-11-18_pbr_style/NOTE.md).
 */
export function smithGgx(n_l: F, n_v: F, rough: F): F {
  const a = alphaOf(rough), a2 = mul(a, a), k = sub(1, a2);
  const gv = mul(n_l, sqrt(add(mul(mul(n_v, n_v), k), a2)));
  const gl = mul(n_v, sqrt(add(mul(mul(n_l, n_l), k), a2)));
  return div(0.5, max(add(gv, gl), 1e-5));
}

/**
 * Schlick's Fresnel: F = f0 + (1 - f0)(1 - v.h)^5, per colour channel. `f0` is the reflectance face on: 0.04 for most
 * dielectrics, the colour itself for a metal. Relatives: a thin film (interference per wavelength), conductor Fresnel
 * from n and k, Schlick with an f90 below 1 (a dusty surface).
 */
export function schlick(v_h: F, f0: P3): P3 {
  const x = sub(1, v_h), x2 = mul(x, x);
  return add(f0, mul(sub(1, f0), mul(mul(x2, x2), x)));
}

/**
 * Lambert: the diffuse colour times n.l (the pi of the BRDF cancels the amount's). `n_v`, `v_h` and `rough` are for a
 * replacement that reads them: Burley's retro-reflection, Oren-Nayar's flattening of a rough body.
 */
export function lambert(albedo: P3, n_l: F, _n_v: F, _v_h: F, _rough: F): P3 {
  return mul(albedo, n_l);
}

/**
 * ACES filmic, as Narkowicz's fit: x (2.51 x + 0.03) / (x (2.43 x + 0.59) + 0.14), clamped to [0, 1], then encoded for
 * the picture (x^(1/2.2), the target is display sRGB). Radiance 1 shows as 0.91, 0.25 as 0.64; nothing clips below
 * about 10. Cannot: hold hue at the top (a bright saturated light goes toward white per channel, as film does).
 * Relatives: Reinhard x / (1 + x); AgX; the identity for measuring radiance (`(c) => c`).
 */
export function aces(c: P3): P3 {
  const x = max(c, 0);
  const y = saturate(div(mul(x, add(mul(x, 2.51), 0.03)), add(mul(x, add(mul(x, 2.43), 0.59)), 0.14)));
  return pow(y, 1 / 2.2);
}

const ZENITH = vec3(0.1, 0.22, 0.5);
const HORIZON = vec3(0.5, 0.58, 0.66);
const BELOW = vec3(0.2, 0.19, 0.17);
const GLOW = vec3(0.35, 0.27, 0.16);

/**
 * A clear day's sky, linear radiance along `dir` with the sun toward `sun`: zenith (0.10, 0.22, 0.50) to horizon
 * (0.50, 0.58, 0.66) by the square root of the height, a warm glow round the sun ((n.sun)^8 x (0.35, 0.27, 0.16)),
 * and a ground (0.20, 0.19, 0.17) below the horizon, blended over 0.08 of height: about what a sunlit floor of
 * albedo 0.2 sends up, so a mirror ball shows a floor rather than a hole. Its irradiance on a level surface is about a
 * third of the sun's. No sun disc: the sun reaches a body as a light, and a disc here would count
 * it twice. Relatives: a sunset (the glow wide and red, the zenith dim); an overcast sky (one grey, no glow); a studio
 * of softboxes as bright patches of `dir`.
 */
export function daylight(dir: P3, sun: P3): P3 {
  const up = dir.y;
  const above = add(mix(HORIZON, ZENITH, sqrt(saturate(up))), mul(GLOW, pow(saturate(dot(dir, sun)), 8)));
  const below = mix(HORIZON, BELOW, saturate(mul(up, -5)));
  return mix(below, above, smoothstep(-0.08, 0, up));
}

/** A sky: linear radiance along a direction, given the direction toward the sun. */
type SkyLaw = PbrParts["sky"];

/** The slots of `pbr` (src/style/pbr.ts), each a function; `pbr(parts)` takes any of them in place of its default. */
export type PbrParts = {
  /** The distribution of microfacet normals at n.h (`ggx`). */
  readonly distribution: (n_h: F, rough: F) => F;
  /** Shadowing and masking over 4 n.l n.v (`smithGgx`). */
  readonly visibility: (n_l: F, n_v: F, rough: F) => F;
  /** The share reflected at v.h, per colour, from the reflectance face on (`schlick`). */
  readonly fresnel: (v_h: F, f0: P3) => P3;
  /** The body's light per unit arrival, cosine included (`lambert`). */
  readonly diffuse: (albedo: P3, n_l: F, n_v: F, v_h: F, rough: F) => P3;
  /**
   * The sky's light in a body facing `normal` (the shading normal; `view` toward the eye): what a white diffuse surface
   * reflects, and what a mirror of `rough` sees along the reflected ray, before the sun's amount (`skyLight`).
   */
  readonly environment: (at: { readonly normal: P3; readonly view: P3; readonly sun: P3 }, rough: F, sky: (dir: P3, sun: P3) => P3) => { readonly diffuse: P3; readonly specular: P3 };
  /** The sky, linear radiance along a direction given the sun's (`daylight`): the background and the environment. */
  readonly sky: (dir: P3, sun: P3) => P3;
  /** Linear radiance to the picture's display values (`aces`). */
  readonly tone: (c: P3) => P3;
};

// ---- the sky projected on spherical harmonics ----------------------------------------------------------------------

/** The nine real SH basis functions of degree <= 2 at a unit direction, in the order l = 0; 1 (y, z, x); 2. */
const basis = (x: number, y: number, z: number): number[] => [
  0.282095, 0.488603 * y, 0.488603 * z, 0.488603 * x,
  1.092548 * x * y, 1.092548 * y * z, 0.315392 * (3 * z * z - 1), 1.092548 * x * z, 0.546274 * (x * x - y * y),
];
const basisOf = (d: P3): F[] => [
  f(0.282095), mul(d.y, 0.488603), mul(d.z, 0.488603), mul(d.x, 0.488603),
  mul(mul(d.x, d.y), 1.092548), mul(mul(d.y, d.z), 1.092548), mul(sub(mul(mul(d.z, d.z), 3), 1), 0.315392), mul(mul(d.x, d.z), 1.092548), mul(sub(mul(d.x, d.x), mul(d.y, d.y)), 0.546274),
];
/** The band (l) of each of the nine. */
const BAND = [0, 1, 1, 1, 2, 2, 2, 2, 2] as const;
const DIRECTIONS = 1024;

const projections = new WeakMap<SkyLaw, Map<string, readonly (readonly [number, number, number])[]>>();
/**
 * The sky's nine SH coefficients (one rgb each), by quadrature over 1024 Fibonacci directions on the twin, at the sun
 * `sun` evaluates to now (a host input at its default) and time 0: once per (sky, sun), cached.
 */
function projected(sky: SkyLaw, sun: P3): readonly (readonly [number, number, number])[] {
  const s = evaluate(sun) as number[];
  const key = s.join(",");
  let bySun = projections.get(sky);
  if (bySun === undefined) { bySun = new Map(); projections.set(sky, bySun); }
  const had = bySun.get(key);
  if (had !== undefined) return had;
  const dir = variable("pbr_sky_dir", "vec3");
  let at: (d: number[]) => number[];
  try {
    const fn = compileJs([sky(dir, vec3(s[0]!, s[1]!, s[2]!))]);
    at = (d) => fn({ vars: { pbr_sky_dir: d }, inputs: {}, params: {} })[0] as number[];
  } catch (e) {
    throw new Error(`pbr: the sky does not evaluate on the CPU, where it is projected once per scene (${(e as Error).message}). Fix: write the sky without raw WGSL, or pass an \`environment\` that does not project it`, { cause: e });
  }
  const c = Array.from({ length: 9 }, () => [0, 0, 0]);
  const golden = Math.PI * (3 - Math.sqrt(5)), w = (4 * Math.PI) / DIRECTIONS;
  for (let i = 0; i < DIRECTIONS; i++) {
    const y = 1 - (2 * (i + 0.5)) / DIRECTIONS, r = Math.sqrt(1 - y * y), phi = i * golden;
    const x = r * Math.cos(phi), z = r * Math.sin(phi);
    const L = at([x, y, z]);
    basis(x, y, z).forEach((b, k) => { for (let ch = 0; ch < 3; ch++) c[k]![ch]! += L[ch]! * b * w; });
  }
  const out = c.map((v) => [v[0]!, v[1]!, v[2]!] as const);
  bySun.set(key, out);
  return out;
}

/** sum_k weight(band k) L_k Y_k(d): the projected sky with each band scaled. */
function shAt(L: readonly (readonly [number, number, number])[], d: P3, weight: (band: 0 | 1 | 2) => F | number): P3 {
  const y = basisOf(d);
  let acc: P3 = vec3(0, 0, 0);
  L.forEach((c, k) => { acc = add(acc, mul(vec3(c[0], c[1], c[2]), mul(y[k]!, weight(BAND[k]!)))); });
  return acc;
}

/**
 * Light from the sky in a body: `diffuse`, the radiance a white Lambert surface facing `normal` reflects (the sky's
 * irradiance over pi), and `specular`, the sky as a mirror of roughness `rough` sees it along the reflected view ray.
 *
 * The law: the sky is projected on spherical harmonics of degree <= 2 once per scene (`projected`). `diffuse` is that
 * projection convolved with the cosine lobe (bands scaled 1, 2/3, 1/4). `specular` is
 *   S(r) = sum_l a_l L_l Y_l(r) + a_3 (sky(r) - sum_l L_l Y_l(r)),  a_l = exp(-l (l + 1) alpha^2):
 * the low bands of the sky blurred as a lobe of GGX width blurs them (a spherical Gaussian of sharpness 1 / (2
 * alpha^2) round r, whose band-l attenuation is a_l), and the rest of the sky, the part the projection does not hold,
 * tapped once at r and faded as band 3 would be. Exactly sky(r) at rough 0; exactly the blurred projection when rough
 * is large. Both are scaled by nothing: the caller multiplies by the sun's amount and the DFG.
 *
 * Cannot: follow a sky that changes with time or a moved sun except through the one tap (the projection is at the
 * stated sun and time 0, as a cloud's tables are); see the ground, other bodies or the body itself (a cavity is lit by
 * open sky: no occlusion); blur a sharp sky feature smoothly at middle roughness (it fades as a whole rather than
 * spreading). Costs about 90 multiply-adds and one sky evaluation a pixel; 1024 sky evaluations on the CPU at compile.
 *
 * Relatives: a second tap along the lobe's tangent for a smoother middle roughness (as `glossed` does with four); an
 * occlusion term from the body's own thickness; a projection per frame for a moving sun (a table, written each frame).
 */
export function skyLight(at: { readonly normal: P3; readonly view: P3; readonly sun: P3 }, rough: F, sky: (dir: P3, sun: P3) => P3): { readonly diffuse: P3; readonly specular: P3 } {
  const L = projected(sky, at.sun);
  const n = at.normal, v = at.view;
  const r = sub(mul(n, mul(dot(n, v), 2)), v);
  const diffuse = max(shAt(L, n, (l) => [1, 2 / 3, 1 / 4][l]!), 0);
  const a = alphaOf(rough), a2 = mul(a, a);
  const fade = (l: number): F => exp(mul(a2, -l * (l + 1)));
  const low = shAt(L, r, (l) => fade(l));
  const all = shAt(L, r, () => 1);
  const specular = max(add(low, mul(sub(sky(normalize(r), at.sun), all), fade(3))), 0);
  return { diffuse, specular };
}
