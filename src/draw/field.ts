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
// the analytic intersection where plain steps left 7e-3 at oblique hits. Normals are the field's gradient by
// differences over a quarter pixel (`NORMAL_WGSL`), evaluated once at the hit; paint sees the hit point and normal (and
// may evaluate any fold there for its cell identity).
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
// Schedules (`FieldSchedule`, DESIGN.md 7.7). The march's free-space steps come from `march_skip`: none when traced; a
// run of steps by the brick's lower bound when bricked, after which one sample of the field itself decides the rest, as
// above. STEPS bounds the field's own samples. A meshed field draws its triangles instead of the proxy.
//
// The safety check (section 6): at compile time the field is sampled on the CPU with `lipschitz` inside its bound; a
// field that can overshoot (measured constant above 1) fails with `FieldSafetyError`, which names the field and the
// measured value and suggests a step scale. Passing `safety` scales every step by it and skips the check: a deliberate
// statement that the field is known to need (or not need) it.

import { nameId } from "./surface.ts";
import { rayDraw, type RecordContext, type Recorded } from "./record.ts";
import { march } from "./march.ts";
import { lit } from "../compile/scene_wgsl.ts";
import { compileJs, f, leaves, libsOf, mul, topo, substitute, variable, type Expr, type LibFn, type TableSource } from "../expr.ts";
import { emitShaderFn, type FnSpec } from "../compile/bind.ts";
import { classify, lookFns, tracedCall, withShading, type Shading } from "./painted.ts";
import { drawSpec, type Form, type CompileContext, type Drawable, type Emitted } from "../compile/context.ts";
import { emitMesh, fromNets } from "./mesh.ts";
import { pixelWorld } from "../lib/screen.ts";
import { lipschitz } from "../lib/sdf3.ts";
import { fieldShadow, shadowSkeleton } from "./field_shadow.ts";
import type { F, Field, P3, Painted, Skeleton } from "../map/types.ts";
import { aabbOf, outlineWgsl, PROXY_VERTICES, TRACE_DEPTH, TRACE_DEPTH_OUT, traceWgsl, type TraceBound, type TraceOutline } from "./trace.ts";
import { bakeBrick, varianceOf, type Brick, type Variance } from "../map/brick.ts";
import { surfaceNets, type TriangleMesh } from "../map/nets.ts";
import { SceneError } from "../errors.ts";

/**
 * The world size of one pixel at the point the field is evaluated at. A field may read it to choose its level of detail
 * (fold depth). Bound per step on the GPU; 0 (full detail) on the CPU.
 */
export const fieldPixel: F = variable("loft_field_px", "f32");

/**
 * Where the field is evaluated (DESIGN.md 7.7). `trace`: the formula at every step of every pixel. `brick`: the field is
 * sampled once into a distance brick (src/map/brick.ts) and the march steps by the brick, evaluating the formula only
 * within a few pixels of the surface, for the hit, the normal and the paint. `mesh`: a triangle mesh extracted once from
 * the brick (src/map/nets.ts), painted per pixel. `auto`: chosen from the field (see `plan`). Only a static field can be
 * bricked or meshed: one that reads time, an input, a param, a curve or a per-frame table is refused by name
 * (`FieldScheduleError`).
 */
export type FieldSchedule = "trace" | "brick" | "mesh" | "auto";

/** What `paint` sees at a hit: the world point and the unit outward normal. */
export type FieldHit = { readonly p: P3; readonly normal: P3 };

export type FieldOptions = {
  /** Where the field is marched: its proxy triangles and the ray's span. The solid must lie inside it. */
  readonly bounds: TraceBound;
  /** Base colour at a hit (lit by the style). To vary per copy, evaluate the fold at `hit.p` and use its `id`. */
  readonly paint: (hit: FieldHit) => P3 | Painted;
  /** Most samples of the field per pixel (steps by a brick's bound are not counted). */
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

/** Who checked the field: a named `draw.field`, or a `march` (which has no name; the error's line points at it). */
export type FieldChecker = "draw.field" | "march";

/**
 * Past this measured constant the field is almost surely not a distance at all. Friction F7 (2026-11-16): a degenerate
 * `sdRoundCone` (|r1 - r2| > |b - a|) measured far above 1, and the message offered `safety`, which would have marched
 * the broken field slowly instead of saying it was broken. A missing fold stretch is usually a small factor.
 */
const NOT_A_DISTANCE = 2;

const describe = (field: string, by: FieldChecker, r: FieldSafetyFailure): string => {
  const where = by === "march" ? "march" : `draw.field "${field}"`;
  switch (r.kind) {
    case "overshoots": return r.measured > NOT_A_DISTANCE
      ? `${where}: the field is not a distance: its measured Lipschitz constant is ${r.measured.toFixed(3)} (a distance changes by at most 1 per unit; a constant this large usually means the field is not one). Look for a degenerate primitive (sdRoundCone with |r1 - r2| >= |b - a|, a negative radius) or a field multiplied up and not divided back. Fix the field; safety would only hide it.`
      : `${where}: the field can overshoot: its measured Lipschitz constant is ${r.measured.toFixed(3)} (a distance must not change faster than 1 per unit). Fix the field (a fold's stretch factor missing?), or pass safety: ${r.suggested.toFixed(3)} to scale its steps.`;
    case "unevaluable": return `${where}: cannot check the field on the CPU: it calls the GPU-only function "${r.fn}". Pass an explicit safety factor to march it unchecked.`;
  }
};

/** A schedule that needs a static field was asked of one that is not. */
export class FieldScheduleError extends SceneError {
  override readonly name = "FieldScheduleError";
  constructor(readonly field: string, readonly schedule: FieldSchedule, readonly variance: Exclude<Variance, { kind: "static" }>) {
    super(`draw.field "${field}": schedule "${schedule}" samples the field once, but ${variance.kind === "changing" ? `it reads ${variance.reads.join(", ")}, which can change after compilation` : `it calls the GPU-only function "${variance.fn}", which the CPU cannot sample`}. Use schedule "trace".`);
  }
}

/**
 * Raised at compile time when a `draw.field` field cannot be certified for sphere tracing: its measured Lipschitz
 * constant is above 1 (steps could pass through the surface), or it calls a GPU-only function the CPU cannot sample.
 * Carries the field's name and `reason` (a `FieldSafetyFailure`, with the measured constant and a suggested `safety`).
 */
export class FieldSafetyError extends SceneError {
  override readonly name = "FieldSafetyError";
  constructor(readonly field: string, readonly reason: FieldSafetyFailure, readonly by: FieldChecker = "draw.field") {
    super(describe(field, by, reason));
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
export function checkField(name: string, field: Field, b: TraceBound, by: FieldChecker = "draw.field"): number {
  const gpuOnly = libsOf([field(P)]).find((l) => l.js === "gpu-only");
  if (gpuOnly) throw new FieldSafetyError(name, { kind: "unevaluable", fn: gpuOnly.name }, by);
  const measured = measureField(field, b);
  if (measured > LIPSCHITZ_LIMIT) throw new FieldSafetyError(name, { kind: "overshoots", measured, suggested: 0.95 / measured }, by);
  return measured;
}

/**
 * What a field's schedule resolves to, once its leaves are known: traced by its formula, traced through its brick,
 * drawn as its mesh, or both of the last two with the choice made per frame by its size on screen.
 */
type Plan =
  | { readonly kind: "trace" }
  | { readonly kind: "brick"; readonly sampled: Sampled }
  | { readonly kind: "mesh"; readonly mesh: TriangleMesh }
  | { readonly kind: "switch"; readonly sampled: Sampled; readonly mesh: TriangleMesh };

/**
 * A field sampled once: its brick, and the members of its union (the arguments of the `min`s at its root, at the
 * GPU's level of detail): one member, or several culled per coarse cell by the mask the brick stores as its tag.
 */
type Sampled = { readonly brick: Brick; readonly union: readonly Expr<"f32">[] };

/** Most union members culled by a mask (bits of an integer held exactly by an f32). */
const UNION_MAX = 24;

/**
 * Expression nodes one evaluation of a field runs: each node once, a loop's body once per iteration, its count taken at
 * full detail at the bound's centre. A count that reads anything else (an enclosing loop's state) counts as one
 * iteration, which only makes the estimate low (so `auto` keeps tracing). A brick lookup is a few scattered loads; on
 * the machine this was measured on (GPU timestamps, 2026-11-11) those cost more than the nest's formula (215 nodes:
 * traced 1.7 ms, bricked 3.5 ms at its overview) and far less than the coral's (2612 nodes: 7.3 -> 4.3 ms).
 */
export function evaluationCost(e: Expr<"f32">, centre: readonly [number, number, number]): number {
  const seen = new Set<number>();
  const countOf = (c: Expr<"f32">): number => {
    const at0 = substitute([c], fieldPixel, f(0))[0] as Expr<"f32">;
    if ([...leaves([at0]).vars.keys()].some((v) => v !== "p")) return 1;
    return Math.max(0, Math.trunc(compileJs([at0])({ vars: { p: [...centre] }, inputs: {}, params: {} })[0] as number));
  };
  const visit = (list: readonly Expr[], times: number): number => {
    let n = 0;
    for (const x of list) {
      if (seen.has(x.id)) continue;
      seen.add(x.id);
      const nd = x.node;
      n += times;
      if (nd.op === "iterate" && !seen.has(-1 - nd.loop.id)) {
        seen.add(-1 - nd.loop.id);
        n += visit(nd.loop.inner, times * countOf(nd.loop.count));
      }
    }
    return n;
  };
  return visit(topo([e]), 1);
}
/** Nodes per evaluation above which `auto` bricks a static field (between the nest's and the coral's: see above). */
export const BRICK_WORTH = 1000;

/** Whether a field's shape depends on the pixel size (its level of detail). */
const readsPixel = (e: Expr<"f32">): boolean => leaves([e]).vars.has("loft_field_px");

/** The arguments of the `min`s at the root of an expression (itself when its root is not a `min`). */
function unionOf(e: Expr<"f32">): Expr<"f32">[] {
  const n = e.node;
  return n.op === "call" && n.fn === "min" && n.ty === "f32" ? n.args.flatMap((a) => unionOf(a as Expr<"f32">)) : [e];
}

/**
 * The `Drawable` that `draw.field` returns: a signed distance field sphere-traced per pixel inside its bounding box. A
 * scene author gets it from `draw.field`, not by constructing it.
 */
export class FieldDrawable implements Drawable {
  private shadow: Skeleton[] | undefined;
  private planned: Plan | undefined;
  /** A soft blob fitted to the solid (src/draw/field_shadow.ts), found on first use: sampling costs a few ms. */
  get skeletons(): readonly Skeleton[] {
    this.shadow ??= [shadowSkeleton(fieldShadow(this.field, this.o.bounds, fieldPixel))];
    return this.shadow;
  }
  /** What it is for other readers: the field, whatever its schedule (a baker meshes it by surface nets, `fieldMesh`). */
  get forms(): readonly Form[] {
    return [{ kind: "field", name: this.name, field: this.field, bound: this.o.bounds, paint: this.o.paint }];
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
    const sampled = (): Sampled & { mesh: () => TriangleMesh } => {
      if (v.kind !== "static") throw new FieldScheduleError(this.name, this.o.schedule, v);
      const safety = this.stepScale();
      return sampleField(this.name, this.field, this.o.bounds, safety);
    };
    const made = ((): Plan => {
      switch (this.o.schedule) {
        case "trace": return { kind: "trace" };
        case "brick": return { kind: "brick", sampled: sampled() };
        case "mesh": return { kind: "mesh", mesh: sampled().mesh() };
        // A field that changes (or that the CPU cannot sample) is traced. A static one is bricked; one whose shape does
        // not follow the pixel size also gets a mesh, drawn instead whenever its cells are small on screen. A field that
        // reads `fieldPixel` is drawn at the pixel's level of detail at every distance, which no one mesh can show
        // (the coral's twigs are a few pixels wide at every distance: as a mesh they were blobs even at 512 cells).
        // A static field cheaper than BRICK_WORTH is traced too: its formula costs less than the brick's lookups.
        case "auto": {
          if (v.kind !== "static") return { kind: "trace" };
          const box = aabbOf(this.o.bounds);
          if (evaluationCost(this.field(P), [0, 1, 2].map((k) => (box.min[k]! + box.max[k]!) / 2) as unknown as [number, number, number]) < BRICK_WORTH) return { kind: "trace" };
          const b = sampled();
          return readsPixel(this.field(P)) ? { kind: "brick", sampled: b } : { kind: "switch", sampled: b, mesh: b.mesh() };
        }
      }
    })();
    this.planned = made;
    return made;
  }

  /** The field's sphere trace as a hit role (draw/march.ts, the same steps and step scale), painted by the style. */
  record(ctx: RecordContext): Recorded {
    const trace = march(this.field, { within: this.o.bounds, steps: this.o.steps, safety: this.stepScale() });
    // The paint sees `fieldPixel` as the pixel's world size at the hit, as the TS fragment binds it (coral, island_tide
    // paint by level of detail); unsubstituted it reached emit as a free variable.
    return rayDraw(ctx, this.name, { kind: "box", box: aabbOf(this.o.bounds) }, (ray) => trace(ray), (p, normal) => atPixel(this.o.paint({ p, normal }), pixelWorld(p)));
  }
  draws(ctx: CompileContext): Emitted {
    const safety = this.stepScale();
    const p = this.plan();
    const args = [["p", "vec3"], ["loft_field_px", "f32"]] as const;
    const d = this.field(P);
    /** `fn field_d(p, fp) -> f32`: the field, or the min over the members of its union that p's coarse cell keeps. */
    const fieldOf = (union: readonly Expr<"f32">[]): { code: string; libs: readonly LibFn[] } => {
      if (union.length === 1) return emitShaderFn(ctx.slots, { name: "field_d", args, out: "f32", roots: [union[0]!], ret: (v) => v[0] });
      const members = union.map((m, k) => emitShaderFn(ctx.slots, { name: `field_u${k}`, args, out: "f32", roots: [m], ret: (v) => v[0] }));
      const code = [...members.map((m) => m.code), /* wgsl */ `
// The union's members that can be nearest in p's coarse cell (the brick's tag); the others are skipped, not evaluated.
fn field_d(p: vec3<f32>, fp: f32) -> f32 {
  let keep = u32(brick_lower_tag(p));
  var d = 1e30;
${union.map((_, k) => `  if ((keep & ${1 << k}u) != 0u) { d = min(d, field_u${k}(p, fp)); }`).join("\n")}
  return d;
}`].join("\n");
      return { code, libs: [...new Set(members.flatMap((m) => m.libs))] };
    };
    const field = p.kind === "brick" || p.kind === "switch" ? fieldOf(p.sampled.union) : fieldOf([d]);
    const said = classify(this.name, this.o.paint({ p: P, normal: N as Expr<"vec3"> }));
    const paint = emitShaderFn(ctx.slots, { name: "field_paint", args: FIELD_PAINT_ARGS, out: "vec3<f32>", roots: [said.base], ret: (v) => v[0] });
    const id = `const FIELD_ID: f32 = ${lit(nameId(this.name))};`;
    // The proxy and its march: the field's formula at every step, or steps by its brick.
    const traced = (skip: string, show: string): Emitted => {
      const key = `field.${this.name}`;
      const box = aabbOf(this.o.bounds);
      const shading = lookFns(ctx, this.name, said, FIELD_PAINT_ARGS, {
        kind: "field", pixel: fieldPixel,
        scale: Math.hypot(...box.max.map((hi, k) => hi - box.min[k]!)),
        // Lighting taps can leave the proxy, where the brick's clamped union mask is no longer a bound.
        distance: (at) => substitute([d], P, at)[0] as F,
      });
      const painted = withShading(paint, shading);
      const code = [traceWgsl(this.o.bounds, show), outlineWgsl(this.o.outline), id, field.code, NORMAL_WGSL, painted.code, skip, fragmentWgsl(safety, shading, (mark) => ctx.markOut(mark, false))].join("\n");
      const libs = [...new Set([...field.libs, ...painted.libs])];
      return {
        shaders: { [key]: { code, libs, depthEntry: depthFragmentWgsl(safety) } },
        draws: [drawSpec({ label: this.name, shader: key, vertices: PROXY_VERTICES, cull: "front", constants: { STEPS: this.o.steps }, depth: TRACE_DEPTH })],
      };
    };
    /** `show`: a WGSL bool per vertex; `gate`: the code it calls. */
    // A composition: the mesh drawn by draw.mesh's own emitter, at rest, painted by the field's paint with the pixel
    // size it would have had (`fieldPixel` is the world size of a pixel at the point).
    const meshed = (mesh: TriangleMesh, show: string, gate: string): Emitted => emitMesh(ctx, this.name, fromNets(mesh), { kind: "rest" }, { kind: "one" }, {
      paint: (_row, at) => atPixel(this.o.paint({ p: at.world, normal: at.normal }), pixelWorld(at.world)),
      sides: "one", outline: this.o.outline === "silhouette" ? "hull" : "none",
    }, { key: `field.${this.name}.mesh`, show, gate });
    const both = (a: Emitted, b: Emitted): Emitted => ({ shaders: { ...a.shaders, ...b.shaders }, draws: [...a.draws, ...b.draws] });
    switch (p.kind) {
      case "trace": return traced(TRACE_SKIP, "true");
      case "brick": return traced(brickSkip(ctx, p.sampled.brick), "true");
      case "mesh": return meshed(p.mesh, "true", "");
      case "switch": {
        const choose = useMeshWgsl(this.o.bounds, p.sampled.brick.cell);
        return both(traced(`${choose}\n${brickSkip(ctx, p.sampled.brick)}`, "!field_use_mesh()"), meshed(p.mesh, "field_use_mesh()", choose));
      }
    }
  }
}

/**
 * The unit normal at a hit: the field's gradient by differences on a tetrahedron a quarter of a pixel across (Quilez),
 * four evaluations of the field's own code. It was the symbolic gradient (grad.ts), which differentiates through a
 * fold's loop: on the coral it cost about as much as the whole march, and its registers slowed every other part of the
 * shader (measured with GPU timestamps, 2026-11-11: brick schedule 10.1 -> 7.7 ms at the overview).
 */
const NORMAL_WGSL = /* wgsl */ `
fn field_n(p: vec3<f32>, fp: f32) -> vec3<f32> {
  let e = max(fp * 0.25, 1e-6);
  let k = vec2<f32>(1.0, -1.0);
  let g = k.xyy * field_d(p + k.xyy * e, fp) + k.yyx * field_d(p + k.yyx * e, fp) + k.yxy * field_d(p + k.yxy * e, fp) + k.xxx * field_d(p + k.xxx * e, fp);
  return g / max(length(g), 1e-20);
}`;

/**
 * The union mask of a coarse cell (centre c, half diagonal r): bit k is set unless member k provably cannot be the
 * nearest anywhere in the cell, at any level of detail. Each member scaled by the step scale is 1-Lipschitz, so in the
 * cell member k is at least u_k(c) - r at full detail and the union at most min_j u_j(c) + r at the coarsest detail
 * (the pixel size the bound's diagonal); a member whose lower bound exceeds that is never the min. This assumes, as the
 * level of detail already does (header), that a coarser level only removes parts: a distance between full detail's
 * and the coarsest's. A cell keeps at least the nearest member at its centre.
 */
function unionMask(union: readonly Expr<"f32">[], safety: number, diag: number): (c: readonly [number, number, number], r: number) => number {
  if (union.length === 1) return () => 1;
  const at = (px: number) => union.map((m) => mul(substitute([m], fieldPixel, f(px))[0] as Expr<"f32">, safety));
  const run = compileJs([...at(0), ...at(diag)]);
  const vars = { p: [0, 0, 0] };
  const env = { vars, inputs: {}, params: {} };
  const n = union.length;
  return (c, r) => {
    vars.p = [...c];
    const v = run(env) as number[];
    const coarsest = Math.min(...v.slice(n));
    let mask = 0;
    // A relative margin for rounding: dropping a member wrongly would cut a hole; keeping one wrongly costs time.
    for (let k = 0; k < n; k++) if (v[k]! - r <= coarsest + r + 1e-5 * (1 + Math.abs(coarsest))) mask |= 1 << k;
    return mask;
  };
}

/**
 * A static field sampled once into its brick (scaled by `safety`, so the brick's cones are those of a 1-Lipschitz
 * function), the members of its union, and its surface-nets mesh on demand. Throws `FieldScheduleError` for a field
 * that can change or that the CPU cannot sample.
 */
function sampleField(name: string, field: Field, b: TraceBound, safety: number): Sampled & { readonly mesh: () => TriangleMesh } {
  const full = substitute([field(P)], fieldPixel, f(0))[0] as Expr<"f32">;
  const v = varianceOf(full);
  if (v.kind !== "static") throw new FieldScheduleError(name, "mesh", v);
  const box = aabbOf(b);
  const diag = Math.hypot(box.max[0] - box.min[0], box.max[1] - box.min[1], box.max[2] - box.min[2]);
  const cell = diag / BRICK_CELLS;
  const d = mul(full, safety);
  const members = unionOf(field(P));
  const union = members.length >= 2 && members.length <= UNION_MAX ? members : [field(P)];
  const brick = bakeBrick(`${name}_brick`, d, "p", { tag: unionMask(union, safety, diag), box, cell, band: BRICK_BAND * cell });
  const run = compileJs([d]);
  const vars = { p: [0, 0, 0] };
  const env = { vars, inputs: {}, params: {} };
  const cpu = (x: number, y: number, z: number): number => { vars.p = [x, y, z]; return run(env)[0] as number; };
  return { brick, union, mesh: () => surfaceNets(brick, cpu) };
}

/**
 * A field form's triangles for a reader that wants a mesh (the baker): its surface nets at full detail, as the `mesh`
 * schedule draws them (steps unscaled: meshing does not march). Throws `FieldScheduleError` for a field that changes.
 */
export function fieldMesh(name: string, field: Field, b: TraceBound): TriangleMesh {
  return sampleField(name, field, b, 1).mesh();
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

const FIELD_PAINT_ARGS: FnSpec["args"] = [["p", "vec3"], ["n", "vec3"], ["loft_field_px", "f32"]];

/**
 * A field paint's answer at a pixel size: `fieldPixel` replaced by `px` in its colour, its channels and its own answers
 * (the mesh path has no per-pixel march to supply it).
 */
function atPixel(x: P3 | Painted, px: F): P3 | Painted {
  const at = <E extends Expr>(e: E): E => substitute([e], fieldPixel, px)[0] as E;
  const c = classify("field", x);
  if (c.said.channels.size === 0 && c.said.scatter === undefined && c.answer.kind === "styled") return at(c.base);
  const scatter = c.said.scatter;
  const wrap = c.answer.kind === "wrapped" ? c.answer.wrap : undefined;
  return {
    ...Object.fromEntries([...c.said.channels].map(([n, e]) => [n, at(e)])),
    base: at(c.base),
    ...(scatter === undefined ? {} : { scatter: (ctx, toward) => at(scatter(ctx, toward)) }),
    ...(wrap === undefined ? {} : { shade: (ctx, styled) => at(wrap(ctx, styled)) }),
  } as Painted;
}

const num = (x: number): string => (Number.isInteger(x) ? `${x}.0` : String(x));

function fragmentWgsl(safety: number, s: Shading, markOut: (mark: string) => string): string {
  const call = (fn: "shade" | "ink" | "mark") => tracedCall(s, fn, "base", "n", "p", "d", "FIELD_ID", "p, n, fp");
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
      // The last step's new distance would be read by no one.
      if (r + 1u < ${REFINE}u) { db = field_d(o + d * t, fp0 + fpk * (t - t0)); }
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
  let colour = select(${call("shade")}, ${call("ink")}, useInk);
  let mask = trace_mask(cover, in.clip.xy);
  if (mask == 0u) { discard; }
  return TraceOut(vec4<f32>(colour, ${markOut(call("mark"))}), trace_depth(p), mask);
}`;
}

/**
 * The depth-only entry (trace.ts `TRACE_DEPTH`): `fs`'s march with the ink and the shading taken out. A ray that hits
 * (or runs out of steps inside the bound, as `fs` takes it) writes its secant-refined depth; any other ray is discarded.
 */
function depthFragmentWgsl(safety: number): string {
  return /* wgsl */ `
${TRACE_DEPTH_OUT}
@diagnostic(off, derivative_uniformity)
@fragment fn fs_depth(in: TraceIn) -> TraceDepthOut {
  let d = view_dir(in.world);
  let o = trace_origin(in.world, d);
  let span = bound_span(o, d);
  let t0 = max(span.x, 0.0);
  let t1 = span.y;
  if (t0 > t1) { discard; }
  let fp0 = pixel_world(o + d * t0);
  let fpk = (pixel_world(o + d * max(t1, t0 + 1e-3)) - fp0) / max(t1 - t0, 1e-3);
  var t = t0;
  var hit = false;
  var exhausted = true;
  var prevDist = 0.0;
  var prevT = t0;
  var prevExact = false;
  var hitDist = 0.0;
  for (var i = 0u; i < STEPS; i++) {
    let skip = march_skip(o, d, t, t1, fp0, fpk, t0);
    if (skip.y > 0.0) { t = skip.x; prevExact = false; if (t > t1) { exhausted = false; break; } }
    let dist = field_d(o + d * t, fp0 + fpk * (t - t0));
    if (dist < HIT * (fp0 + fpk * (t - t0))) { hit = true; hitDist = dist; exhausted = false; break; }
    prevDist = dist;
    prevT = t;
    prevExact = true;
    t += dist * SAFETY;
    if (t > t1) { exhausted = false; break; }
  }
  if (!(hit || exhausted)) { discard; }
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
    if (r + 1u < ${REFINE}u) { db = field_d(o + d * t, fp0 + fpk * (t - t0)); }
    pairExact = true;
  }
  return TraceDepthOut(trace_depth(o + d * t));
}`;
}

/** A distance field, sphere-traced inside its bound. */
export function drawField(name: string, field: Field, o: FieldOptions): FieldDrawable {
  return new FieldDrawable(name, field, o);
}
