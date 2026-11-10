// The compiler's extension interface (DESIGN.md section 4).
//
// A new kind of thing is one module whose object implements `Drawable`: in `draws(ctx)` it emits its shaders (from
// expressions, with `emitShaderFn`, or as raw WGSL) and its draws. Per-frame tables are discovered from the expressions
// a drawable emits (`fetch`), or declared with `ctx.table` when the drawable writes raw WGSL that reads one. The
// compiler owns bindings, table packing, the pose function and draw order; nothing in src/compile changes for a new kind.

import type { Draw } from "../artefact.ts";
import type { LibFn, TableSource } from "../expr.ts";
import type { Sea } from "../geom/sea.ts";
import type { Grid } from "../draw/surface.ts";
import type { P3, Paint, Skeleton, Surface } from "../map/types.ts";
import type { TriangleMesh } from "../map/nets.ts";
import type { V3 } from "../map/transport.ts";
export type { V3 };
import type { Style } from "../style.ts";
import type { Slots } from "./bind.ts";

/** What `ctx.table` hands back: `wgsl` is the accessor to call in raw WGSL, `fn <wgsl>(i: u32) -> vec4<f32>` (i counts vec4s). */
export type TableRef = { readonly name: string; readonly wgsl: string };

/**
 * When a draw runs. Opaque phases run first, then transparent ones; within a phase, in the order drawables emit.
 * background: behind everything, no depth (the sky). opaque: solid geometry. water: the sea surface. on-water: layers
 * laid on the water after it (shadows, markers, wakes). transparent: soft things over everything (clouds, glows).
 */
export type Phase = "background" | "opaque" | "water" | "on-water" | "transparent";
export const PHASES: readonly Phase[] = ["background", "opaque", "water", "on-water", "transparent"];
export const PASS_OF: Readonly<Record<Phase, Draw["pass"]>> = {
  background: "opaque", opaque: "opaque", water: "transparent", "on-water": "transparent", transparent: "transparent",
};

/** A draw as a drawable states it: the artefact's `Draw`, with a phase in place of the host's pass. */
export type DrawSpec = Omit<Draw, "pass"> & { readonly phase: Phase };

/**
 * A shader module: its code (entry points `vs` and `fs`) and the library functions that code calls. The compiler puts
 * each library function either in `sceneWgsl` (when the shared functions call it too) or in the module, never both, so
 * a function that needs scene depth stays out of shaders that cannot read it.
 */
export type Shader = { readonly code: string; readonly libs: readonly LibFn[] };

export type Emitted = { readonly shaders: Readonly<Record<string, Shader>>; readonly draws: readonly DrawSpec[] };

/** A full-screen pass after the scene: `wgsl` defines `@fragment fn fs(in: PostIn) -> @location(0) vec4<f32>`. */
export type PostSpec = { readonly label: string; readonly wgsl: string };

export interface CompileContext {
  /** Where inputs, params, curve rows and tables live: emit expressions with `emitShaderFn(ctx.slots, ...)`. */
  readonly slots: Slots;
  readonly style: Style;
  readonly sea: Sea;
  /** Unit direction toward the sun. */
  readonly sun: V3;
  /** Every thing in the scene (for drawables derived from others, such as shadows). */
  readonly things: readonly Drawable[];
  /** Declare a per-frame table that raw WGSL reads (tables that expressions `fetch` are found without this). */
  table(t: TableSource): TableRef;
  /** Add a full-screen pass after the scene (after the style's own). */
  post(pass: PostSpec): void;
}

/** A surface a drawable draws, as the baker needs it: the map, its paint, the grid it is sampled on, its sides. */
export type SurfaceBakeable = { readonly name: string; readonly surface: Surface; readonly paint: Paint; readonly grid: Grid; readonly sides: "one" | "two" };
/** A closed triangle mesh a drawable draws (a field's, src/map/nets.ts), with the paint of a point and its normal. */
export type MeshBakeable = { readonly name: string; readonly mesh: TriangleMesh; readonly paint: (hit: { readonly p: P3; readonly normal: P3 }) => P3 };
/**
 * What the baker can evaluate. The two are told apart by their `mesh` member (bake/mesh.ts), so the surface drawables
 * that build the first kind did not change when the second was added.
 */
export type Bakeable = SurfaceBakeable | MeshBakeable;

export interface Drawable {
  readonly name: string;
  draws(ctx: CompileContext): Emitted;
  /** The chains this thing casts shadows with (empty for none). */
  readonly skeletons: readonly Skeleton[];
  /** The surfaces this thing draws, for baking (empty when it draws none that can be evaluated on the CPU). */
  readonly surfaces: readonly Bakeable[];
}

/** A draw with the defaults most draws share; each field still has to be meant. */
export function drawSpec(d: Pick<DrawSpec, "label" | "shader" | "vertices"> & Partial<DrawSpec>): DrawSpec {
  return {
    instances: 1, phase: "opaque", cull: "back", depthWrite: true, blend: "none", layer: "scene", constants: {},
    ...d,
  };
}
