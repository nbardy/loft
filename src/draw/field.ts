// draw.field: a solid defined by a distance function, sphere-traced per pixel inside a bound (docs/folding_and_tracing.md
// section 3.1). The bound is drawn as triangles (trace.ts); the fragment stage marches the view ray between the bound's
// entry and exit, stepping by the field's distance, and writes colour, depth and coverage, so a traced solid meets
// triangle bodies and the sea exactly as a triangle body would (the sea's contact foam reads scene depth and needs
// nothing from here).
//
// The march. A hit is a distance below HIT pixels at that range (the pixel's world size is affine along a ray, so it
// costs one multiply-add per step): far away, small folds fall below a pixel and the march ends early. Steps are capped;
// a ray that runs out of steps inside the bound takes the point reached as its hit (it is crawling along a surface),
// which leaves no hole. A hit is refined by REFINE secant steps on the last two distances, which lands within 2e-4 of
// the analytic intersection where plain steps left 7e-3 at oblique hits. Normals are the field's symbolic gradient
// (grad.ts), evaluated once at the hit; paint sees the hit point and normal (and may evaluate any fold there for its
// cell identity).
//
// Level of detail. A field may read `fieldPixel`, the world size of one pixel at the point being evaluated, to choose
// its fold depth (an `iterate` count), so depth falls with distance along each ray. (It was the pixel size where the
// ray enters the bound, constant per ray; with the camera inside the bound that is the eye's, zero, and far twigs were
// drawn at full depth as sub-pixel grain.) Changing depth along a ray is safe for recursions where fewer levels draw a
// subset of the parts (`grow`): a coarse step can only pass parts that are below a pixel there. On the CPU it is 0
// (full detail) and, for the safety check, also the bound's size (the coarsest).
//
// Edges. Between two samples the surface can come no closer to the ray than where the two spheres of free space
// meet; that closest-approach estimate (not the sampled distances, which drew silhouettes and ink as random grain)
// decides coverage and ink. Within a pixel of the hit threshold a pixel is partly covered (sample mask, trace.ts),
// shaded at the closest point. With `outline: "silhouette"` a ray that passes within the ink width of a surface while
// moving away from it is ink: the first such near miss along the ray, so inner silhouettes (a branch in front of
// another) get ink too. Ink is the style's width, at least 1.5 and at most 4 pixels (trace.ts `ink_px`).
//
// The safety check (section 6): at compile time the field is sampled on the CPU with `lipschitz` inside its bound; a
// field that can overshoot (measured constant above 1) fails with `FieldSafetyError`, which names the field and the
// measured value and suggests a step scale. Passing `safety` scales every step by it and skips the check: a deliberate
// statement that the field is known to need (or not need) it.

import { nameId } from "./surface.ts";
import { lit } from "../compile/scene_wgsl.ts";
import { div, f, length, libsOf, max, substitute, variable, type Expr } from "../expr.ts";
import { gradient } from "../grad.ts";
import { emitShaderFn } from "../compile/bind.ts";
import { drawSpec, type Bakeable, type CompileContext, type Drawable, type Emitted } from "../compile/context.ts";
import { lipschitz } from "../lib/sdf3.ts";
import { fieldShadow, shadowSkeleton } from "./field_shadow.ts";
import type { F, Field, P3, Skeleton } from "../map/types.ts";
import { aabbOf, outlineWgsl, PROXY_VERTICES, traceWgsl, type TraceBound, type TraceOutline } from "./trace.ts";

/**
 * The world size of one pixel at the point the field is evaluated at. A field may read it to choose its level of detail
 * (fold depth). Bound per step on the GPU; 0 (full detail) on the CPU.
 */
export const fieldPixel: F = variable("loft_field_px", "f32");

/** What `paint` sees at a hit: the world point and the unit outward normal. */
export type FieldHit = { readonly p: P3; readonly normal: P3 };

export type FieldOptions = {
  /** Where the field is marched: its proxy triangles and the ray's span. The solid must lie inside it. */
  readonly bounds: TraceBound;
  /** Base colour at a hit (lit by the style). To vary per copy, evaluate the fold at `hit.p` and use its `id`. */
  readonly paint: (hit: FieldHit) => P3;
  /** Most sphere-tracing steps per pixel. */
  readonly steps: number;
  /** Ink on silhouettes (the toon outline of a traced solid), or none. */
  readonly outline: TraceOutline;
  /**
   * Absent: the field is checked at compile time and must not overshoot. Present: every step is scaled by this factor
   * in (0, 1] and the check is skipped (for fields whose bound is known, or that the CPU cannot evaluate).
   */
  readonly safety?: number;
};

/** Why a field was refused at compile time. */
export type FieldSafetyFailure =
  /** Sampled Lipschitz constant above 1: steps by this distance can pass through the surface (holes). */
  | { readonly kind: "overshoots"; readonly measured: number; readonly suggested: number }
  /** The field calls a GPU-only function, so the CPU cannot check it. */
  | { readonly kind: "unevaluable"; readonly fn: string };

const describe = (field: string, r: FieldSafetyFailure): string => {
  switch (r.kind) {
    case "overshoots": return `draw.field "${field}": the field can overshoot: its measured Lipschitz constant is ${r.measured.toFixed(3)} (a distance must not change faster than 1 per unit). Fix the field (a fold's stretch factor missing?), or pass safety: ${r.suggested.toFixed(3)} to scale its steps.`;
    case "unevaluable": return `draw.field "${field}": cannot check the field on the CPU: it calls the GPU-only function "${r.fn}". Pass an explicit safety factor to march it unchecked.`;
  }
};

export class FieldSafetyError extends Error {
  override readonly name = "FieldSafetyError";
  constructor(readonly field: string, readonly reason: FieldSafetyFailure) {
    super(describe(field, reason));
  }
}

/** Largest sampled Lipschitz constant accepted. Sampled in f64; a true distance measures at most 1 + 1e-9. */
export const LIPSCHITZ_LIMIT = 1.01;
/** Pairs sampled per level of detail. */
const CHECK_SAMPLES = 20000;
/** Hit when the distance is below this many pixels. */
const HIT = 0.3;
/** Refining steps taken after a hit (secant steps on the distance along the ray). */
const REFINE = 2;

const P = variable("p", "vec3"), N = variable("n", "vec3");

/** The sampled Lipschitz constant of a field, worst over full detail and the coarsest detail in its bound. */
export function measureField(field: Field, b: TraceBound): number {
  const box = aabbOf(b);
  // A margin round the bound, so seams at its faces are sampled too.
  const pad = 0.02 * Math.hypot(box.max[0] - box.min[0], box.max[1] - box.min[1], box.max[2] - box.min[2]);
  const bounds = { min: box.min.map((x) => x - pad), max: box.max.map((x) => x + pad) };
  const diag = Math.hypot(...bounds.max.map((x, i) => x - bounds.min[i]));
  return Math.max(...[0, diag].map((px, k) => lipschitz((p: P3) => substitute([field(p)], fieldPixel, f(px))[0] as F, { samples: CHECK_SAMPLES, bounds, seed: 0x5eed + k })));
}

/** Check a field the way compilation does: throws FieldSafetyError, or returns the measured constant. */
export function checkField(name: string, field: Field, b: TraceBound): number {
  const gpuOnly = libsOf([field(P)]).find((l) => l.js === "gpu-only");
  if (gpuOnly) throw new FieldSafetyError(name, { kind: "unevaluable", fn: gpuOnly.name });
  const measured = measureField(field, b);
  if (measured > LIPSCHITZ_LIMIT) throw new FieldSafetyError(name, { kind: "overshoots", measured, suggested: 0.95 / measured });
  return measured;
}

export class FieldDrawable implements Drawable {
  private shadow: Skeleton[] | undefined;
  /** A soft blob fitted to the solid (src/draw/field_shadow.ts), found on first use: sampling costs a few ms. */
  get skeletons(): readonly Skeleton[] {
    this.shadow ??= [shadowSkeleton(fieldShadow(this.field, this.o.bounds, fieldPixel))];
    return this.shadow;
  }
  /** Traced: there is no surface to bake. */
  readonly surfaces: readonly Bakeable[] = [];

  constructor(readonly name: string, readonly field: Field, readonly o: FieldOptions) {
    if (!/^[A-Za-z][A-Za-z0-9_]*$/.test(name)) throw new Error(`draw.field "${name}": names must be identifiers (they name shaders)`);
    if (!(Number.isInteger(o.steps) && o.steps >= 1)) throw new Error(`draw.field "${name}": steps must be a positive integer, got ${o.steps}`);
    if (o.safety !== undefined && !(o.safety > 0 && o.safety <= 1)) throw new Error(`draw.field "${name}": safety must be in (0, 1], got ${o.safety}`);
  }

  /** The step scale: the explicit one, or 1 once the field passes the check. */
  private stepScale(): number {
    if (this.o.safety !== undefined) return this.o.safety;
    checkField(this.name, this.field, this.o.bounds);
    return 1;
  }

  draws(ctx: CompileContext): Emitted {
    const safety = this.stepScale();
    const args = [["p", "vec3"], ["loft_field_px", "f32"]] as const;
    const d = this.field(P);
    const g = gradient(d, P);
    const field = emitShaderFn(ctx.slots, { name: "field_d", args, out: "f32", roots: [d], ret: (v) => v[0] });
    const normal = emitShaderFn(ctx.slots, { name: "field_n", args, out: "vec3<f32>", roots: [div(g, max(length(g), 1e-12))], ret: (v) => v[0] });
    const paint = emitShaderFn(ctx.slots, {
      name: "field_paint", args: [["p", "vec3"], ["n", "vec3"], ["loft_field_px", "f32"]], out: "vec3<f32>",
      roots: [this.o.paint({ p: P, normal: N as Expr<"vec3"> })], ret: (v) => v[0],
    });
    const key = `field.${this.name}`;
    const code = [traceWgsl(this.o.bounds), outlineWgsl(this.o.outline), `const FIELD_ID: f32 = ${lit(nameId(this.name))};`, field.code, normal.code, paint.code, fragmentWgsl(safety)].join("\n");
    return {
      shaders: { [key]: { code, libs: [...new Set([...field.libs, ...normal.libs, ...paint.libs])] } },
      draws: [drawSpec({ label: this.name, shader: key, vertices: PROXY_VERTICES, cull: "front", constants: { STEPS: this.o.steps } })],
    };
  }
}

const num = (x: number): string => (Number.isInteger(x) ? `${x}.0` : String(x));

function fragmentWgsl(safety: number): string {
  return /* wgsl */ `
override STEPS: u32 = 64u;
const SAFETY: f32 = ${num(safety)};
const HIT: f32 = ${num(HIT)};
// The style's shading takes screen-space derivatives (band edges) after a loop whose exit differs per pixel. Every
// invocation of a quad is still running there (a missed ray is discarded only at the end, which demotes it to a
// helper), so the derivatives are defined; the uniformity analysis cannot see that.
@diagnostic(off, derivative_uniformity)
@fragment fn fs(in: TraceIn) -> TraceOut {
  let d = view_dir(in.world);
  let o = trace_origin(in.world, d);
  let span = bound_span(o, d);
  let t0 = max(span.x, 0.0);
  let t1 = span.y;
  if (t0 > t1) { discard; }
  // Pixel size is affine along the ray (it is proportional to clip w).
  let fp0 = pixel_world(o + d * t0);
  let fpk = (pixel_world(o + d * max(t1, t0 + 1e-3)) - fp0) / max(t1 - t0, 1e-3);
  var t = t0;
  var hit = false;
  var nearT = t0;        // where the ray came closest (in pixels) without hitting
  var nearPx = 1e30;
  var inked = false;
  var prevDist = 0.0;
  var prevT = t0;
  var exhausted = true;
  var hitDist = 0.0;
  for (var i = 0u; i < STEPS; i++) {
    let fp = fp0 + fpk * (t - t0);
    let dist = field_d(o + d * t, fp);
    if (dist < HIT * fp) { hit = true; hitDist = dist; exhausted = false; break; }
    // The closest the surface can come to the ray between this sample and the last: it lies outside both spheres of
    // free space, so at most at the circle where they meet, x back from here and h from the ray. (The sampled
    // distances alone miss the closest approach by up to a step, which drew silhouettes and ink as random grain.)
    let s = t - prevT;
    let x = (dist * dist - prevDist * prevDist + s * s) / max(2.0 * s, 1e-20);
    let between = i > 0u && x > 0.0 && x < s;
    let h = select(dist, sqrt(max(dist * dist - x * x, 0.0)), between);
    let th = select(t, t - x, between);
    let hPx = h / (fp0 + fpk * (th - t0));
    // A near miss: the ray passed within the ink width of a surface and is moving away from it. In front of anything
    // further on.
    if (INK_ON && between && dist > prevDist && hPx < ink_px(fp0 + fpk * (th - t0))) { inked = true; nearT = th; nearPx = hPx; exhausted = false; break; }
    if (hPx < nearPx) { nearPx = hPx; nearT = th; }
    prevDist = dist;
    prevT = t;
    t += dist * SAFETY;
    if (t > t1) { exhausted = false; break; }
  }
  // Out of steps while still inside the bound: the march is crawling along a surface (grazing, or a crease). Taking
  // the point reached as the hit leaves no hole; the error is under a pixel because the distance is already small.
  hit = hit || exhausted;
  if (hit) {
    // Secant steps on the last two samples: the distance is nearly linear along the ray at the surface, so this
    // converges fast even at an oblique hit, where a plain step (t += d) only removes a share cos(angle) of the error
    // per step (that left 7e-3 world units at 3 pixels in from a sphere's silhouette).
    var ta = prevT;
    var da = prevDist;
    var db = hitDist;
    for (var r = 0u; r < ${REFINE}u; r++) {
      let slope = (db - da) / (t - ta);
      let tn = select(t + db * SAFETY, t - db / slope, slope < -1e-6 && t > ta);
      ta = t;
      da = db;
      t = tn;
      db = field_d(o + d * t, fp0 + fpk * (t - t0));
    }
  }
  let tq = select(nearT, t, hit);
  let p = o + d * tq;
  let fp = fp0 + fpk * (tq - t0);
  let n = field_n(p, fp);
  let base = field_paint(p, n, fp);
  let inkPx = ink_px(fp);
  // Coverage: full on a hit; on a miss, falling from full to none over one pixel beyond the hit threshold (or beyond
  // the ink band).
  let edge = select(HIT, inkPx, INK_ON);
  let cover = select(clamp(1.0 + edge - nearPx, 0.0, 1.0), 1.0, hit || inked);
  let useInk = INK_ON && !hit;
  let colour = select(trace_shade(base, n, p, d, FIELD_ID), trace_ink(base, n, p, d, FIELD_ID), useInk);
  let mask = trace_mask(cover, in.clip.xy);
  if (mask == 0u) { discard; }
  return TraceOut(vec4<f32>(colour, trace_mark(base, n, p, d, FIELD_ID)), trace_depth(p), mask);
}`;
}

/** A distance field, sphere-traced inside its bound. */
export function drawField(name: string, field: Field, o: FieldOptions): FieldDrawable {
  return new FieldDrawable(name, field, o);
}
