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
import { compileJs, div, f, length, libsOf, max, mul, substitute, variable, type Expr, type TableSource } from "../expr.ts";
import { gradient } from "../grad.ts";
import { emitShaderFn } from "../compile/bind.ts";
import { drawSpec, type Bakeable, type CompileContext, type Drawable, type DrawSpec, type Emitted } from "../compile/context.ts";
import { lipschitz } from "../lib/sdf3.ts";
import { fieldShadow, shadowSkeleton } from "./field_shadow.ts";
import type { F, Field, P3, Skeleton } from "../map/types.ts";
import { aabbOf, outlineWgsl, PROXY_VERTICES, traceShadeWgsl, traceWgsl, type TraceBound, type TraceOutline } from "./trace.ts";
import { bakeBrick, varianceOf, type Brick, type Variance } from "../map/brick.ts";
import { surfaceNets, type TriangleMesh } from "../map/nets.ts";

/**
 * The world size of one pixel at the point the field is evaluated at. A field may read it to choose its level of detail
 * (fold depth). Bound per step on the GPU; 0 (full detail) on the CPU.
 */
export const fieldPixel: F = variable("loft_field_px", "f32");

/**
 * Where the field is evaluated (DESIGN.md 7.7). `trace`: the formula at every step of every pixel. `brick`: the field is
 * sampled once into a distance brick (src/map/brick.ts) and the march steps by the brick, evaluating the formula only
 * within a few pixels of the surface, for the hit, the normal and the paint. Only a static field can be bricked: one
 * that reads time, an input, a param, a curve or a per-frame table is refused by name (`FieldScheduleError`).
 */
export type FieldSchedule = "trace" | "brick" | "mesh" | "auto";

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
  /** How the field is drawn (see `FieldSchedule`). */
  readonly schedule: FieldSchedule;
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

/** A schedule that needs a static field was asked of one that is not. */
export class FieldScheduleError extends Error {
  override readonly name = "FieldScheduleError";
  constructor(readonly field: string, readonly schedule: FieldSchedule, readonly variance: Exclude<Variance, { kind: "static" }>) {
    super(`draw.field "${field}": schedule "${schedule}" samples the field once, but ${variance.kind === "changing" ? `it reads ${variance.reads.join(", ")}, which can change after compilation` : `it calls the GPU-only function "${variance.fn}", which the CPU cannot sample`}. Use schedule "trace".`);
  }
}

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

/**
 * Fine brick cells along the bound's diagonal. The stated error: where the field is within a band of the surface the
 * brick's bound is at most one cell diagonal below it (diag(bound) sqrt(3) / BRICK_CELLS: 0.68% of the bound's
 * diagonal); elsewhere it is a lower bound and at least the band.
 */
export const BRICK_CELLS = 256;
/** Shell round the surface sampled exactly, in brick cells. */
const BRICK_BAND = 1;

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

/**
 * What a field's schedule resolves to, once its leaves are known: traced by its formula, traced through its brick,
 * drawn as its mesh, or both of the last two with the choice made per frame by its size on screen.
 */
type Plan =
  | { readonly kind: "trace" }
  | { readonly kind: "brick"; readonly brick: Brick }
  | { readonly kind: "mesh"; readonly brick: Brick; readonly mesh: TriangleMesh }
  | { readonly kind: "switch"; readonly brick: Brick; readonly mesh: TriangleMesh };

export class FieldDrawable implements Drawable {
  private shadow: Skeleton[] | undefined;
  private planned: Plan | undefined;
  /** A soft blob fitted to the solid (src/draw/field_shadow.ts), found on first use: sampling costs a few ms. */
  get skeletons(): readonly Skeleton[] {
    this.shadow ??= [shadowSkeleton(fieldShadow(this.field, this.o.bounds, fieldPixel))];
    return this.shadow;
  }
  /** A field drawn as a mesh bakes as that mesh (paint evaluated at its vertices); a traced one has nothing to bake. */
  get surfaces(): readonly Bakeable[] {
    const p = this.plan();
    switch (p.kind) {
      case "trace": case "brick": return [];
      // At full detail: the baker has no pixel size.
      case "mesh": case "switch": return [{ name: this.name, mesh: p.mesh, paint: (hit: FieldHit) => substitute([this.o.paint(hit)], fieldPixel, f(0))[0] as P3 }];
    }
  }

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

  /** The field at full detail (the pixel size 0), which every coarser level of detail lies above. */
  private full(): Expr<"f32"> {
    return substitute([this.field(P)], fieldPixel, f(0))[0] as Expr<"f32">;
  }

  /** Whether the field can be sampled once (by its leaves: nothing is asked of the author). */
  variance(): Variance {
    return varianceOf(this.full());
  }

  /**
   * The schedule resolved, baking on first use (seconds of CPU for a large field; a scene compiled twice in one process
   * bakes once). The brick and the mesh are of the field scaled by its step scale, so the brick's cones are those of a
   * 1-Lipschitz function.
   */
  plan(): Plan {
    if (this.planned) return this.planned;
    const v = this.variance();
    const sampled = (): { brick: Brick; mesh: () => TriangleMesh } => {
      if (v.kind !== "static") throw new FieldScheduleError(this.name, this.o.schedule, v);
      const safety = this.stepScale();
      const box = aabbOf(this.o.bounds);
      const diag = Math.hypot(box.max[0] - box.min[0], box.max[1] - box.min[1], box.max[2] - box.min[2]);
      const cell = diag / BRICK_CELLS;
      const d = mul(this.full(), safety);
      const brick = bakeBrick(`${this.name}_brick`, d, "p", { box, cell, band: BRICK_BAND * cell });
      const run = compileJs([d]);
      const vars = { p: [0, 0, 0] };
      const env = { vars, inputs: {}, params: {} };
      const cpu = (x: number, y: number, z: number): number => { vars.p = [x, y, z]; return run(env)[0] as number; };
      return { brick, mesh: () => surfaceNets(brick, cpu) };
    };
    const made = ((): Plan => {
      switch (this.o.schedule) {
        case "trace": return { kind: "trace" };
        case "brick": return { kind: "brick", brick: sampled().brick };
        case "mesh": { const b = sampled(); return { kind: "mesh", brick: b.brick, mesh: b.mesh() }; }
        // A field that changes (or that the CPU cannot sample) is traced; a static one is sampled and switches.
        case "auto": {
          if (v.kind !== "static") return { kind: "trace" };
          const b = sampled();
          return { kind: "switch", brick: b.brick, mesh: b.mesh() };
        }
      }
    })();
    this.planned = made;
    return made;
  }

  draws(ctx: CompileContext): Emitted {
    const safety = this.stepScale();
    const p = this.plan();
    const args = [["p", "vec3"], ["loft_field_px", "f32"]] as const;
    const d = this.field(P);
    const field = emitShaderFn(ctx.slots, { name: "field_d", args, out: "f32", roots: [d], ret: (v) => v[0] });
    const paint = emitShaderFn(ctx.slots, {
      name: "field_paint", args: [["p", "vec3"], ["n", "vec3"], ["loft_field_px", "f32"]], out: "vec3<f32>",
      roots: [this.o.paint({ p: P, normal: N as Expr<"vec3"> })], ret: (v) => v[0],
    });
    const g = gradient(d, P);
    const normal = emitShaderFn(ctx.slots, { name: "field_n", args, out: "vec3<f32>", roots: [div(g, max(length(g), 1e-12))], ret: (v) => v[0] });
    const id = `const FIELD_ID: f32 = ${lit(nameId(this.name))};`;
    const libs = [...new Set([...field.libs, ...normal.libs, ...paint.libs])];
    // The proxy and its march: the field's formula at every step, or steps by its brick.
    const traced = (skip: string, show: string): Emitted => {
      const key = `field.${this.name}`;
      const code = [traceWgsl(this.o.bounds, show), outlineWgsl(this.o.outline), id, field.code, normal.code, paint.code, skip, fragmentWgsl(safety)].join("\n");
      return {
        shaders: { [key]: { code, libs } },
        draws: [drawSpec({ label: this.name, shader: key, vertices: PROXY_VERTICES, cull: "front", constants: { STEPS: this.o.steps } })],
      };
    };
    const meshed = (mesh: TriangleMesh, show: string): Emitted => {
      const vertices = ctx.table(meshTable(`${this.name}_mesh`, mesh));
      const tris = ctx.table(indexTable(`${this.name}_tris`, mesh));
      const key = `field.${this.name}.mesh`;
      const code = [traceShadeWgsl(), outlineWgsl(this.o.outline), id, paint.code, meshWgsl(vertices.wgsl, tris.wgsl, show)].join("\n");
      const n = mesh.indices.length;
      const ink: DrawSpec[] = this.o.outline === "silhouette" ? [drawSpec({ label: `${this.name}_ink`, shader: key, vertices: n, cull: "front", constants: { INK_HULL: 1 } })] : [];
      return {
        shaders: { [key]: { code, libs: paint.libs } },
        draws: [drawSpec({ label: this.name, shader: key, vertices: n, cull: "back", constants: { INK_HULL: 0 } }), ...ink],
      };
    };
    const both = (a: Emitted, b: Emitted): Emitted => ({ shaders: { ...a.shaders, ...b.shaders }, draws: [...a.draws, ...b.draws] });
    switch (p.kind) {
      case "trace": return traced(TRACE_SKIP, "true");
      case "brick": return traced(brickSkip(ctx, p.brick), "true");
      case "mesh": return meshed(p.mesh, "true");
      case "switch": {
        const choose = useMeshWgsl(this.o.bounds, p.brick.cell);
        return both(traced(`${choose}\n${brickSkip(ctx, p.brick)}`, "!field_use_mesh()"), meshed(p.mesh, "field_use_mesh()"));
      }
    }
  }
}

/** The trace schedule's skip: none (every step is the field's own). */
const TRACE_SKIP = /* wgsl */ `
fn march_skip(o: vec3<f32>, d: vec3<f32>, t: f32, t1: f32, fp0: f32, fpk: f32, t0: f32) -> vec2<f32> { return vec2<f32>(t, 0.0); }`;

/**
 * WGSL `fn march_skip(o, d, t, t1, fp0, fpk, t0) -> vec2<f32>`: from t, the furthest the ray can go without coming within
 * the band of the surface, proved by the brick's bound (already scaled by SAFETY when it was baked), and how many steps
 * that took. A whole run of brick steps is one call, so a SIMD group whose rays are mostly in free space does not
 * evaluate the field once per brick step for the one ray that needs it. Over-relaxed (Keinert et al. 2014): a step of
 * OMEGA times the bound, taken back whenever this sample's sphere of free space and the last one's do not overlap.
 */
function brickSkip(ctx: CompileContext, b: Brick): string {
  ctx.table(b.table);
  return /* wgsl */ `${b.wgsl("brick_lower")}
const OMEGA: f32 = 1.6;
fn march_skip(o: vec3<f32>, d: vec3<f32>, ts: f32, t1: f32, fp0: f32, fpk: f32, t0: f32) -> vec2<f32> {
  var t = ts;
  var n = 0.0;
  var w = OMEGA;
  var stepLen = 0.0;
  var prevR = 0.0;
  for (var k = 0u; k < 48u; k++) {
    let r = brick_lower(o + d * t);
    if (w > 1.0 && r + prevR < stepLen) {
      // The relaxed step overshot what the last sphere covered: back to its edge, unrelaxed from here on.
      t -= stepLen - prevR;
      stepLen = prevR;
      w = 1.0;
      continue;
    }
    if (r <= march_band(fp0 + fpk * (t - t0))) { break; }
    stepLen = r * w;
    prevR = r;
    t += stepLen;
    n += 1.0;
    if (t > t1) { break; }
  }
  return vec2<f32>(t, n);
}`;
}

/**
 * Brick cells per pixel below which a field switching between its mesh and its brick is drawn as its mesh: the mesh's
 * cell is then small on screen, so its facets and lost sub-cell twigs are below what the eye resolves there.
 */
const MESH_PX = 1.5;

/** WGSL `fn field_use_mesh() -> bool`: true when the brick's cell, at the bound's nearest point to the eye, is small. */
function useMeshWgsl(b: TraceBound, cell: number): string {
  const box = aabbOf(b);
  const c = [0, 1, 2].map((k) => (box.min[k]! + box.max[k]!) / 2);
  const r = Math.hypot(...[0, 1, 2].map((k) => (box.max[k]! - box.min[k]!) / 2));
  return /* wgsl */ `
fn field_use_mesh() -> bool {
  let c = vec3<f32>(${c.map(num).join(", ")});
  let away = frame.eye.xyz - c;
  let dist = length(away);
  // The bound's nearest point (its bounding sphere's), or its centre with the eye inside.
  let near = c + away * (max(dist - ${num(r)}, 0.0) / max(dist, 1e-6));
  return ${num(cell)} < ${num(MESH_PX)} * pixel_world(near);
}`;
}

/** A static table of the mesh's vertices: position then normal, one vec4 each. */
function meshTable(name: string, m: TriangleMesh): TableSource {
  const n = m.positions.length / 3;
  const data = new Float32Array(n * 8);
  for (let i = 0; i < n; i++) {
    data.set(m.positions.subarray(i * 3, i * 3 + 3), i * 8);
    data.set(m.normals.subarray(i * 3, i * 3 + 3), i * 8 + 4);
  }
  return { name, lanes: 2, entries: Math.max(1, n), role: "static", prepare: () => (_frame, out) => out.set(data) };
}

/** A static table of the mesh's triangle corners, four vertex indices per vec4 (exact as f32 below 2^24). */
function indexTable(name: string, m: TriangleMesh): TableSource {
  if (m.positions.length / 3 >= 2 ** 24) throw new Error(`mesh "${name}": ${m.positions.length / 3} vertices; indices are stored as f32, exact below 2^24`);
  const data = new Float32Array(Math.max(4, Math.ceil(m.indices.length / 4) * 4));
  data.set(m.indices);
  return { name, lanes: 1, entries: data.length / 4, role: "static", prepare: () => (_frame, out) => out.set(data) };
}

/**
 * The mesh's module: the vertex stage reads a triangle corner's index and its vertex from the static tables; the fragment
 * stage paints per pixel at the interpolated world point and normal and lights it as a traced point would be. With
 * INK_HULL the same triangles are pushed out along their normals by the traced ink width and drawn back faces only, in
 * ink: the inverted hull a triangle body's outline is (there is no near miss to measure on a raster).
 */
function meshWgsl(vertices: string, tris: string, show: string): string {
  return /* wgsl */ `
override INK_HULL: u32 = 0u;
struct MeshIn { @builtin(position) clip: vec4<f32>, @location(0) world: vec3<f32>, @location(1) normal: vec3<f32> };
@vertex fn vs(@builtin(vertex_index) vi: u32) -> MeshIn {
  let k = u32(${tris}(vi >> 2u)[vi & 3u]);
  let n = ${vertices}(k * 2u + 1u).xyz;
  var p = ${vertices}(k * 2u).xyz;
  if (INK_HULL == 1u) { let fp = pixel_world(p); p += n * ink_px(fp) * fp; }
  return MeshIn(select(HIDDEN_VERTEX, frame.view_proj * vec4<f32>(p, 1.0), ${show}), p, n);
}
// The paint may run a loop whose length differs per pixel (a fold's level of detail) before the style's derivatives.
@diagnostic(off, derivative_uniformity)
@fragment fn fs(in: MeshIn) -> @location(0) vec4<f32> {
  let n = normalize(in.normal);
  let d = view_dir(in.world);
  let fp = pixel_world(in.world);
  let base = field_paint(in.world, n, fp);
  let colour = select(trace_shade(base, n, in.world, d, FIELD_ID), trace_ink(base, n, in.world, d, FIELD_ID), INK_HULL == 1u);
  return vec4<f32>(colour, trace_mark(base, n, in.world, d, FIELD_ID));
}`;
}

const num = (x: number): string => (Number.isInteger(x) ? `${x}.0` : String(x));

function fragmentWgsl(safety: number): string {
  return /* wgsl */ `
override STEPS: u32 = 64u;
const SAFETY: f32 = ${num(safety)};
const HIT: f32 = ${num(HIT)};
/// Within this of the surface only the field's own samples may be used: hits, near misses (ink) and coverage are
/// decided by them. The ink or hit band plus the pixel coverage falls over, widened by the share a step can cut (two
/// spheres of free space meet no closer than 0.87 of the smaller radius).
fn march_band(fp: f32) -> f32 {
  return (select(HIT, ink_px(fp), INK_ON) + 1.5) * fp * 1.2;
}
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
  var prevExact = false;
  var exhausted = true;
  var hitDist = 0.0;
  for (var i = 0u; i < STEPS; i++) {
    // Free space first: a cheaper bound (a brick) may prove the next stretch of the ray clear of the band round the
    // surface; then one sample of the field itself, which alone decides hits, near misses and coverage.
    let skip = march_skip(o, d, t, t1, fp0, fpk, t0);
    if (skip.y > 0.0) { t = skip.x; prevExact = false; if (t > t1) { exhausted = false; break; } }
    let fp = fp0 + fpk * (t - t0);
    let dist = field_d(o + d * t, fp);
    if (dist < HIT * fp) { hit = true; hitDist = dist; exhausted = false; break; }
    // The closest the surface can come to the ray between this sample and the last: it lies outside both spheres of
    // free space, so at most at the circle where they meet, x back from here and h from the ray. (The sampled
    // distances alone miss the closest approach by up to a step, which drew silhouettes and ink as random grain.)
    let s = t - prevT;
    let x = (dist * dist - prevDist * prevDist + s * s) / max(2.0 * s, 1e-20);
    let between = prevExact && x > 0.0 && x < s;
    let h = select(dist, sqrt(max(dist * dist - x * x, 0.0)), between);
    let th = select(t, t - x, between);
    let hPx = h / (fp0 + fpk * (th - t0));
    // A near miss: the ray passed within the ink width of a surface and is moving away from it. In front of anything
    // further on.
    if (INK_ON && between && dist > prevDist && hPx < ink_px(fp0 + fpk * (th - t0))) { inked = true; nearT = th; nearPx = hPx; exhausted = false; break; }
    if (hPx < nearPx) { nearPx = hPx; nearT = th; }
    prevDist = dist;
    prevT = t;
    prevExact = true;
    t += dist * SAFETY;
    if (t > t1) { exhausted = false; break; }
  }
  // Out of field evaluations while still inside the bound: the march is crawling along a surface (grazing, or a
  // crease). Taking the point reached as the hit leaves no hole; the error is under a pixel because the distance is
  // already small.
  hit = hit || exhausted;
  if (hit) {
    // Secant steps on the last two samples: the distance is nearly linear along the ray at the surface, so this
    // converges fast even at an oblique hit, where a plain step (t += d) only removes a share cos(angle) of the error
    // per step (that left 7e-3 world units at 3 pixels in from a sphere's silhouette). Only between two of the field's
    // own samples: a brick's bound lies below the field, which would make the secant overshoot.
    var ta = prevT;
    var da = prevDist;
    var db = hitDist;
    var pairExact = prevExact;
    for (var r = 0u; r < ${REFINE}u; r++) {
      let slope = (db - da) / (t - ta);
      let tn = select(t + db * SAFETY, t - db / slope, pairExact && slope < -1e-6 && t > ta);
      ta = t;
      da = db;
      t = tn;
      db = field_d(o + d * t, fp0 + fpk * (t - t0));
      pairExact = true;
    }
  }
  let tq = select(nearT, t, hit);
  let p = o + d * tq;
  let fp = fp0 + fpk * (tq - t0);
  let inkPx = ink_px(fp);
  // Coverage: full on a hit; on a miss, falling from full to none over one pixel beyond the hit threshold (or beyond
  // the ink band).
  let edge = select(HIT, inkPx, INK_ON);
  let cover = select(clamp(1.0 + edge - nearPx, 0.0, 1.0), 1.0, hit || inked);
  // The normal and paint cost several evaluations of the field; a ray that covers nothing is discarded below, so it
  // skips them (most of a bound's pixels are clean misses). Its stand-ins only reach the derivatives the style takes
  // in a neighbouring covered pixel, at a silhouette, where the old values were another surface's anyway.
  var n = -d;
  var base = vec3<f32>(0.0);
  if (cover > 0.0) {
    n = field_n(p, fp);
    base = field_paint(p, n, fp);
  }
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
