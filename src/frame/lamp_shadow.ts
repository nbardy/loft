// A lamp's shadow from a depth picture taken from the lamp (DESIGN.md 17): `lamp(at, { ..., shadow: true })`.
//
// The compiler calls `lampShadows` once per compile, before anything is emitted. For each lamp that asked, it makes a
// depth pass (`scenePass`, keep "depth") whose camera is a `look` from the lamp framed each frame on the casters'
// chains (the bounding spheres castRay culls by, `chainSpheres`), and a test the light fold multiplies the lamp's
// arrival by (`foldAt`, src/light.ts). Nothing else changes: a scene without a shadowed lamp makes no pass.
//
// Why framing on the casters is enough: the camera's apex is the lamp, so its frustum is a cone, and a cone is
// star-shaped from its apex. A receiver shadowed by caster C lies on the ray from the lamp through C, so a cone holding
// every caster holds every shadow they throw; receivers outside it are lit, correctly.

import { SceneError } from "../errors.ts";
import {
  add, atan2, clamp, cos, cross, div, dot, f, fetch, floor, fract, gt, iterate, length, lt, max, min, mul, normalize, select, sin, sqrt, sub, vec2, vec3,
  type Expr, type TableSource,
} from "../expr.ts";
import { chainSpheres } from "../draw/cast.ts";
import type { Drawable } from "../compile/context.ts";
import type { Arrival, LampShadow, Light, Lighting, Receiver, ShadowTest } from "../light.ts";
import { scenePass, type Pass } from "./pass.ts";

/** Texels a side. Fixed (DESIGN.md 17.1): at the crab's 63-degree frame, about 0.045 world units at the crab. */
const TEXELS = 1024;
/** The widest the picture is framed, off its axis. Past it a texel at the edge is 8.5 times the centre's. */
const HALF_MAX = (70 * Math.PI) / 180;
/** The narrowest (a lone small caster far away): keeps the projection finite. */
const HALF_MIN = 0.01;
/** Blocker search and filter taps of a soft lamp (Vogel discs; the filter only where the search is mixed); a hard lamp takes four (bilinear PCF). */
const SEARCH = 8, FILTER = 16;
/** The widest search or filter radius, texels. */
const RADIUS_MAX = 16;

/** How a lamp's edge is filtered, classified once from its size. */
type Edge = { readonly kind: "hard" } | { readonly kind: "soft"; readonly size: number };

/** What the compiler checks at rest: the frame's half-angle for each shadowed lamp, before the cap, when its pass is read. */
export type Spread = { readonly light: number; readonly pass: string; readonly at: Expr<"vec3">; readonly half: Expr<"f32"> };

/**
 * Each lamp of `lighting` that asked for a shadow, given its depth pass and test. Returns the lighting the compiler
 * folds (with `shadows` filled) and the spreads to check at rest (`refuseSpread`). Refuses a shadowed lamp in a scene
 * where nothing has a chain to frame the picture on.
 */
export function lampShadows(lighting: Lighting, things: readonly Drawable[]): { readonly lighting: Lighting; readonly spreads: readonly Spread[] } {
  const asked = lighting.lights.flatMap((l, k) => (l.shadow === undefined ? [] : [{ light: l, k, shadow: l.shadow }]));
  if (asked.length === 0) return { lighting, spreads: [] };
  const { infos, table } = chainSpheres(things);
  if (infos.length === 0) {
    throw new SceneError(`lights[${asked[0]!.k}]: lamp shadow`, "a scene where no thing has a skeleton chain of two joints", "at least one thing with a skeleton (bodies, strands and fields have one) to frame the lamp's picture on", "drop `shadow: true`, or add the thing that should cast");
  }
  const shadows = new Map<Light, ShadowTest>(lighting.shadows);
  const spreads: Spread[] = [];
  for (const { light, k, shadow } of asked) {
    const m = mapOf(light, shadow, things, table, infos.length);
    shadows.set(light, m.test);
    spreads.push({ light: k, pass: m.pass, at: shadow.at, half: m.raw });
  }
  return { lighting: { ...lighting, shadows }, spreads };
}

type Made = { readonly test: ShadowTest; readonly pass: string; readonly raw: Expr<"f32"> };
/**
 * One map per lamp and list of things, made once: a pass's `at` is a library function, whose name must be one
 * function's in a process, so compiling the same scene again (`loft dev`, a bench) must reuse it. Maps are numbered
 * in the order made (`loft_lamp0`, ...): the same in every process that compiles one scene.
 */
const made = new WeakMap<Light, WeakMap<readonly Drawable[], Made>>();
let count = 0;
function mapOf(light: Light, shadow: LampShadow, things: readonly Drawable[], table: TableSource, n: number): Made {
  const byThings = made.get(light) ?? new WeakMap<readonly Drawable[], Made>();
  made.set(light, byThings);
  const seen = byThings.get(things);
  if (seen !== undefined) return seen;
  const frame = framing(shadow.at, table, n);
  const near = shadow.reach / 100, far = 4 * shadow.reach;
  const p = scenePass({
    name: `lamp${count++}`, size: [TEXELS, TEXELS], keep: "depth",
    camera: { kind: "look", eye: shadow.at, target: add(shadow.at, frame.axis), lens: { fovY: mul(frame.half, 2) }, near, far },
  });
  const m = { test: testOf(p.pass, p.camera, shadow, near, far), pass: p.pass.name, raw: frame.raw };
  byThings.set(things, m);
  return m;
}

/** Refuses a lamp the casters surround at rest (`half` is the spread evaluated then, radians). */
export function refuseSpread(s: Spread, at: readonly number[], half: number): void {
  if (half <= HALF_MAX) return;
  const deg = (half * 180) / Math.PI;
  throw new SceneError(`lights[${s.light}]: lamp shadow`, `the things that cast spread ${deg.toFixed(0)} degrees off the picture's axis, seen from the lamp at [${at.map((x) => x.toFixed(2)).join(", ")}]${deg >= 90 ? " (the lamp is among them)" : ""}`, `at most ${(HALF_MAX * 180) / Math.PI} degrees: one depth picture from a point covers less than a hemisphere`, "put the lamp above or beside the things, or cast its shadow in the light's own function with castRay (examples/light/area_lamp.ts)");
}

/**
 * The picture's axis and half-angle from the lamp at L over n chain spheres (centre, radius) this frame: the axis is
 * the normalised sum of the unit directions to the centres; the half-angle the widest sphere's angle off it plus its
 * own angular radius (asin(r / d), 90 degrees when L is inside it). `raw` before the cap, `half` after it.
 */
function framing(L: Expr<"vec3">, spheres: TableSource, n: number): { readonly axis: Expr<"vec3">; readonly half: Expr<"f32">; readonly raw: Expr<"f32"> } {
  const parts = Array.from({ length: n }, (_, i) => {
    const s = fetch(spheres, f(i), 0);
    const d = sub(s.xyz, L), dist = max(length(d), 1e-6);
    return { u: div(d, dist), q: min(div(s.w, dist), 1) };
  });
  const axis = normalize(parts.slice(1).reduce((a: Expr<"vec3">, p) => add(a, p.u), parts[0]!.u));
  const raw = parts.reduce((h: Expr<"f32">, p) => {
    const off = atan2(length(cross(p.u, axis)), dot(p.u, axis));
    const own = atan2(p.q, sqrt(max(sub(1, mul(p.q, p.q)), 0)));
    return max(h, add(off, own));
  }, f(0));
  return { axis, raw, half: clamp(raw, HALF_MIN, HALF_MAX) };
}

/** The receiver's normal as the test reads it: its own, or facing the light when it has none. */
function normalOf(r: Receiver, lit: Arrival): Expr<"vec3"> {
  switch (r.kind) {
    case "surface": return r.normal;
    case "point": return lit.toward;
  }
}

/** The fraction of the lamp a receiver sees past the depth picture (DESIGN.md 17.3). */
function testOf(pass: Pass, camera: TableSource, shadow: LampShadow, near: number, far: number): ShadowTest {
  const edge: Edge = shadow.size > 0 ? { kind: "soft", size: shadow.size } : { kind: "hard" };
  // `look`'s perspective stores z = -A + B / D for the depth D along the axis; so D = B / (z + A). 1 reads as far.
  const A = far / (near - far), B = (near * far) / (near - far);
  const L = shadow.at;
  return (r, lit) => {
    // The camera's rows, from the columns `project` fetches anyway: row 0 is fx right, row 1 fy up, row 3 forward (the
    // clip w of a perspective is the depth along the axis), so tan(half) = 1 / fy.
    const c = [0, 1, 2].map((k) => fetch(camera, 0, k));
    const row = (j: "x" | "y" | "w"): Expr<"vec3"> => vec3(c[0]![j], c[1]![j], c[2]![j]);
    const fy = length(row("y"));
    const right = normalize(row("x")), up = div(row("y"), fy), forward = row("w");
    const n = normalOf(r, lit);
    const depth = (p: Expr<"vec3">): Expr<"f32"> => dot(forward, sub(p, L));
    const texel = (d: Expr<"f32">): Expr<"f32"> => div(mul(d, 2 / TEXELS), fy);
    // One texel along the normal first: a silhouette seen edge-on otherwise samples its own depth.
    const p = add(r.at, mul(n, texel(depth(r.at))));
    const D = depth(p), t = texel(D);
    const q = pass.project(p).xy;
    // Bias by the receiver's own plane (through p, normal n): a tap at texel offset (dx, dy) looks along the ray from L
    // through P = p + t (dx right - dy up), which meets the plane at depth D k / n.(P - L), k = n.(p - L). Exact for a
    // plane at any tap distance. Where the ray misses the plane in front (seen edge-on) the plain D is used.
    const k = dot(n, sub(p, L));
    const facing = clamp(dot(n, lit.toward), 0.2, 1);
    // What is left: a texel's own quantisation, three quarters of a texel across the slope (tan theta), plus one.
    // tan theta from cos; 1 - cos^2 clamped at 0, since n.l of a receiver facing the lamp (n = l) rounds above 1 and a
    // NaN bias fails every comparison (it printed the whole pool on the crab's water as speckle).
    const bias = mul(t, add(1, mul(0.75, div(sqrt(max(sub(1, mul(facing, facing)), 0)), facing))));
    const tap = (off: Expr<"vec2">): { readonly receiver: Expr<"f32">; readonly blocker: Expr<"f32"> } => {
      const P = add(p, mul(sub(mul(right, off.x), mul(up, off.y)), t));
      const m = dot(n, sub(P, L));
      const ratio = select(gt(mul(m, k), 0), clamp(div(k, m), 0.5, 2), f(1));
      const z = pass.at(add(q, mul(off, 1 / TEXELS))).x;
      return { receiver: mul(D, ratio), blocker: div(B, add(z, A)) };
    };
    const lit1 = (off: Expr<"vec2">): Expr<"f32"> => { const s = tap(off); return select(lt(sub(s.receiver, s.blocker), bias), f(1), f(0)); };
    switch (edge.kind) {
      case "hard": {
        // Bilinear PCF: the four texel centres round q, each compared, weighted by where q falls between them.
        const g = sub(mul(q, TEXELS), 0.5), base = floor(g), w = fract(g);
        const at = (ox: number, oy: number) => lit1(sub(add(base, vec2(ox + 0.5, oy + 0.5)), mul(q, TEXELS)));
        const top = add(mul(at(0, 0), sub(1, w.x)), mul(at(1, 0), w.x));
        const bottom = add(mul(at(0, 1), sub(1, w.x)), mul(at(1, 1), w.x));
        return add(mul(top, sub(1, w.y)), mul(bottom, w.y));
      }
      case "soft": {
        // PCSS. Search over a radius covering blockers down to a third of D (a blocker at depth b throws a penumbra
        // of s (D - b) / b, so at b = D / 3, 2 s: in texels 2 s / t); then the penumbra from the mean blocker.
        const s = edge.size;
        const search = clamp(div(2 * s, t), 1, RADIUS_MAX);
        let sum: Expr<"f32"> = f(0), count: Expr<"f32"> = f(0);
        for (const o of vogel(SEARCH)) {
          const h = tap(mul(vec2(o[0], o[1]), search));
          const blocked = gt(sub(h.receiver, h.blocker), bias);
          sum = add(sum, select(blocked, h.blocker, f(0)));
          count = add(count, select(blocked, f(1), f(0)));
        }
        const mean = div(sum, max(count, 1));
        const penumbra = clamp(div(mul(sub(D, mean), s), mul(max(mean, near), t)), 1, RADIUS_MAX);
        // The filter only where the search was mixed: no search tap blocked is lit, every one blocked is in umbra (the
        // filter's disc lies inside the search's whenever the mean blocker is deeper than D / 3). A loop whose count
        // is 0 there, not an unrolled sum under a select, so most pixels (the open water) skip its 16 loads.
        const mixed = select(gt(count, 0), select(lt(count, SEARCH), f(FILTER), f(0)), f(0));
        const [seen] = iterate(mixed, [f(0)] as const, ([acc], i) => {
          const r = mul(sqrt(mul(add(i, 0.5), 1 / FILTER)), penumbra), a = mul(i, GOLDEN);
          return [add(acc, lit1(mul(vec2(cos(a), sin(a)), r)))] as const;
        });
        return select(gt(count, 0), select(lt(count, SEARCH), mul(seen, 1 / FILTER), f(0)), f(1));
      }
    }
  };
}

/** n points of a Vogel disc of radius 1 (golden-angle spiral, even in area): fixed, so a flat style shows no grain. */
const GOLDEN = Math.PI * (3 - Math.sqrt(5));
function vogel(n: number): readonly (readonly [number, number])[] {
  const golden = GOLDEN;
  return Array.from({ length: n }, (_, k) => {
    const r = Math.sqrt((k + 0.5) / n), a = k * golden;
    return [r * Math.cos(a), r * Math.sin(a)] as const;
  });
}
