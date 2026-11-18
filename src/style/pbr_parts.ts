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

import { SceneError } from "../errors.ts";
import { add, compileJs, div, dot, evaluate, exp, f, leaves, max, min, mix, mul, normalize, pow, saturate, smoothstep, sqrt, sub, variable, vec3, type Expr } from "../expr.ts";
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

/**
 * The slots of `pbr` (src/style/pbr.ts); `pbr(parts)` takes any of them in place of its default. Each is a function
 * but `clear`, a colour.
 */
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
  /**
   * The sky, linear radiance along a direction given the sun's (`daylight`): the background and the environment. It is
   * radiance per unit of the sun's amount (`pbr` multiplies it by the amount). Projected once when the style is built,
   * so it may read its direction, the sun and constants: one that reads time or a param is refused (`PbrError`).
   */
  readonly sky: (dir: P3, sun: P3) => P3;
  /** Linear radiance to the picture's display values (`aces`). */
  readonly tone: (c: P3) => P3;
  /**
   * The specular lobe's directional albedo with Fresnel split out, the environment's specular weight: under Schlick
   * the lobe reflects f0 `scale` + `bias` of a uniform sky (the split sum's A and B). Default: integrated from the
   * given `distribution` and `visibility` on the twin when the style is built (about 30 ms, cached per pair; a fit
   * to the default pair needs none), so a swapped lobe reflects the sky as it reflects the lights.
   */
  readonly lobeAlbedo: (n_v: F, rough: F) => { readonly scale: F; readonly bias: F };
  /**
   * The clear colour, display values 0 to 1: what shows where nothing is drawn. Default: the sky on the horizon under
   * a sun overhead, toned, which needs the sky and the tone map to evaluate on the CPU; give it for a sky that cannot.
   */
  readonly clear: readonly [number, number, number];
};

/** Why `pbr` cannot build from its parts (exit 4: the parts as written must change). */
export type PbrFailure =
  /** A part evaluated once at build time reads something that changes after it: the projection would be stale. */
  | { readonly kind: "varies"; readonly part: "sky" | "distribution" | "visibility"; readonly reads: string }
  /** A part evaluated at build time has no CPU twin (raw WGSL, a per-frame table). */
  | { readonly kind: "gpu-only"; readonly part: "sky" | "tone" | "distribution" | "visibility"; readonly detail: string }
  /** The integrated lobe albedo is not a smooth enough surface for the environment's fit. */
  | { readonly kind: "unfitted"; readonly worst: number };

const failureParts = (x: PbrFailure): [string, string, string, string] => {
  switch (x.kind) {
    case "varies": return [`pbr ${x.part}`, `a ${x.part} that reads ${x.reads}`, `a ${x.part} of its arguments and constants alone: it is evaluated once, when the style is built`,
      x.part === "sky" ? "make the sky a function of its direction and the sun alone, or pass `clear: [r, g, b]` and an `environment` that does not project the sky" : `make the ${x.part} a function of its arguments alone, or pass \`lobeAlbedo\` for it`];
    case "gpu-only": return [`pbr ${x.part}`, `a ${x.part} the CPU cannot evaluate (${x.detail})`, `a ${x.part} the JS twin evaluates: it is evaluated once, when the style is built`,
      x.part === "sky" || x.part === "tone" ? "write it without raw WGSL; or pass `clear: [r, g, b]` and an `environment` that does not project the sky" : "write it without raw WGSL, or pass `lobeAlbedo` for it"];
    case "unfitted": return ["pbr lobeAlbedo", `a distribution and visibility whose albedo the environment's fit misses by ${x.worst.toFixed(3)}`, "a fit within 0.02", "pass `lobeAlbedo: (n_v, rough) => ({ scale, bias })` for these parts"];
  }
};

/** `pbr` refused its parts, by name (`PbrFailure`). */
export class PbrError extends SceneError {
  constructor(readonly failure: PbrFailure) {
    const [where, given, expected, fix] = failureParts(failure);
    super(where, given, expected, fix);
  }
}

/** What in an expression changes after build time: the first input (time is one) or param it reads, or none. */
function variesBy(e: Expr): string | undefined {
  const { inputs, params } = leaves([e]);
  if (inputs.length > 0) return inputs[0]!.name === "time" ? "time" : `the input "${inputs[0]!.name}"`;
  if (params.length > 0) return `the param "${params[0]!.name}"`;
  return undefined;
}

/** A part's expression compiled for the twin, or a typed refusal. */
function onCpu(part: "sky" | "tone" | "distribution" | "visibility", roots: readonly Expr[]): ReturnType<typeof compileJs> {
  const refuse = (e: unknown): never => { throw new PbrError({ kind: "gpu-only", part, detail: (e as Error).message }); };
  let fn: ReturnType<typeof compileJs>;
  try { fn = compileJs(roots); } catch (e) { return refuse(e); }
  // A per-frame table compiles and fails only when read: that is the same refusal.
  return (env) => { try { return fn(env); } catch (e) { return refuse(e); } };
}

/** The default clear colour: the sky on the horizon under a sun overhead, toned (see `PbrParts.clear`). */
export function horizonClear(sky: SkyLaw, tone: (c: P3) => P3): readonly [number, number, number] {
  const sky0 = sky(vec3(1, 0, 0), vec3(0, 1, 0));
  const reads = variesBy(sky0);
  if (reads !== undefined) throw new PbrError({ kind: "varies", part: "sky", reads });
  const c = onCpu("tone", [tone(sky0)])({ vars: {}, inputs: {}, params: {} })[0] as number[];
  return [c[0]!, c[1]!, c[2]!];
}

// ---- the lobe's albedo, integrated -----------------------------------------------------------------------------------

/** Degrees of the fit in sqrt(n.v) and rough, and the grid it is fitted on. */
const FIT_MU = 6, FIT_R = 5, GRID = 16;
const fitted = new WeakMap<object, WeakMap<object, readonly [readonly number[], readonly number[]]>>();

/**
 * The split sum's (A, B) of a distribution and visibility, by quadrature on the twin: A = int D Vis n.l (1 - (1 - v.h)^5),
 * B = int D Vis n.l (1 - v.h)^5 over light directions, at `GRID` x `GRID` points of n.v and rough in [0.05, 1]. The
 * integral is taken over half vectors (dl = 4 v.h dh), stratified by GGX's own distribution at that roughness (32 x 16
 * points), so a sharp lobe is resolved; any `distribution` enters as its ratio to GGX. Then each of A and B is fitted by
 * least squares with a polynomial of degree 6 in sqrt(n.v) and 5 in rough, and the fit is checked at the midpoints
 * between grid points: off by more than 0.02 is refused, not used.
 */
function integrated(distribution: PbrParts["distribution"], visibility: PbrParts["visibility"]): readonly [readonly number[], readonly number[]] {
  const had = fitted.get(distribution)?.get(visibility);
  if (had !== undefined) return had;
  const nh = variable("pbr_n_h", "f32"), nl = variable("pbr_n_l", "f32"), nv = variable("pbr_n_v", "f32"), r = variable("pbr_rough", "f32");
  const d = distribution(nh, r), vis = visibility(nl, nv, r);
  for (const [part, e] of [["distribution", d], ["visibility", vis]] as const) {
    const reads = variesBy(e);
    if (reads !== undefined) throw new PbrError({ kind: "varies", part, reads });
  }
  const dFn = onCpu("distribution", [d]), vFn = onCpu("visibility", [vis]);
  const at = (mu: number, rough: number): [number, number] => {
    const a = Math.max(rough, 0.05) ** 2, a2 = a * a, vx = Math.sqrt(1 - mu * mu);
    const NU = 32, NP = 16;
    let A = 0, B = 0;
    for (let k = 0; k < NU; k++) for (let p = 0; p < NP; p++) {
      const u = (k + 0.5) / NU, phi = (2 * Math.PI * (p + 0.5)) / NP;
      const c = Math.sqrt((1 - u) / (1 + (a2 - 1) * u)), s = Math.sqrt(1 - c * c);
      const hx = s * Math.cos(phi), v_h = vx * hx + mu * c;
      const n_l = 2 * v_h * c - mu;
      if (v_h <= 0 || n_l <= 0) continue;
      const ggxD = a2 / (Math.PI * (c * c * (a2 - 1) + 1) ** 2);
      const D = dFn({ vars: { pbr_n_h: c, pbr_rough: rough }, inputs: {}, params: {} })[0] as number;
      const V = vFn({ vars: { pbr_n_l: n_l, pbr_n_v: mu, pbr_rough: rough }, inputs: {}, params: {} })[0] as number;
      const w = (D * V * n_l * 4 * v_h) / (ggxD * c) / (NU * NP), fc = (1 - v_h) ** 5;
      A += w * (1 - fc); B += w * fc;
    }
    return [A, B];
  };
  const node = (i: number): number => 0.05 + (0.95 * i) / (GRID - 1);
  const rows: { mu: number; r: number; ab: [number, number] }[] = [];
  for (let i = 0; i < GRID; i++) for (let j = 0; j < GRID; j++) rows.push({ mu: node(i), r: node(j), ab: at(node(i), node(j)) });
  const coefficients = [0, 1].map((k) => leastSquares(rows.map((x) => ({ mu: x.mu, r: x.r, y: x.ab[k]! }))));
  let worst = 0;
  for (let i = 0; i < GRID - 1; i++) for (let j = 0; j < GRID - 1; j++) {
    const mu = node(i + 0.5), rough = node(j + 0.5), [A, B] = at(mu, rough);
    const fa = polyAt(coefficients[0]!, mu, rough), fb = polyAt(coefficients[1]!, mu, rough);
    if (!Number.isFinite(A + B)) throw new PbrError({ kind: "unfitted", worst: Infinity });
    worst = Math.max(worst, Math.abs(fa + fb - A - B), Math.abs(fb - B));
  }
  if (!(worst <= 0.02)) throw new PbrError({ kind: "unfitted", worst });
  const out = [coefficients[0]!, coefficients[1]!] as const;
  let byVis = fitted.get(distribution);
  if (byVis === undefined) { byVis = new WeakMap(); fitted.set(distribution, byVis); }
  byVis.set(visibility, out);
  return out;
}

/** The fit's basis at (n.v, rough): sqrt(n.v)^i rough^j, i <= FIT_MU, j <= FIT_R. */
const basisAt = (mu: number, r: number): number[] => {
  const s = Math.sqrt(mu), b: number[] = [];
  for (let i = 0; i <= FIT_MU; i++) for (let j = 0; j <= FIT_R; j++) b.push(s ** i * r ** j);
  return b;
};
const polyAt = (c: readonly number[], mu: number, r: number): number => basisAt(mu, r).reduce((acc, b, k) => acc + b * c[k]!, 0);

/** Least squares by the normal equations, solved by Gauss-Jordan with partial pivoting (42 unknowns). */
function leastSquares(data: readonly { mu: number; r: number; y: number }[]): number[] {
  const n = (FIT_MU + 1) * (FIT_R + 1);
  const m = Array.from({ length: n }, () => new Array<number>(n + 1).fill(0));
  for (const x of data) {
    const b = basisAt(x.mu, x.r);
    for (let i = 0; i < n; i++) { for (let j = 0; j < n; j++) m[i]![j]! += b[i]! * b[j]!; m[i]![n]! += b[i]! * x.y; }
  }
  for (let c = 0; c < n; c++) {
    let p = c;
    for (let i = c + 1; i < n; i++) if (Math.abs(m[i]![c]!) > Math.abs(m[p]![c]!)) p = i;
    [m[c], m[p]] = [m[p]!, m[c]!];
    // An all-zero lobe (`distribution: () => 0`) leaves the right-hand side zero: every coefficient is 0.
    if (m[c]![c] === 0) continue;
    for (let i = 0; i < n; i++) if (i !== c) { const k = m[i]![c]! / m[c]![c]!; for (let j = c; j <= n; j++) m[i]![j]! -= k * m[c]![j]!; }
  }
  return m.map((row, i) => (row[i] === 0 ? 0 : row[n]! / row[i]!));
}

/** The fitted polynomial as an expression, Horner in sqrt(n.v) and in rough (both clamped to the fitted [0.05, 1]). */
function polyExpr(c: readonly number[], n_v: F, rough: F): F {
  const s = sqrt(min(max(n_v, 0.05), 1)), r = min(max(rough, 0.05), 1);
  let acc: F = f(0);
  for (let i = FIT_MU; i >= 0; i--) {
    let inner: F = f(0);
    for (let j = FIT_R; j >= 0; j--) inner = add(mul(inner, r), c[i * (FIT_R + 1) + j]!);
    acc = add(mul(acc, s), inner);
  }
  return acc;
}

/**
 * The default `lobeAlbedo` for these parts: for the default pair (`ggx`, `smithGgx`) a fixed fit (`ggxAlbedo`); for
 * any other pair the integral of exactly that pair (`integrated`), so a lobe of 0 reflects no sky.
 */
export function albedoOf(distribution: PbrParts["distribution"], visibility: PbrParts["visibility"]): PbrParts["lobeAlbedo"] {
  if (distribution === ggx && visibility === smithGgx) return ggxAlbedo;
  const [a, b] = integrated(distribution, visibility);
  return (n_v, rough) => ({ scale: max(polyExpr(a, n_v, rough), 0), bias: max(polyExpr(b, n_v, rough), 0) });
}

/**
 * The lobe's albedo under the default pair: `e`, the integral of D Vis n.l over the hemisphere with F = 1, and `t`, the
 * share of it Schlick's (1 - v.h)^5 carries, so scale = e (1 - t) and bias = e t. Fitted by least squares (n.v from
 * 0.05) to a numerical integration of these parts on a 24 x 24 grid of n.v and rough: within 0.06 of f0 A + B at f0 = 1
 * and 0.04 at f0 = 0.04, mean 0.01. Karis's analytic fit (2014), made for another visibility, is up to 0.47 off for
 * these parts (agent_notes/2026-11-18_pbr_style/NOTE.md). Kept for the default pair so its pictures do not change.
 */
function ggxAlbedo(n_v: F, rough: F): { readonly scale: F; readonly bias: F } {
  const mu = max(n_v, 0.05), r = max(rough, 0.05);
  const poly = (c0: number, c1: number, c2: number): F => add(c0, mul(mu, add(c1, mul(mu, c2))));
  const e = saturate(add(1, mul(mul(r, r), add(poly(-2.407, 5.998, -2.676), mul(r, add(poly(5.343, -18.319, 9.583), mul(r, poly(-3.119, 11.39, -6.479))))))));
  const t = mul(pow(sub(1, mu), add(4.879, mul(r, -3.565))), exp(mul(r, add(-0.698, mul(r, -4.272)))));
  return { scale: mul(e, sub(1, t)), bias: mul(e, t) };
}

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
  const law = sky(dir, vec3(s[0]!, s[1]!, s[2]!));
  // Projected once, a sky that reads time or a param would light bodies as it was at time 0 and the param's default
  // while the background (and the sharp reflection) moved on: refused by name rather than drawn stale.
  const reads = variesBy(law);
  if (reads !== undefined) throw new PbrError({ kind: "varies", part: "sky", reads });
  const fn = onCpu("sky", [law]);
  const at = (d: number[]): number[] => fn({ vars: { pbr_sky_dir: d }, inputs: {}, params: {} })[0] as number[];
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
 * Cannot: follow a moved sun except through the one tap (the projection is at the sun's stated direction, as a
 * cloud's tables are); project a sky that reads time or a param (refused by name, `PbrError`: it would be stale); see the ground, other bodies or the body itself (a cavity is lit by
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
