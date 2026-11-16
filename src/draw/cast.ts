// castRay: a ray against every thing's stand-in, the capsules of its skeleton as posed this frame, and a plane ground
// (docs/proposals/geometry_and_rays.md section 4.5, with the id and m of docs/proposals/headroom.md A3).
//
// The expression is a call to library functions whose WGSL the compiler writes per scene, only when some emitted code
// calls them (src/compile/index.ts): a scene that never casts builds neither the chains' bounds nor the functions, so
// its artefact is the same bytes. The capsules are the joints the shadows already pose on the GPU every frame
// (`casters`, src/draw/shadows.ts); one more map table holds a bounding sphere per chain, so a ray tests a chain's
// capsules only when it passes near the chain. Each segment is the rounded cone between two joints (impostors.ts).
//
// Colour: a thing's mean paint, sampled on the CPU at rest over its surfaces or near its field's surface, or a
// stated grey where it has neither or its paint cannot run on the CPU (`Colour`). Not the style's ink at the hit, as
// first planned: the ink reads screen derivatives, which a ray cast from inside a branch of a paint cannot take. The ground: a plane's own colour; a sea is met as its still plane, in
// the style's clear colour (its swell is not traced). The CPU twin (`castCpu`) is the same search over numbers.

import { add, f, fetch, length, lib, max, mul, sub, substitute, variable, vec3, vec4, type Expr, type LibFn, type TableSource } from "../expr.ts";
import { aabbOf } from "./trace.ts";
import { fieldPixel } from "./field.ts";
import type { CompileContext, Drawable, Form } from "../compile/context.ts";
import type { CpuFn } from "../compile/index.ts";
import type { F, P3, PaintAt } from "../map/types.ts";
import { lit, vec3Lit } from "../compile/scene_wgsl.ts";
import { casters } from "./shadows.ts";
import { nameId } from "./surface.ts";
import { classify } from "./painted.ts";
import { CLOSEST_WGSL, shapeWgsl } from "./impostors.ts";
import type { Ray, Traced } from "./traced.ts";

/** The search: (t, gap, k, 0) with k the segment's first joint, -1 the ground, -2 nothing. Its text is the scene's. */
export const CAST_LIB: LibFn = { name: "loft_cast", ret: "vec4", wgsl: "", js: "gpu-only" };
const after = (name: string, ret: LibFn["ret"]): LibFn => ({ name, ret, wgsl: "", js: "gpu-only", deps: [CAST_LIB] });
const NORMAL_LIB = after("loft_cast_normal", "vec3"), COLOUR_LIB = after("loft_cast_colour", "vec3"), ID_LIB = after("loft_cast_id", "f32"), M_LIB = after("loft_cast_m", "f32");
const CAST_LIBS: readonly LibFn[] = [CAST_LIB, NORMAL_LIB, COLOUR_LIB, ID_LIB, M_LIB];

/**
 * A ray against every thing's stand-in (the capsules of its skeleton as posed this frame) and a plane ground: the
 * nearest hit, or the nearest miss weighed by the ray's width (`Traced`; a miss by more than a width is reported as a
 * clean one, gap = width at t1, at t1, with nothing met), with the colour there, the thing's id (its
 * shading id, varied per copy) and m (where along the chain, 0 root to 1 tip).
 *
 * Law: the nearest of the scene's skeleton capsules (rounded cones between joints) and the ground plane, with
 * closest-approach soft edges; `width` is read at t0 and t1 and taken as affine between. Parameters: the ray. Cannot:
 * see exact surfaces (a wide flat part, a carapace or a sail, is its round chain); see things with no skeleton; see
 * the sea's swell; return a hit's real paint (a thing's mean paint, or the style's ink where none can be sampled); run
 * on the CPU inside an expression (the twin is `castCpu`, for checks). Cost: about 0.045 ms per capsule tested per
 * million rays; chains are culled by bounding sphere first. Relatives: your own `hit` or `march(field)` for the one thing
 * that must be exact; closed-form occlusion of a sphere or capsule (Quilez) for ambient light; screen-space hits for
 * on-screen colour.
 */
export function castRay(ray: Ray): Traced & { readonly colour: P3; readonly id: Expr<"f32">; readonly m: Expr<"f32"> } {
  const h = lib<"vec4">(CAST_LIB, ray.o, ray.d, ray.t0, ray.t1, ray.width(ray.t0), ray.width(ray.t1));
  return { t: h.x, gap: h.y, normal: lib<"vec3">(NORMAL_LIB, ray.o, ray.d, h), colour: lib<"vec3">(COLOUR_LIB, ray.o, ray.d, h), id: lib<"f32">(ID_LIB, h), m: lib<"f32">(M_LIB, ray.o, ray.d, h) };
}

/** What a chain is met as: its first joint in the joint table, its joints, and the thing and copy it came from. */
export type ChainInfo = { readonly first: number; readonly joints: number; readonly thing: number; readonly copy: number };

/** A thing's colour as a ray sees it, classified once: its mean paint, or mid grey (and why). */
type Colour = { readonly kind: "mean"; readonly rgb: readonly [number, number, number] } | { readonly kind: "grey"; readonly why: string };

/** Surface samples per side for a thing's mean paint. */
const MEAN_GRID = 6;

/** A plan made when the scene calls castRay: the chains' bounds table is declared; the WGSL is written after layout. */
export type CastPlan = { readonly libs: readonly LibFn[]; wgsl(cpu: (roots: readonly Expr[]) => CpuFn, rest: Parameters<CpuFn>[0]): string };

/** Whether the emitted code casts rays, and if so the plan (the bounds table declared now, before the layout). */
export function castPlan(ctx: CompileContext, used: readonly LibFn[]): CastPlan | undefined {
  if (!used.some((l) => CAST_LIBS.includes(l))) return undefined;
  const { infos, at, table } = chainSpheres(ctx.things);
  const joints = ctx.table(at).wgsl;
  const bounds = ctx.table(table).wgsl;
  return {
    libs: CAST_LIBS,
    wgsl: (cpu, rest) => castWgsl(ctx, infos, joints, bounds, ctx.things.map((t) => colourOf(t, cpu, rest))),
  };
}

/** One bounding sphere per chain (centre, radius), from its posed joints: a map over the joint table. */
/**
 * Every chain of the scene's casters (`casters`) and one per-frame map table of a bounding sphere per chain (centre,
 * radius), one per scene (keyed by its list of things): castRay culls by it, and a lamp's shadow frames its picture on
 * it (src/frame/lamp_shadow.ts), so a scene doing both builds it once.
 */
export function chainSpheres(things: readonly Drawable[]): { readonly infos: readonly ChainInfo[]; readonly at: TableSource; readonly table: TableSource } {
  const seen = spheres.get(things);
  if (seen !== undefined) return seen;
  const { chains, at } = casters(things);
  const infos: ChainInfo[] = [];
  let first = 0;
  things.forEach((t, thing) => t.skeletons.filter((s) => s.joints >= 2).forEach((s, copy) => { infos.push({ first, joints: s.joints, thing, copy }); first += s.joints; }));
  if (infos.length !== chains.length) throw new Error(`castRay: ${infos.length} chains counted, ${chains.length} cast (casters' order changed?)`);
  const made = { infos, at, table: boundsTable(infos, at) };
  spheres.set(things, made);
  return made;
}
const spheres = new WeakMap<readonly Drawable[], { readonly infos: readonly ChainInfo[]; readonly at: TableSource; readonly table: TableSource }>();

function boundsTable(infos: readonly ChainInfo[], at: TableSource): TableSource {
  const runs = infos.map((c) => ({
    count: 1,
    lanes: () => {
      const js = Array.from({ length: c.joints }, (_, k) => fetch(at, f(c.first + k), 0));
      const centre = mul(js.slice(1).reduce((s: P3, j) => add(s, j.xyz), js[0]!.xyz), 1 / c.joints) as P3;
      const radius = js.reduce((r: F, j) => max(r, add(length(sub(j.xyz, centre)), j.w)), f(0));
      return [vec4(centre, radius)];
    },
  }));
  return { name: "cast_chains", lanes: 1, entries: Math.max(1, infos.length), role: "data", program: { kind: "map", runs } };
}

/** A thing's mean paint at rest: over its surfaces' (u, v), or at samples of its field near the surface; else grey. */
function colourOf(t: Drawable, cpu: (roots: readonly Expr[]) => CpuFn, rest: Parameters<CpuFn>[0]): Colour {
  const samples: { roots: Expr[]; vars: Record<string, number | number[]>[] }[] = [];
  const U = variable("cast_u", "f32"), V = variable("cast_v", "f32"), P = variable("cast_p", "vec3");
  for (const form of t.forms) {
    switch (form.kind) {
      case "surface": {
        const at: PaintAt = { world: form.surface.point(U, V), normal: form.surface.normal(U, V), copy: f(form.copy) };
        // A thing's base colour: what it says beyond a colour is a style's to print, not a ray's.
        samples.push({ roots: [classify(form.name, form.paint(U, V, at)).base], vars: Array.from({ length: MEAN_GRID * MEAN_GRID }, (_, k) => ({ cast_u: ((k % MEAN_GRID) + 0.5) / MEAN_GRID, cast_v: (Math.floor(k / MEAN_GRID) + 0.5) / MEAN_GRID })) });
        break;
      }
      case "field": {
        // Points of a grid in the bound within a cell of the surface (the field at full detail), painted facing up.
        const box = aabbOf(form.bound), n = 4 * MEAN_GRID;
        const cell = Math.max(...[0, 1, 2].map((k) => (box.max[k]! - box.min[k]!) / n));
        let d: CpuFn;
        try { d = cpu([substitute([form.field(P)], fieldPixel, f(0))[0]!]); } catch (e) { return { kind: "grey", why: `${t.name}'s field cannot run on the CPU (${String(e).slice(0, 60)})` }; }
        const near: Record<string, number[]>[] = [];
        for (let i = 0; i < n; i++) for (let j = 0; j < n; j++) for (let k = 0; k < n; k++) {
          const p = [0, 1, 2].map((a) => box.min[a]! + ((([i, j, k][a]!) + 0.5) / n) * (box.max[a]! - box.min[a]!));
          if (Math.abs(d(rest, { cast_p: p })[0] as number) < cell) near.push({ cast_p: p });
        }
        if (near.length > 0) samples.push({ roots: [substitute([classify(form.name, form.paint({ p: P, normal: vec3(0, 1, 0) })).base], fieldPixel, f(0))[0]!], vars: near });
        break;
      }
      case "mesh": case "medium": case "hit": break;
    }
  }
  if (samples.length === 0) return { kind: "grey", why: `${t.name} has no surface or field form to sample` };
  const sum = [0, 0, 0];
  let count = 0;
  for (const s of samples) {
    let run: CpuFn;
    try { run = cpu(s.roots); } catch (e) { return { kind: "grey", why: `${t.name}'s paint cannot run on the CPU (${String(e).slice(0, 60)})` }; }
    for (const v of s.vars) { const c = run(rest, v)[0] as number[]; for (let k = 0; k < 3; k++) sum[k] += c[k]!; count++; }
  }
  return { kind: "mean", rgb: [sum[0]! / count, sum[1]! / count, sum[2]! / count] };
}

/** The scene's cast functions: the search, then the normal, colour, id and m at its result. */
function castWgsl(ctx: CompileContext, infos: readonly ChainInfo[], J: string, B: string, colours: readonly Colour[]): string {
  const N = infos.length;
  const ground = ctx.ground.kind;
  // Per joint: its chain (so a segment knows its thing, copy and place along the chain).
  const chainOf = infos.flatMap((c, i) => Array.from({ length: c.joints }, () => i));
  const u32s = (xs: readonly number[]) => (xs.length === 0 ? "0u" : xs.map((x) => `${x}u`).join(", "));
  const colourCase = (c: Colour, i: number): string => {
    switch (c.kind) {
      case "mean": return `  if (th == ${i}u) { return ${vec3Lit(c.rgb)}; }`;
      case "grey": return `  if (th == ${i}u) { return vec3<f32>(0.5); } // ${c.why.replace(/[\n*/]/g, " ")}`;
    }
  };
  const groundHit = ground === "nothing" ? "" : /* wgsl */ `
  // The ground: the plane y = 0 (a sea's still plane).
  if (d.y < -1e-6) {
    let tg = -o.y / d.y;
    if (tg >= t0 && tg <= t1 && tg < hitT) { hitT = tg; hitK = -1.0; }
  }`;
  const groundColour = ground === "plane" ? "ground_colour(p, -d, h.x, 0.0)" : vec3Lit(ctx.style.clear);
  // The rounded cone's functions under names of their own: a module that draws copies defines the same ones.
  return /* wgsl */ `
${`${shapeWgsl("cone")}\n${CLOSEST_WGSL}`.replace(/\bimp_/g, "cast_imp_")}
const CAST_CHAINS: u32 = ${N}u;
const CAST_FIRST = array<u32, ${Math.max(1, N)}>(${u32s(infos.map((c) => c.first))});
const CAST_COUNT = array<u32, ${Math.max(1, N)}>(${u32s(infos.map((c) => c.joints))});
const CAST_THING = array<u32, ${Math.max(1, N)}>(${u32s(infos.map((c) => c.thing))});
const CAST_COPY = array<u32, ${Math.max(1, N)}>(${u32s(infos.map((c) => c.copy))});
const CAST_ID = array<f32, ${Math.max(1, ctx.things.length)}>(${ctx.things.map((t) => lit(nameId(t.name))).join(", ")});
const CAST_JOINT_CHAIN = array<u32, ${Math.max(1, chainOf.length)}>(${u32s(chainOf)});
fn loft_cast(o: vec3<f32>, d: vec3<f32>, t0: f32, t1: f32, w0: f32, w1: f32) -> vec4<f32> {
  let wk = (w1 - w0) / max(t1 - t0, 1e-6);
  var hitT = 1e30;
  var hitK = -2.0;
  // A miss further than the ray's own width covers none of it: the search keeps only nearer ones, so a chain whose
  // bounding sphere the ray clears by a width is never opened (without this cap the largest example cost +4 to +7 ms).
  var nearR = 1.0;
  var nearT = t1;
  var nearG = w1;
  var nearK = -2.0;
  for (var c = 0u; c < CAST_CHAINS; c++) {
    let s = ${B}(c);
    // The chain's sphere: skipped when the ray passes it further (in widths) than the nearest miss so far, or when a
    // hit is already nearer than the sphere's front.
    let tc = clamp(dot(s.xyz - o, d), t0, t1);
    let clear = length(o + d * tc - s.xyz) - s.w;
    if (clear > 0.0 && (hitT < 1e30 || clear / max(w0 + wk * (tc - t0), 1e-9) >= nearR)) { continue; }
    if (dot(s.xyz - o, d) - s.w > hitT) { continue; }
    let last = CAST_FIRST[c] + CAST_COUNT[c] - 1u;
    for (var j = CAST_FIRST[c]; j < last; j++) {
      let a = ${J}(j);
      let b = ${J}(j + 1u);
      // A ray that starts inside a capsule meets it where it starts.
      let hh = select(cast_imp_hit(o, d, a, b), vec4<f32>(t0, 0.0, 0.0, 0.0), cast_imp_sd(o + d * t0, a, b) < 0.0);
      if (hh.x >= t0 && hh.x <= t1) {
        if (hh.x < hitT) { hitT = hh.x; hitK = f32(j); }
      } else if (hitT >= 1e30) {
        let tq = clamp(cast_imp_closest(o, d, a.xyz, b.xyz), t0, t1);
        let g = cast_imp_sd(o + d * tq, a, b);
        let r = g / max(w0 + wk * (tq - t0), 1e-9);
        if (r < nearR) { nearR = r; nearT = tq; nearG = g; nearK = f32(j); }
      }
    }
  }${groundHit}
  return select(vec4<f32>(nearT, max(nearG, 1e-9), nearK, 0.0), vec4<f32>(hitT, 0.0, hitK, 0.0), hitT < 1e30);
}
/// Where along segment k the point p lies, 0..1.
fn loft_cast_u(p: vec3<f32>, k: u32) -> f32 {
  let a = ${J}(k).xyz;
  let ba = ${J}(k + 1u).xyz - a;
  return clamp(dot(p - a, ba) / max(dot(ba, ba), 1e-20), 0.0, 1.0);
}
fn loft_cast_normal(o: vec3<f32>, d: vec3<f32>, h: vec4<f32>) -> vec3<f32> {
  if (h.z < -1.5) { return -d; }
  if (h.z < -0.5) { return vec3<f32>(0.0, 1.0, 0.0); }
  let p = o + d * h.x;
  let k = u32(h.z);
  let a = ${J}(k).xyz;
  let ba = ${J}(k + 1u).xyz - a;
  return normalize(p - (a + ba * loft_cast_u(p, k)));
}
fn loft_cast_id(h: vec4<f32>) -> f32 {
  if (h.z < -0.5) { return 0.0; }
  let c = CAST_JOINT_CHAIN[u32(h.z)];
  return copy_id(CAST_ID[CAST_THING[c]], CAST_COPY[c]);
}
fn loft_cast_m(o: vec3<f32>, d: vec3<f32>, h: vec4<f32>) -> f32 {
  if (h.z < -0.5) { return 0.0; }
  let k = u32(h.z);
  let c = CAST_JOINT_CHAIN[k];
  return (f32(k - CAST_FIRST[c]) + loft_cast_u(o + d * h.x, k)) / f32(CAST_COUNT[c] - 1u);
}
fn loft_cast_colour(o: vec3<f32>, d: vec3<f32>, h: vec4<f32>) -> vec3<f32> {
  let p = o + d * h.x;
  // Before any branch: the ground's colour may take screen derivatives, which only uniform control flow may.
  let ground = ${groundColour};
  if (h.z < -1.5) { return vec3<f32>(0.0); }
  if (h.z < -0.5) { return ground; }
  let th = CAST_THING[CAST_JOINT_CHAIN[u32(h.z)]];
${colours.map(colourCase).join("\n")}
  return vec3<f32>(0.0);
}`;
}

// ---- the CPU twin -------------------------------------------------------------------------------------------------

type V3 = readonly [number, number, number];
/** A joint as the CPU sees it: centre and radius. */
export type CastJoint = { readonly p: V3; readonly r: number };
const sub3 = (a: V3, b: V3): V3 => [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
const dot3 = (a: V3, b: V3): number => a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
const at3 = (o: V3, d: V3, t: number): V3 => [o[0] + d[0] * t, o[1] + d[1] * t, o[2] + d[2] * t];

/** The rounded cone between two joints: where the ray enters it (Quilez's closed form, as impostors.ts), or -1. */
function coneHit(o: V3, d: V3, a: CastJoint, b: CastJoint): number {
  const ra = a.r, rb = b.r;
  if (Math.max(ra, rb) <= 0) return -1;
  const ba = sub3(b.p, a.p), oa = sub3(o, a.p), ob = sub3(o, b.p), rr = ra - rb;
  const m0 = dot3(ba, ba), m1 = dot3(ba, oa), m2 = dot3(ba, d), m3 = dot3(d, oa), m5 = dot3(oa, oa), m6 = dot3(ob, d), m7 = dot3(ob, ob);
  const d2 = m0 - rr * rr, k2 = d2 - m2 * m2, k1 = d2 * m3 - m1 * m2 + m2 * rr * ra, k0 = d2 * m5 - m1 * m1 + m1 * rr * ra * 2 - m0 * ra * ra;
  const h = k1 * k1 - k0 * k2;
  if (h < 0) return -1;
  const t = (-Math.sqrt(h) - k1) / k2, y = m1 - ra * rr + t * m2;
  if (y > 0 && y < d2) return t;
  const h1 = m3 * m3 - m5 + ra * ra, h2 = m6 * m6 - m7 + rb * rb;
  if (Math.max(h1, h2) < 0) return -1;
  let r = 1e20;
  if (h1 > 0 && ra > 0) r = -m3 - Math.sqrt(h1);
  if (h2 > 0 && rb > 0) r = Math.min(r, -m6 - Math.sqrt(h2));
  return r >= 1e19 ? -1 : r;
}

/** The rounded cone's exact distance (impostors.ts `imp_sd`, src/lib/sdf3.ts sdRoundCone). */
function coneDistance(p: V3, a: CastJoint, b: CastJoint): number {
  const ba = sub3(b.p, a.p), l2 = dot3(ba, ba), rr = a.r - b.r, a2 = l2 - rr * rr, il2 = 1 / l2;
  const pa = sub3(p, a.p), y = dot3(pa, ba), z = y - l2;
  const v: V3 = [pa[0] * l2 - ba[0] * y, pa[1] * l2 - ba[1] * y, pa[2] * l2 - ba[2] * y];
  const x2 = dot3(v, v), y2 = y * y * l2, z2 = z * z * l2, k = Math.sign(rr) * rr * rr * x2;
  if (Math.sign(z) * a2 * z2 > k) return Math.sqrt(x2 + z2) * il2 - b.r;
  if (Math.sign(y) * a2 * y2 < k) return Math.sqrt(x2 + y2) * il2 - a.r;
  return (Math.sqrt(x2 * a2 * il2) + y * rr) * il2 - a.r;
}

/** The ray's t nearest the segment a-b (impostors.ts `imp_closest`). */
function closestT(o: V3, d: V3, a: V3, b: V3): number {
  const ba = sub3(b, a), w = sub3(o, a), bb = dot3(ba, ba), db = dot3(d, ba), den = bb - db * db;
  const s = den < 1e-9 * Math.max(bb, 1e-20) || bb < 1e-20 ? 0 : Math.min(1, Math.max(0, (dot3(w, ba) - db * dot3(w, d)) / den));
  return Math.max(dot3(sub3(at3(a, ba, s), o), d), 0);
}

/**
 * castRay on the CPU, over chains of joints as numbers (a check's posed skeletons): the nearest hit (gap 0) or the
 * nearest miss weighed by the width, which is w0 at t0 and w1 at t1. `chain` and `segment` say what was met (-1: the
 * ground y = 0, when `ground` is true; -2: nothing).
 */
export function castCpu(o: V3, d: V3, t0: number, t1: number, w0: number, w1: number, chains: readonly (readonly CastJoint[])[], ground: boolean): { readonly t: number; readonly gap: number; readonly chain: number; readonly segment: number } {
  const wk = (w1 - w0) / Math.max(t1 - t0, 1e-6);
  let hit = { t: Infinity, chain: -2, segment: -2 }, near = { r: 1, t: t1, gap: w1, chain: -2, segment: -2 };
  chains.forEach((c, ci) => {
    for (let j = 0; j + 1 < c.length; j++) {
      const a = c[j]!, b = c[j + 1]!;
      const th = coneDistance(at3(o, d, t0), a, b) < 0 ? t0 : coneHit(o, d, a, b);
      if (th >= t0 && th <= t1) { if (th < hit.t) hit = { t: th, chain: ci, segment: j }; continue; }
      const tq = Math.min(t1, Math.max(t0, closestT(o, d, a.p, b.p))), g = coneDistance(at3(o, d, tq), a, b), r = g / Math.max(w0 + wk * (tq - t0), 1e-9);
      if (r < near.r) near = { r, t: tq, gap: g, chain: ci, segment: j };
    }
  });
  if (ground && d[1] < -1e-6) { const tg = -o[1] / d[1]; if (tg >= t0 && tg <= t1 && tg < hit.t) hit = { t: tg, chain: -1, segment: -1 }; }
  return hit.t < Infinity ? { t: hit.t, gap: 0, chain: hit.chain, segment: hit.segment } : { t: near.t, gap: Math.max(near.gap, 1e-9), chain: near.chain, segment: near.segment };
}
