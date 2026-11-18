// The compiler's extension interface (DESIGN.md section 4).
//
// A new kind of thing is one module whose object implements `Drawable`: in `draws(ctx)` it emits its shaders (from
// expressions, with `emitShaderFn`, or as raw WGSL) and its draws. Per-frame tables are discovered from the expressions
// a drawable emits (`fetch`), or declared with `ctx.table` when the drawable writes raw WGSL that reads one. The
// compiler owns bindings, table packing, the pose function and draw order; nothing in src/compile changes for a new kind.

import type { Detail, Draw } from "../artefact.ts";
import type { Expr, LibFn, TableSource } from "../expr.ts";
import type { Ground } from "../draw/ground.ts";
import type { Grid } from "../draw/surface.ts";
import type { F, Field, P3, Paint, Painted, Poses, Skeleton, Surface } from "../map/types.ts";
import type { Mesh, MeshOptions, MeshPaint } from "../draw/mesh.ts";
import type { FieldHit } from "../draw/field.ts";
import type { TraceBound } from "../draw/trace.ts";
import type { Ray, Traced, TracedBound, TracedPaint } from "../draw/traced.ts";
import type { V3 } from "../map/transport.ts";
export type { V3 };
import type { Style } from "../style.ts";
import type { OutlineMethod } from "../scene.ts";
import type { Lighting, Sun } from "../light.ts";
import type { Slots } from "./bind.ts";

/** What `ctx.table` hands back: `wgsl` is the accessor to call in raw WGSL, `fn <wgsl>(i: u32) -> vec4<f32>` (i counts vec4s). */
export type TableRef = { readonly name: string; readonly wgsl: string };

/**
 * When a draw runs. Opaque phases run first, then transparent ones; within a phase, in the order drawables emit.
 * background: behind everything, no depth (the sky). opaque: solid geometry. on-bodies: what is laid on the opaque
 * scene before the water covers it (a style's shadow block, src/draw/light.ts). water: the ground (a sea, or a plane),
 * which writes no depth. on-water: layers laid on the ground after it (shadows, markers, wakes). transparent: soft things over everything (clouds, glows).
 */
export type Phase = "background" | "opaque" | "on-bodies" | "water" | "on-water" | "transparent";
export const PHASES: readonly Phase[] = ["background", "opaque", "on-bodies", "water", "on-water", "transparent"];
export const PASS_OF: Readonly<Record<Phase, Draw["pass"]>> = {
  background: "opaque", opaque: "opaque", "on-bodies": "transparent", water: "transparent", "on-water": "transparent", transparent: "transparent",
};

/**
 * A draw's level of detail as a drawable states it (the artefact's `Detail` before slots and table offsets exist):
 * a grid names the per-frame table of its bounds (two lanes per instance, declared through `ctx.table`); the compiler
 * gives each bound table, and each projected spec, its slots. Draws stating the same bound table share one choice.
 */
export type DetailSpec =
  | { readonly kind: "fixed" }
  | {
    readonly kind: "grid";
    /** One lane per instance: (anchor point, scale), per frame. */
    readonly bound: TableSource;
    /** Cell counts per level, finest first. */
    readonly counts: readonly (readonly [number, number])[];
    /** What the compiler measures at rest: 5 x 5 grid points (u fastest) and the anchor point (src/draw/grid_detail.ts). */
    readonly rest: { readonly samples: readonly Expr<"vec3">[]; readonly anchor: Expr<"vec3"> };
  }
  | Omit<Extract<Detail, { kind: "projected" }>, "slot">;

/** A draw as a drawable states it: the artefact's `Draw`, with a phase in place of the host's pass. */
export type DrawSpec = Omit<Draw, "pass" | "detail"> & { readonly phase: Phase; readonly detail: DetailSpec };

/**
 * A shader module: its code (entry points `vs` and `fs`) and the library functions that code calls. The compiler puts
 * each library function either in `sceneWgsl` (when the shared functions call it too) or in the module, never both, so
 * a function that needs scene depth stays out of shaders that cannot read it.
 */
export type Shader = {
  readonly code: string;
  /** Alternative body with the same shader interface. The host supplies `enable f16;`. */
  readonly mixedCode?: string;
  readonly libs: readonly LibFn[];
  /**
   * WGSL defining a depth-only fragment entry (`DrawDepth` "entry"), appended to the module only when the scene has a
   * depth pass, so a scene without one keeps its shaders byte for byte. Its draws say `depth: { kind: "entry", entry }`.
   */
  readonly depthEntry?: string;
};

export type Emitted = { readonly shaders: Readonly<Record<string, Shader>>; readonly draws: readonly DrawSpec[] };

/** A full-screen pass after the scene: `wgsl` defines `@fragment fn fs(in: PostIn) -> @location(0) vec4<f32>`. */
export type PostSpec = {
  readonly label: string;
  readonly wgsl: string;
  /** Alternative fragment with the same interface. The host supplies `enable f16;`. */
  readonly mixedWgsl?: string;
};

/**
 * What a `Drawable` is handed when it emits its shaders: the slots for inputs, params and tables, the scene's style,
 * ground and sun, and every other thing in the scene. For authors of their own drawable or host; a scene author does
 * not need it.
 */
export interface CompileContext {
  /** Where inputs, params, curve rows and tables live: emit expressions with `emitShaderFn(ctx.slots, ...)`. */
  readonly slots: Slots;
  readonly style: Style;
  /** What the scene stands on: a sea, a plane or nothing (src/draw/ground.ts dispatches on it). */
  readonly ground: Ground;
  /** Unit direction toward the sun as the scene states it (a movable sun's default). */
  readonly sun: V3;
  /** The sun as shaders read it: that literal, or a host input (src/light.ts). */
  readonly sunAt: Sun;
  /** The sun, its amount and the scene's lights: what a context's `lights` fold is built from (`foldAt`, src/light.ts). */
  readonly lighting: Lighting;
  /** Every thing in the scene (for drawables derived from others, such as shadows). */
  readonly things: readonly Drawable[];
  /** Declare a per-frame table that raw WGSL reads (tables that expressions `fetch` are found without this). */
  table(t: TableSource): TableRef;
  /** Add a full-screen pass after the scene (after the style's own). */
  post(pass: PostSpec): void;
  /** How ink outlines are drawn in this scene. */
  readonly outline: OutlineMethod;
  /**
   * The alpha an opaque fragment writes for `mark` (WGSL expression `mark`): the mark itself with hull outlines; with
   * screen outlines, the mark and whether this surface is outlined (src/draw/ink.ts), and asking for the ink pass.
   */
  markOut(mark: string, inked: boolean): string;
  /** Record a line for the artefact's `notes` (a channel no part of the style reads; a wrap that ignores the style). */
  note(line: string): void;
}

/**
 * What a thing is, for any reader other than this renderer (the baker, the checks, another renderer): one variant per
 * kind of geometry, every reader dispatching on `kind` exhaustively. A reader that meets a variant it does not handle
 * reports it by name, never falls back silently (the baker's `skipped` list). `hit` is the author's own: every other
 * kind can be lowered to it by a reader that wants to (docs/proposals/headroom.md A3 and section 9).
 *
 * - surface: a (u, v) map, its paint, the grid it is drawn on, its sides; `copy` is the copy's index (0 for one).
 * - mesh: triangles made by code (`draw.mesh`, or a field's surface nets): rows, moved by `at` (absent: at rest) and
 *   placed by `copies` (absent: one), painted over rows.
 * - field: a signed distance field inside its bound, painted at a hit.
 * - medium: a density inside a box and its colour (`draw.density`).
 * - hit: any solid the author can intersect (`draw.traced`): `count` copies, each searched in its bound.
 */
export type Form =
  | { readonly kind: "surface"; readonly name: string; readonly surface: Surface; readonly paint: Paint; readonly grid: Grid; readonly sides: "one" | "two"; readonly copy: number }
  | { readonly kind: "mesh"; readonly name: string; readonly mesh: Mesh; readonly at: MeshOptions["at"]; readonly copies: Poses | undefined; readonly paint: MeshPaint }
  | { readonly kind: "field"; readonly name: string; readonly field: Field; readonly bound: TraceBound; readonly paint: (hit: FieldHit) => P3 | Painted }
  | { readonly kind: "medium"; readonly name: string; readonly bound: TraceBound; readonly density: (p: P3) => F; readonly colour: (p: P3) => P3 }
  | { readonly kind: "hit"; readonly name: string; readonly count: number; readonly bound: TracedBound; readonly hit: (ray: Ray, copy: F) => Traced; readonly paint: TracedPaint };

/**
 * Anything a scene can draw: a name, the shaders and draw calls it emits for a `CompileContext`, the shadow-casting
 * chains it has, and what it is for other readers (`forms`). The `draw` members, `glow` and `flare` make Drawables. For
 * authors of their own drawable; a scene author builds Drawables with those and does not need this interface.
 */
export interface Drawable {
  readonly name: string;
  draws(ctx: CompileContext): Emitted;
  /** The chains this thing casts shadows with (empty for none). */
  readonly skeletons: readonly Skeleton[];
  /** What it is, for any other reader (the baker, the checks, another renderer). Empty: nothing a reader can use. */
  readonly forms: readonly Form[];
}

/**
 * Builds one `DrawSpec` (a draw call: label, shader, vertex count and its state) with the defaults most draws share.
 * For authors of their own drawable; a scene author does not need it.
 */
export function drawSpec(d: Pick<DrawSpec, "label" | "shader" | "vertices"> & Partial<DrawSpec>): DrawSpec {
  return {
    instances: 1, phase: "opaque", cull: "back", depthWrite: true, blend: "none", layer: "scene", constants: {}, detail: { kind: "fixed" },
    ...d,
  };
}
