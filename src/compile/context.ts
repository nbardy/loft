// The compiler's extension interface (DESIGN.md section 4 and "Writing a new kind of thing").
//
// A new kind of thing is one module whose object implements `Drawable`: in `draws(ctx)` it asks the context for what
// it needs (per-frame tables, a strand's stations at some time, a strand's chart functions, shared WGSL, post passes)
// and returns its shaders and draws. The compiler owns bindings, table packing, the pose function and draw order;
// nothing in src/compile needs to change for a new kind.

import type { Draw } from "../artefact.ts";
import type { Sea } from "../chart.ts";
import type { Env, Expr } from "../expr.ts";
import type { V3 } from "../geom/stations.ts";
import type { Strand } from "../geom/strand.ts";
import type { Style } from "../style.ts";
import type { Slots } from "./bind.ts";

// ---- per-frame tables ----------------------------------------------------------------------------------------------

/** A per-frame table, computed on the CPU once per frame and read by shaders as `array<vec4<f32>>` entries. */
export type TableSpec = {
  /** An identifier, unique in the artefact. */
  readonly name: string;
  /** Size in vec4s; fixed at compile time (size it for the worst case and put a count in a header entry). */
  readonly vec4s: number;
  /** Fill `out` (length `vec4s * 4`, zeroed) for this frame. Must be pure: the same frame, the same numbers. */
  readonly fill: (frame: PoseFrame, out: Float32Array) => void;
};

/** What `ctx.table` hands back: `wgsl` is the accessor to call in any shader, `fn <wgsl>(i: u32) -> vec4<f32>`. */
export type TableRef = { readonly name: string; readonly wgsl: string };

/** Expressions the pose pass evaluates once per frame on the CPU (all requests share one compiled function). */
export type CpuRef = { readonly id: number; readonly count: number };

/** What a table's fill function sees: this frame's state, other tables, CPU-evaluated expressions, the sea. */
export interface PoseFrame {
  /** Inputs (including time), params and the curve table, as the JavaScript backend reads them. */
  readonly env: Env;
  /** Another table's contents this frame (filled on first read, so tables may depend on tables, without cycles). */
  table(t: TableRef): Float32Array;
  /** The values of expressions registered with `ctx.evaluate`, in order. */
  values(r: CpuRef): readonly (number | number[] | boolean)[];
  /** Sea surface height at (x, z) now. */
  seaHeight(x: number, z: number): number;
}

// ---- bodies, slices and charts -------------------------------------------------------------------------------------

/** Where one slice of a strand sits in the stations table, in stations (not floats). */
export type StationRange = { readonly table: TableRef; readonly base: number; readonly count: number };

/**
 * A point of a strand's chart at body coordinates (m, v, rho): the point, the skin's outward normal at (m, v), and an
 * orthonormal frame: `x` the spine tangent (toward increasing m), `y` the outward direction at v in the cross-section
 * (the in-plane normal of the section, so it never leans with a taper), `z = x cross y`. At v = 0.25 the frame is the
 * station frame (tangent, dorsal, side). Attachments and scattered instances ride this frame.
 */
export type BodyPoint = { readonly pos: V3; readonly normal: V3; readonly x: V3; readonly y: V3; readonly z: V3 };

/** A strand's chart at one time slice, callable from any shader and on the CPU. */
export interface ChartRef {
  readonly range: StationRange;
  /** WGSL `fn <wgsl>(m: f32, v: f32, rho: f32) -> BodyPoint`, defined in `sceneWgsl`. */
  readonly wgsl: string;
  /** The CPU twin over this frame's posed stations (for table fills, checks and tests). */
  cpu(frame: PoseFrame): (m: number, v: number, rho: number) => BodyPoint;
}

// ---- draws -------------------------------------------------------------------------------------------------------

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

export type Emitted = { readonly shaders: Readonly<Record<string, string>>; readonly draws: readonly DrawSpec[] };

/** A full-screen pass after the scene: `wgsl` defines `@fragment fn fs(in: PostIn) -> @location(0) vec4<f32>`. */
export type PostSpec = { readonly label: string; readonly wgsl: string };

// ---- the context -------------------------------------------------------------------------------------------------

export interface CompileContext {
  /** Where inputs, params and curve rows live: emit expressions with `emitFn(ctx.slots, ...)`. */
  readonly slots: Slots;
  readonly style: Style;
  readonly sea: Sea;
  /** Unit direction toward the sun. */
  readonly sun: V3;
  /** Every thing in the scene (for drawables derived from others, such as shadows). */
  readonly things: readonly Drawable[];
  /** Declare a per-frame table. */
  table(spec: TableSpec): TableRef;
  /** Evaluate expressions once per frame on the CPU; read the values in a fill with `frame.values(ref)`. */
  evaluate(roots: readonly Expr[]): CpuRef;
  /**
   * The strand's stations at time `at` (default: `time`), posed every frame. A strand asked for at several times gets
   * one range per time: a telegraph asks for `input("attack.arm", ...)`, a smear for `add(time, -0.2)`.
   */
  stations(strand: Strand, at?: Expr<"f32">): StationRange;
  /** The strand's chart at time `at` (default: `time`). */
  chart(strand: Strand, at?: Expr<"f32">): ChartRef;
  /** Module-scope WGSL shared by every shader (in `sceneWgsl`), once per key; the same key must carry the same code. */
  shared(key: string, wgsl: string): void;
  /** Add a full-screen pass after the scene (after the style's own). */
  post(pass: PostSpec): void;
}

export interface Drawable {
  readonly name: string;
  draws(ctx: CompileContext): Emitted;
}

/** Has a chain of capsules (a station range): bodies that cast shadows. */
export interface Skeleton {
  skeleton(ctx: CompileContext): StationRange;
}
export const isSkeleton = <T extends object>(t: T): t is T & Skeleton => typeof (t as Partial<Skeleton>).skeleton === "function";

/** A draw with the defaults most draws share; each field still has to be meant. */
export function draw(d: Pick<DrawSpec, "label" | "shader" | "vertices"> & Partial<DrawSpec>): DrawSpec {
  return {
    instances: 1, phase: "opaque", cull: "back", depthWrite: true, blend: "none", layer: "scene", constants: {},
    ...d,
  };
}
