// draw.traced: any solid the author can intersect, drawn per pixel inside a bound (docs/proposals/geometry_and_rays.md
// section 4.2). The author writes only `hit`; Loft supplies the bound's proxy, the view ray, depth, coverage, ink, the
// style's light and the CPU twin.

import { add, compileJs, f, leaves, libsOf, lift, mul, sub, substitute, variable, vec2, vec3, type Expr } from "../expr.ts";
import { emitShaderFn } from "../compile/bind.ts";
import { lit } from "../compile/scene_wgsl.ts";
import { drawSpec, type CompileContext, type Drawable, type Emitted, type Form, type Shader } from "../compile/context.ts";
import { SceneError } from "../errors.ts";
import type { F, P3, Skeleton } from "../map/types.ts";
import { fitBlob, shadowSkeleton } from "./field_shadow.ts";
import { nameId } from "./surface.ts";
import { aabbOf, outlineWgsl, PROXY_VERTICES, TRACE_DEPTH, TRACE_DEPTH_OUT, tracePlumbingWgsl, traceWgsl, type TraceBound, type TraceOutline } from "./trace.ts";

/**
 * A ray, or a thin cone: from `o` along unit `d`, searched for t in [t0, t1]. `width(t)` is how wide the ray is at t.
 * For a view ray it is the pixel's footprint; for a shadow ray, the light's apparent size times t; for a reflection,
 * how much it blurs.
 */
export type Ray = { readonly o: P3; readonly d: P3; readonly t0: F; readonly t1: F; readonly width: (t: F) => F };

/**
 * Where a ray meets a solid, or passes nearest it. `gap` is the solid's distance at t: 0 on a hit, positive on a miss,
 * at the t where gap / width(t) is least. `normal` is the solid's outward normal there.
 */
export type Traced = { readonly t: F; readonly gap: F; readonly normal: P3 };

/** Where a traced solid is searched: one static bound, or one box per copy (expressions: copies may move). */
export type TracedBound = TraceBound | { readonly kind: "copies"; readonly count: number; readonly box: (copy: F) => { readonly centre: P3; readonly half: P3 } };

/** What a traced solid's paint sees at a hit: the point, the normal, the ray's direction and the copy's index. */
export type TracedHit = { readonly p: P3; readonly normal: P3; readonly d: P3; readonly copy: F };
export type TracedPaint = (hit: TracedHit) => P3;

export type TracedOptions = {
  /**
   * Where the hit is searched: one static bound, or one box per copy (expressions: copies may move). A static bound
   * must hold the solid and its ink (leave the ink width round it, a few per cent): what lies outside is cut.
   */
  readonly bound: TracedBound;
  /** The intersection, as expressions: the ray spans the bound (or the copy's box). `copy` is 0 for one bound. */
  readonly hit: (ray: Ray, copy: F) => Traced;
  /** Base colour at a hit, lit by the style: the point, normal, ray direction and copy index. */
  readonly paint: TracedPaint;
  readonly outline: TraceOutline;
  /** Absent: a blob fitted to the hits of rays cast down through a static bound on the CPU, or none for copies. */
  readonly skeletons?: readonly Skeleton[];
};

/** A hit within this many pixels of the surface is full coverage without ink (as draw.field's march, field.ts). */
const HIT = 0.3;

// The view ray's variables, made on first use (a module-load expression would renumber every scene's WGSL lets).
let made: { O: Expr<"vec3">; D: Expr<"vec3">; T0: F; T1: F; FP0: F; FPK: F; COPY: F; T: F } | undefined;
const vars = () => made ??= {
  O: variable("tr_o", "vec3"), D: variable("tr_d", "vec3"), T0: variable("tr_t0", "f32"), T1: variable("tr_t1", "f32"),
  FP0: variable("tr_fp0", "f32"), FPK: variable("tr_fpk", "f32"), COPY: variable("tr_copy", "f32"), T: variable("tr_t", "f32"),
};

/** The view ray through a pixel as the fragment stage binds it: width is the pixel's world size, affine along the ray. */
function viewRay(): Ray {
  const { O, D, T0, T1, FP0, FPK } = vars();
  return { o: O, d: D, t0: T0, t1: T1, width: (t) => add(FP0, mul(FPK, sub(t, T0))) };
}

const where = (name: string) => `draw.traced "${name}"`;

/** A static bound, or boxes per copy, classified once. */
type Proxy = { readonly kind: "one"; readonly bound: TraceBound } | { readonly kind: "copies"; readonly count: number; readonly box: (copy: F) => { readonly centre: P3; readonly half: P3 } };
const proxyOf = (b: TracedBound): Proxy => (b.kind === "copies" ? { kind: "copies", count: b.count, box: b.box } : { kind: "one", bound: b });

/**
 * The `Drawable` that `draw.traced` returns: the author's own intersection, drawn per pixel in a bound with depth,
 * coverage, ink and the style's light. A scene author gets it from `draw.traced`, not by constructing it.
 */
export class TracedDrawable implements Drawable {
  readonly skeletons: readonly Skeleton[];
  readonly forms: readonly Form[];
  private readonly proxy: Proxy;

  constructor(readonly name: string, readonly o: TracedOptions) {
    if (!/^[A-Za-z][A-Za-z0-9_]*$/.test(name)) throw new SceneError(where(name), `the name "${name}"`, "an identifier (it names shaders)", "letters, digits and _ only");
    this.proxy = proxyOf(o.bound);
    if (this.proxy.kind === "copies" && !(Number.isInteger(this.proxy.count) && this.proxy.count >= 1)) throw new SceneError(where(name), `${this.proxy.count} copies`, "a whole count of at least 1", "count: copies.length");
    this.skeletons = o.skeletons ?? defaultShadow(name, o.hit, this.proxy);
    this.forms = [{ kind: "hit", name, count: this.proxy.kind === "copies" ? this.proxy.count : 1, bound: o.bound, hit: o.hit, paint: o.paint }];
  }

  draws(ctx: CompileContext): Emitted {
    const { D, COPY, T } = vars();
    const h = this.o.hit(viewRay(), COPY);
    const rayArgs = [["tr_o", "vec3"], ["tr_d", "vec3"], ["tr_t0", "f32"], ["tr_t1", "f32"], ["tr_fp0", "f32"], ["tr_fpk", "f32"], ["tr_copy", "f32"]] as const;
    const hitFn = emitShaderFn(ctx.slots, { name: "traced_hit", args: rayArgs, out: "vec2<f32>", roots: [vec2(h.t, h.gap)], ret: (v) => v[0]! });
    // The normal is asked only where a pixel is covered, at the t found: what it reads of the hit's t is that value, so
    // a march is not run twice (the rest of its expression is recomputed there).
    const normalFn = emitShaderFn(ctx.slots, { name: "traced_normal", args: [...rayArgs, ["tr_t", "f32"]], out: "vec3<f32>", roots: [substitute([h.normal], h.t, T)[0]!], ret: (v) => v[0]! });
    const P = variable("tr_p", "vec3"), N = variable("tr_n", "vec3");
    const paintFn = emitShaderFn(ctx.slots, {
      name: "traced_paint", args: [["tr_p", "vec3"], ["tr_n", "vec3"], ["tr_d", "vec3"], ["tr_copy", "f32"], ["loft_field_px", "f32"]], out: "vec3<f32>",
      roots: [this.o.paint({ p: P, normal: N, d: D, copy: COPY })], ret: (v) => v[0]!,
    });
    const key = `traced.${this.name}`;
    const head = this.proxy.kind === "one"
      ? { code: `${traceWgsl(this.proxy.bound, "true")}\nfn traced_span(in: TraceIn, o: vec3<f32>, d: vec3<f32>) -> vec2<f32> { return bound_span(o, d); }\nfn traced_copy(in: TraceIn) -> f32 { return 0.0; }`, libs: [] }
      : copiesProxy(ctx, this.proxy);
    const code = [head.code, outlineWgsl(this.o.outline), `const TRACED_ID: f32 = ${lit(nameId(this.name))};`, hitFn.code, normalFn.code, paintFn.code, fragmentWgsl(ctx.markOut("trace_mark(base, n, p, d, id)", false))].join("\n");
    const libs = [...new Set([...head.libs, ...hitFn.libs, ...normalFn.libs, ...paintFn.libs])];
    return {
      shaders: { [key]: { code, libs, depthEntry: DEPTH_ENTRY_WGSL } },
      draws: [drawSpec({ label: this.name, shader: key, vertices: PROXY_VERTICES, instances: this.proxy.kind === "copies" ? this.proxy.count : 1, cull: "front", depth: TRACE_DEPTH })],
    };
  }
}

/** The per-copy proxy: an axis-aligned box per instance from `box(copy)`, grown by the ink width and one pixel. */
function copiesProxy(ctx: CompileContext, p: Extract<Proxy, { kind: "copies" }>): Shader {
  const b = p.box(vars().COPY);
  const fn = emitShaderFn(ctx.slots, { name: "traced_box", args: [["tr_copy", "f32"]], out: "array<vec3<f32>, 2>", roots: [b.centre, b.half], ret: (v) => `array<vec3<f32>, 2>(${v.join(", ")})` });
  return { libs: fn.libs, code: `${tracePlumbingWgsl()}
${fn.code}
struct TraceIn { @builtin(position) clip: vec4<f32>, @location(0) world: vec3<f32>, @location(1) @interpolate(flat) lo: vec3<f32>, @location(2) @interpolate(flat) hi: vec3<f32>, @location(3) @interpolate(flat) copy: f32 };
@vertex fn vs(@builtin(vertex_index) vi: u32, @builtin(instance_index) ii: u32) -> TraceIn {
  let b = traced_box(f32(ii));
  let fp = max(pixel_world(b[0] - b[1]), pixel_world(b[0] + b[1]));
  let half = abs(b[1]) + vec3<f32>(select(0.0, ink_px(fp) * fp, INK_ON) + fp);
  let w = b[0] + LOFT_CUBE[vi] * half;
  return TraceIn(frame.view_proj * vec4<f32>(w, 1.0), w, b[0] - half, b[0] + half, f32(ii));
}
fn traced_span(in: TraceIn, o: vec3<f32>, d: vec3<f32>) -> vec2<f32> {
  let inv = 1.0 / d;
  let a = (in.lo - o) * inv;
  let c = (in.hi - o) * inv;
  let lo = min(a, c);
  let hi = max(a, c);
  return vec2<f32>(max(lo.x, max(lo.y, lo.z)), min(hi.x, min(hi.y, hi.z)));
}
fn traced_copy(in: TraceIn) -> f32 { return in.copy; }` };
}
/**
 * The fragment stage every traced solid shares: the view ray and its span, the author's hit, then coverage, ink and
 * light from { t, gap, normal } (one gap serves all three: coverage is 1 - gap / width over a pixel, ink a near miss).
 */
function fragmentWgsl(markOut: string): string {
  return /* wgsl */ `
const HIT: f32 = ${HIT};
// The author's hit may run a loop whose exit differs per pixel before the style's derivatives (see field.ts).
@diagnostic(off, derivative_uniformity)
@fragment fn fs(in: TraceIn) -> TraceOut {
  let d = view_dir(in.world);
  let o = trace_origin(in.world, d);
  let span = traced_span(in, o, d);
  let t0 = max(span.x, 0.0);
  let t1 = span.y;
  if (t0 > t1) { discard; }
  // Pixel size is affine along the ray (it is proportional to clip w): the view ray's width.
  let fp0 = pixel_world(o + d * t0);
  let fpk = (pixel_world(o + d * max(t1, t0 + 1e-3)) - fp0) / max(t1 - t0, 1e-3);
  let copy = traced_copy(in);
  let h = traced_hit(o, d, t0, t1, fp0, fpk, copy);
  let tq = h.x;
  let p = o + d * tq;
  let fp = fp0 + fpk * (tq - t0);
  let inkPx = ink_px(fp);
  let gapPx = h.y / fp;
  let hit = h.y <= 0.0;
  let edge = select(HIT, inkPx, INK_ON);
  let cover = select(clamp(1.0 + edge - gapPx, 0.0, 1.0), 1.0, hit || gapPx < edge);
  // The normal is asked only of covered pixels (most of a bound's pixels are clean misses). The paint is asked of
  // every pixel, in uniform control flow: a paint may take screen derivatives (a reflected ray's ground colour does),
  // which a branch would forbid. A miss is painted with its stand-in normal and then discarded.
  var n = -d;
  if (cover > 0.0) {
    n = traced_normal(o, d, t0, t1, fp0, fpk, copy, tq);
  }
  let base = traced_paint(p, n, d, copy, fp);
  let id = copy_id(TRACED_ID, u32(copy));
  let useInk = INK_ON && !hit;
  let colour = select(trace_shade(base, n, p, d, id), trace_ink(base, n, p, d, id), useInk);
  let mask = trace_mask(cover, in.clip.xy);
  if (mask == 0u) { discard; }
  return TraceOut(vec4<f32>(colour, ${markOut}), trace_depth(p), mask);
}`;
}

/**
 * The depth-only entry (trace.ts `TRACE_DEPTH`, for a depth pass): `fs` up to the author's hit, with no normal, paint,
 * ink or light. A ray that hits (gap 0 or less) writes its depth; a near miss covers nothing.
 */
const DEPTH_ENTRY_WGSL = /* wgsl */ `
${TRACE_DEPTH_OUT}
@diagnostic(off, derivative_uniformity)
@fragment fn fs_depth(in: TraceIn) -> TraceDepthOut {
  let d = view_dir(in.world);
  let o = trace_origin(in.world, d);
  let span = traced_span(in, o, d);
  let t0 = max(span.x, 0.0);
  let t1 = span.y;
  if (t0 > t1) { discard; }
  let fp0 = pixel_world(o + d * t0);
  let fpk = (pixel_world(o + d * max(t1, t0 + 1e-3)) - fp0) / max(t1 - t0, 1e-3);
  let h = traced_hit(o, d, t0, t1, fp0, fpk, traced_copy(in));
  if (h.y > 0.0) { discard; }
  return TraceDepthOut(trace_depth(o + d * h.x));
}`;

/** Rays cast down through a static bound per side of this grid, for the default shadow. */
const SHADOW_GRID = 24;

/** The default shadow: a blob fitted to where rays cast straight down through the bound hit, at time 0. */
function defaultShadow(name: string, hit: TracedOptions["hit"], p: Proxy): readonly Skeleton[] {
  switch (p.kind) {
    case "copies": return [];
    case "one": {
      const { min, max } = aabbOf(p.bound);
      const ray: Ray = { o: variable("sh_o", "vec3"), d: vec3(0, -1, 0), t0: f(0), t1: f(max[1] - min[1]), width: () => lift<"f32">(1e-3) };
      const h = hit(ray, f(0));
      const gpu = libsOf([h.t, h.gap]).find((l) => l.js === "gpu-only");
      if (gpu || leaves([h.t, h.gap]).tables.length > 0) throw new SceneError(where(name), `a hit the CPU cannot evaluate (${gpu ? `the GPU-only function "${gpu.name}"` : "a per-frame table"})`, "skeletons for the shadow", "skeletons: [] for none, or chains inside the solid");
      const run = compileJs([h.t, h.gap]);
      const pts: { p: [number, number, number]; w: number }[] = [];
      for (let i = 0; i < SHADOW_GRID; i++) for (let k = 0; k < SHADOW_GRID; k++) {
        const x = min[0] + ((i + 0.5) / SHADOW_GRID) * (max[0] - min[0]), z = min[2] + ((k + 0.5) / SHADOW_GRID) * (max[2] - min[2]);
        const [t, gap] = run({ vars: { sh_o: [x, max[1], z] }, inputs: { time: [0] }, params: {} }) as number[];
        if (gap! <= 0) pts.push({ p: [x, max[1] - t!, z], w: 1 });
      }
      if (pts.length < 2) return [];
      const cell = Math.max(max[0] - min[0], max[2] - min[2]) / SHADOW_GRID;
      return [shadowSkeleton({ kind: "sampled", inside: pts.length, chain: fitBlob(pts, cell) })];
    }
  }
}

/** Any solid you can intersect, drawn per pixel in a bound: `draw.traced(name, { bound, hit, paint, outline, skeletons? })`. */
export function drawTraced(name: string, o: TracedOptions): TracedDrawable {
  return new TracedDrawable(name, o);
}
