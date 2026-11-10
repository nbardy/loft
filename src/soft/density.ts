// draw.density: a medium integrated per pixel in closed form (no marching, no sorting of primitives).
//
// The bound is drawn as its back faces (so it covers the screen whether the camera is outside or inside it) at depth
// 0, with no depth write: every pixel the box covers runs the fragment stage. There the view ray is clipped to the box,
// to the opaque scene behind it (scene depth: a body inside a cloud ends the integral at its skin, so it is veiled
// by exactly the cloud in front of it and the cloud has a soft edge where it meets it) and to the sea.
//
// Per pixel: every puff's optical depth on the segment (closed form, culled by closest approach), its centroid along
// the ray and its light there. Long elements (a fog layer seen along the ground) are cut into pieces along the ray so
// fog in front of a cloud and fog behind it are separate. The pieces go in a short list (overflow merges the faintest
// into a remainder), and are composited without sorting: each piece is dimmed by the optical depth of the others in
// front of its centroid, sum_j tau_j Phi((t_i - t_j) / sigma_j), a smooth version of "in front of". Alpha is exact
// (1 - exp(-total depth)); colour is the front-weighted average of the pieces' light.
//
// Lighting of a piece at its centroid p (all closed forms): the other puffs' depth toward the sun and the zenith from
// the puff's centre (the CPU light pass), plus the puff's own depth from p toward the sun and the zenith. Sun light
// goes through a two-lobe extinction (single scattering, plus a softer lobe standing in for multiple scattering so
// shadowed interiors are dark, not black) and a forward-scattering phase (silver edges toward the sun); sky light is
// the style's sky at the zenith. Flash light is per puff (CPU, through the medium) plus a small even share.

import { add, max, mul, variable } from "../expr.ts";
import { emitShaderFn } from "../compile/bind.ts";
import { drawSpec, type CompileContext, type Drawable, type Emitted, type Shader, type V3 } from "../compile/context.ts";
import { fbm3 } from "../lib/noise.ts";
import type { Skeleton } from "../map/types.ts";
import { BANK_LIB, GAUSS_LIB, type Box, type Medium } from "./medium.ts";

export type Rgb = readonly [number, number, number];

export type DensityOptions = {
  /** Where the medium is integrated: the box drawn, and the clip of every ray. Keep it 3 radii beyond the puffs. */
  readonly bounds: Box;
  /** Sun light colour and strength (multiplies the albedo). */
  readonly sun: Rgb;
  /** Strength of the sky light (the style's sky colour at the zenith). */
  readonly sky: number;
  /** Forward scattering, 0 (even) to 0.9 (a bright rim toward the sun). */
  readonly forward: number;
  /**
   * How strongly the other puffs shadow a puff, 0 to 1 (1 is the plain optical depth). Below 1 stands in for the light
   * that reaches a cloud's surface layers sideways (a stylised cloud reads best with its sunlit side near white).
   */
  readonly shadowing: number;
  /**
   * Detail: each piece's optical depth is multiplied by 1 + amount * noise at the point the eye sees, in the puff's own
   * frame (so the detail rides with it), with cells `scale` world units across. It breaks up the Gaussian edges into
   * wisps and lumps at the cost of one noise lookup per piece. `{ kind: "none" }` for smooth puffs.
   */
  readonly detail: { readonly kind: "none" } | { readonly kind: "noise"; readonly amount: number; readonly scale: number };
  /** Fade toward the style's sky with distance: 1 - exp(-distance * haze). 0 for none. */
  readonly haze: number;
};

/** Pieces kept per pixel; more merge into a remainder. */
const KMAX = 48;

const f = (x: number): string => (Number.isInteger(x) ? `${x}.0` : String(x));
const v3 = (v: readonly number[]): string => `vec3<f32>(${v.map(f).join(", ")})`;

export class DensityDrawable implements Drawable {
  readonly skeletons: readonly Skeleton[] = [];
  readonly surfaces = [];

  constructor(readonly name: string, readonly medium: Medium, readonly o: DensityOptions) {
    if (!/^[A-Za-z][A-Za-z0-9_]*$/.test(name)) throw new Error(`draw.density "${name}": names must be identifiers (they name shaders)`);
    for (let a = 0; a < 3; a++) if (!(o.bounds.max[a] > o.bounds.min[a])) throw new Error(`draw.density "${name}": bounds are empty on axis ${a}`);
    if (!(o.shadowing >= 0 && o.shadowing <= 1)) throw new Error(`draw.density "${name}": shadowing must be in [0, 1]`);
    if (!(o.forward >= 0 && o.forward < 1)) throw new Error(`draw.density "${name}": forward must be in [0, 1)`);
  }

  draws(ctx: CompileContext): Emitted {
    const t = this.medium.tables(ctx.sun);
    const P = ctx.table(t.puffs).wgsl;
    const NP = this.medium.puffs.length;
    const banks = t.banks.kind === "some" ? { n: this.medium.banks.length, acc: ctx.table(t.banks.table).wgsl } : { n: 0, acc: "" };
    const key = `density.${this.name}`;
    const detail = detailFn(ctx, this.o.detail);
    const code = [detail.code, densityWgsl(NP, P, banks, this.o, ctx.sun)].join("\n");
    return {
      shaders: { [key]: { code, libs: [...detail.libs, ...(banks.n > 0 ? [GAUSS_LIB, BANK_LIB] : [GAUSS_LIB])] } },
      draws: [drawSpec({ label: this.name, shader: key, vertices: 36, phase: "transparent", cull: "front", depthWrite: false, blend: "alpha", sceneDepth: true })],
    };
  }
}

const DP = variable("p", "vec3");
/** `fn soft_detail(p: vec3<f32>) -> f32`: the factor a piece's optical depth is multiplied by. */
function detailFn(ctx: CompileContext, d: DensityOptions["detail"]): Shader {
  switch (d.kind) {
    case "none": return { code: "fn soft_detail(p: vec3<f32>) -> f32 { return 1.0; }", libs: [] };
    case "noise": {
      const n = fbm3(mul(DP, 1 / d.scale), { octaves: 2, lacunarity: 2.03, gain: 0.5 });
      return emitShaderFn(ctx.slots, { name: "soft_detail", args: [["p", "vec3"]], out: "f32", roots: [max(add(mul(n, d.amount), 1), 0)], ret: (v) => v[0] });
    }
  }
}

/** The box's six faces: outward normal n and in-face axes u, v with u x v = n (so each face winds counter-clockwise seen from outside). */
const FACES = [
  [[1, 0, 0], [0, 1, 0], [0, 0, 1]], [[-1, 0, 0], [0, 0, 1], [0, 1, 0]],
  [[0, 1, 0], [0, 0, 1], [1, 0, 0]], [[0, -1, 0], [1, 0, 0], [0, 0, 1]],
  [[0, 0, 1], [1, 0, 0], [0, 1, 0]], [[0, 0, -1], [0, 1, 0], [1, 0, 0]],
] as const;

/** Box vertex stage shared by the soft volumes: back faces at depth 0, the world point as a varying. */
export function boxVertexWgsl(b: Box): string {
  const c = [0, 1, 2].map((a) => (b.min[a] + b.max[a]) / 2), h = [0, 1, 2].map((a) => (b.max[a] - b.min[a]) / 2);
  return /* wgsl */ `
const BOX_C: vec3<f32> = ${v3(c)};
const BOX_H: vec3<f32> = ${v3(h)};
const BOX_MIN: vec3<f32> = ${v3(b.min)};
const BOX_MAX: vec3<f32> = ${v3(b.max)};
const BOX_N = array<vec3<f32>, 6>(${FACES.map((x) => v3(x[0])).join(", ")});
const BOX_U = array<vec3<f32>, 6>(${FACES.map((x) => v3(x[1])).join(", ")});
const BOX_V = array<vec3<f32>, 6>(${FACES.map((x) => v3(x[2])).join(", ")});
const BOX_S = array<vec2<f32>, 6>(vec2<f32>(-1.0, -1.0), vec2<f32>(1.0, -1.0), vec2<f32>(1.0, 1.0), vec2<f32>(-1.0, -1.0), vec2<f32>(1.0, 1.0), vec2<f32>(-1.0, 1.0));
struct VOut { @builtin(position) clip: vec4<f32>, @location(0) world: vec3<f32> };
@vertex fn vs(@builtin(vertex_index) vi: u32) -> VOut {
  let k = vi / 6u;
  let s = BOX_S[vi % 6u];
  let p = BOX_C + BOX_H * (BOX_N[k] + BOX_U[k] * s.x + BOX_V[k] * s.y);
  var out: VOut;
  let clip = frame.view_proj * vec4<f32>(p, 1.0);
  // Depth 0: the volume is integrated per pixel up to the opaque scene, so the box itself must never be depth-rejected
  // (its back faces lie behind the bodies inside it). Clipping still happens at w = 0, in front of the eye.
  out.clip = vec4<f32>(clip.xy, 0.0, clip.w);
  out.world = p;
  return out;
}
/// The ray through this pixel: origin on the eye (or the eye plane, orthographic) and unit direction.
fn soft_ray(world: vec3<f32>) -> array<vec3<f32>, 2> {
  let d = view_dir(world);
  return array<vec3<f32>, 2>(world - d * dot(world - frame.eye.xyz, d), d);
}
/// (near, far) of the ray inside the box.
fn soft_box(o: vec3<f32>, d: vec3<f32>) -> vec2<f32> {
  let dd = select(d, vec3<f32>(1e-12), abs(d) < vec3<f32>(1e-12));
  let ta = (BOX_MIN - o) / dd;
  let tb = (BOX_MAX - o) / dd;
  let lo = min(ta, tb);
  let hi = max(ta, tb);
  return vec2<f32>(max(max(lo.x, lo.y), lo.z), min(min(hi.x, hi.y), hi.z));
}
/// Distance along the ray to the sea surface (three fixed-point steps on the swell), or far when it looks up.
fn soft_sea(o: vec3<f32>, d: vec3<f32>) -> f32 {
  if (d.y > -1e-4) { return 1e30; }
  var t = -o.y / d.y;
  for (var k = 0u; k < 3u; k++) { let p = o + d * max(t, 0.0); t = (sea_height(p.xz) - o.y) / d.y; }
  return max(t, 0.0);
}
/// Where the ray ends: the opaque scene at this pixel, or the sea.
fn soft_end(px: vec2<f32>, o: vec3<f32>, d: vec3<f32>) -> f32 {
  return min(dot(scene_position(px) - o, d), soft_sea(o, d));
}`;
}

function densityWgsl(NP: number, P: string, banks: { readonly n: number; readonly acc: string }, o: DensityOptions, sun: V3): string {
  const g = o.forward;
  const bankLoop = banks.n === 0 ? "" : /* wgsl */ `
  for (var i = 0u; i < ${banks.n}u; i++) {
    let a0 = ${banks.acc}(i * 4u);
    if (a0.w <= 0.0) { continue; }
    let a1 = ${banks.acc}(i * 4u + 1u);
    let a2 = ${banks.acc}(i * 4u + 2u);
    let e = ${banks.acc}(i * 4u + 3u);
    let O = a1.xyz * (o - a0.xyz);
    let D = a1.xyz * d;
    let s0 = e.x * o.x + e.y * o.z - e.z;
    let s1 = e.x * d.x + e.y * d.z;
    let cut = loft_bank_split(s0, s1);
    soft_element(vec4<f32>(O, 0.0), vec4<f32>(D, 0.0), max(t0, cut.x), min(t1, cut.y), a0, a1.xyz, a2.xyz, vec4<f32>(0.0), vec3<f32>(0.0), o, d, f32(i) + 0.5);
    soft_element(vec4<f32>(O, e.w * s0), vec4<f32>(D, e.w * s1), max(t0, cut.z), min(t1, cut.w), a0, a1.xyz, a2.xyz, vec4<f32>(0.0), vec3<f32>(0.0), o, d, f32(i) + 0.5);
  }`;
  return /* wgsl */ `${boxVertexWgsl(o.bounds)}
const KMAX: u32 = ${KMAX}u;
const SUN_DIR: vec3<f32> = ${v3(sun)};
const SUN_LIGHT: vec3<f32> = ${v3(o.sun)};
const SHADOWING: f32 = ${f(o.shadowing)};
const SELF: f32 = 0.35;
var<private> soft_e: array<vec4<f32>, ${KMAX + 1}>;   // (tau, centroid, spread, 0)
var<private> soft_c: array<vec3<f32>, ${KMAX + 1}>;   // light
var<private> soft_n: u32;
var<private> soft_rem: vec4<f32>;                     // remainder: (tau, tau t, tau (t^2 + spread^2), 0)
var<private> soft_remc: vec3<f32>;                    // remainder: tau-weighted light
var<private> soft_sky: vec3<f32>;
var<private> soft_amb: vec3<f32>;

fn soft_remainder(e: vec4<f32>, c: vec3<f32>) {
  soft_rem += vec4<f32>(e.x, e.x * e.y, e.x * (e.y * e.y + e.z * e.z), 0.0);
  soft_remc += e.x * c;
}
fn soft_push(e: vec4<f32>, c: vec3<f32>) {
  if (soft_n < KMAX) { soft_e[soft_n] = e; soft_c[soft_n] = c; soft_n += 1u; return; }
  var m = 0u;
  for (var j = 1u; j < KMAX; j++) { if (soft_e[j].x < soft_e[m].x) { m = j; } }
  if (soft_e[m].x < e.x) {
    soft_remainder(soft_e[m], soft_c[m]);
    soft_e[m] = e;
    soft_c[m] = c;
  } else {
    soft_remainder(e, c);
  }
}
/// The light a puff sends toward the eye at p: sun (through the other puffs and itself), sky, flashes.
fn soft_light(albedo: vec3<f32>, tauSun: f32, tauUp: f32, flash: vec3<f32>, cosv: f32) -> vec3<f32> {
  // Octaves of extinction (Wrenninge's multiple-scattering approximation): each octave half as strong and a third as
  // extinguished, so thick cloud stays luminous a long way in and only the deepest parts go dark.
  let sunVis = (exp(-tauSun) + 0.5 * exp(-0.3 * tauSun) + 0.25 * exp(-0.09 * tauSun)) * (1.0 / 1.75);
  let skyVis = (exp(-0.5 * tauUp) + 0.5 * exp(-0.15 * tauUp)) * (1.0 / 1.5);
  let hg = ${f(1 - g * g)} / pow(${f(1 + g * g)} - ${f(2 * g)} * cosv, 1.5);
  let phase = mix(1.0, hg, 0.5);
  return albedo * (SUN_LIGHT * sunVis * phase + soft_sky * skyVis + flash) + soft_amb;
}
/// The other puffs' depth toward a light at q (in the puff's units), between its values one radius behind and in front.
fn soft_others(q: vec3<f32>, k: vec3<f32>, u: vec3<f32>, lo: f32, hi: f32) -> f32 {
  let ku = k * u;
  // Half the slope of the line through the two samples: the full slope gave every surface puff its own hard terminator
  // (a cloud of lit spheres); light scattered sideways inside a cloud softens it.
  let f = clamp(dot(q, ku) / max(length(ku), 1e-9), -1.0, 1.0);
  return mix(lo, hi, 0.5 + 0.25 * f);
}
/// One element (a puff, or one side of a bank) on [ia0, ib0]: its pieces, each lit where it is seen, pushed.
fn soft_element(O: vec4<f32>, D: vec4<f32>, ia0: f32, ib0: f32, a0: vec4<f32>, k: vec3<f32>, albedo: vec3<f32>, others: vec4<f32>, flash: vec3<f32>, o: vec3<f32>, d: vec3<f32>, seed: f32) {
  // Only where the exponent is within 13 of its minimum does the element matter (e^-13 ~ 2e-6).
  let al = max(dot(D, D), 1e-20);
  let tm = -dot(O, D) / al;
  let half = 3.6 / sqrt(al);
  let ia = max(ia0, tm - half);
  let ib = min(ib0, tm + half);
  if (ib <= ia) { return; }
  // A layer (unbounded on some axis) is cut into 4 pieces along the ray, so fog in front of a cloud and fog behind it
  // are separate pieces. A fixed count: a count that grew with the length jumped from one pixel to the next and drew
  // streaks along the view rays where it changed.
  let pieces = select(1.0, 4.0, min(k.x, min(k.y, k.z)) == 0.0);
  let cosv = dot(d, SUN_DIR);
  for (var j = 0.0; j < pieces; j += 1.0) {
    let pa = mix(ia, ib, j / pieces);
    let pb = mix(ia, ib, (j + 1.0) / pieces);
    let r = loft_gauss_ray(O, D, pa, pb);
    // Detail only on bounded puffs: on a layer seen toward the horizon it was finer than a pixel and drew grain.
    let detail = select(soft_detail(o + d * r.y - a0.xyz + vec3<f32>(seed * 37.31, seed * 11.17, seed * 5.71)), 1.0, pieces > 1.0);
    let tau = a0.w * r.x * detail;
    if (tau < 1e-4) { continue; }
    // Light the piece where the eye sees it: the median of the first-scattering depth, where the piece's own optical
    // depth reaches -ln((1 + e^-tau) / 2) (its centre when thin, ln 2 into it when dense). Lighting it at its centroid
    // instead lit a dense puff's hidden middle and drew every puff as a dark disc with a bright rim.
    let fs = select(0.5, -log(0.5 * (1.0 + exp(-tau))) / tau, tau > 1e-3);
    let tv = clamp(r.y + r.z * (1.0 / 1.7) * log(fs / (1.0 - fs)), pa, pb);
    let p = o + d * tv;
    let Op = k * (p - a0.xyz);
    let selfSun = a0.w * loft_gauss_tail(Op, k * SUN_DIR);
    let selfUp = a0.w * loft_gauss_tail(Op, vec3<f32>(0.0, k.y, 0.0));
    let sunOthers = soft_others(Op, k, SUN_DIR, others.x, others.y);
    let upOthers = soft_others(Op, k, vec3<f32>(0.0, 1.0, 0.0), others.z, others.w);
    // Shadowing scales the whole depth toward a light, the puff's own part too: scaling only the others' part left each
    // puff's self-shadow dominant, and every puff of a backlit cloud drew as a bright ring round a dark middle.
    // A puff's own shadow counts a third: exact for a lone Gaussian, it lit each lump of a cloud as a separate sphere
    // with its own hard terminator, where light scattered sideways through the mass softens it.
    soft_push(vec4<f32>(tau, r.y, r.z, 0.0), soft_light(albedo, SHADOWING * (sunOthers + SELF * selfSun), SHADOWING * (upOthers + SELF * selfUp), flash, cosv));
  }
}
@fragment fn fs(in: VOut) -> @location(0) vec4<f32> {
  let ray = soft_ray(in.world);
  let o = ray[0];
  let d = ray[1];
  let box = soft_box(o, d);
  let t0 = max(box.x, 0.0);
  let t1 = min(box.y, soft_end(in.clip.xy, o, d));
  if (t1 <= t0) { discard; }
  soft_n = 0u;
  soft_rem = vec4<f32>(0.0);
  soft_remc = vec3<f32>(0.0);
  soft_sky = ${f(o.sky)} * sky(vec3<f32>(0.0, 1.0, 0.0));
  soft_amb = ${P}(${NP}u * 5u).xyz;
  for (var i = 0u; i < ${NP}u; i++) {
    let a0 = ${P}(i * 5u);
    let a1 = ${P}(i * 5u + 1u);
    let O = a1.xyz * (o - a0.xyz);
    let D = a1.xyz * d;
    let al = max(dot(D, D), 1e-20);
    let tm = clamp(-dot(O, D) / al, t0, t1);
    let pc = O + D * tm;
    if (a0.w <= 0.0 || dot(pc, pc) > 16.0) { continue; }
    let a2 = ${P}(i * 5u + 2u);
    let a3 = ${P}(i * 5u + 3u);
    let a4 = ${P}(i * 5u + 4u);
    soft_element(vec4<f32>(O, 0.0), vec4<f32>(D, 0.0), t0, t1, a0, a1.xyz, a2.xyz, a4, a3.xyz, o, d, f32(i));
  }${bankLoop}
  // The remainder is one more element.
  var n = soft_n;
  if (soft_rem.x > 0.0) {
    let tc = soft_rem.y / soft_rem.x;
    soft_e[n] = vec4<f32>(soft_rem.x, tc, sqrt(max(soft_rem.z / soft_rem.x - tc * tc, 1e-4)), 0.0);
    soft_c[n] = soft_remc / soft_rem.x;
    n += 1u;
  }
  var total = 0.0;
  for (var i = 0u; i < n; i++) { total += soft_e[i].x; }
  if (total < 1e-4) { discard; }
  var acc = vec3<f32>(0.0);
  var wsum = 0.0;
  var tmean = 0.0;
  for (var i = 0u; i < n; i++) {
    let ei = soft_e[i];
    var front = 0.0;
    for (var j = 0u; j < n; j++) {
      let ej = soft_e[j];
      // Phi((t_i - t_j) / sigma_j), logistic form; self excluded.
      front += select(ej.x / (1.0 + exp(-1.7 * (ei.y - ej.y) / max(ej.z, 1e-3))), 0.0, j == i);
    }
    let w = exp(-front) * (1.0 - exp(-ei.x));
    acc += w * soft_c[i];
    wsum += w;
    tmean += w * ei.y;
  }
  var colour = acc / max(wsum, 1e-8);
  ${o.haze > 0 ? `colour = mix(colour, sky(d), 1.0 - exp(-(tmean / max(wsum, 1e-8)) * ${f(o.haze)}));` : ""}
  return vec4<f32>(colour, 1.0 - exp(-total));
}`;
}
