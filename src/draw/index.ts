// One generic drawable per signature (DESIGN.md 7.4). `draw.surface`, `draw.curve` and `draw.points` are implemented;
// `draw.relief` is built (relief.ts); `draw.field` and `draw.density` are specified here (their options and what they draw) and belong to
// the tracers of docs/folding_and_tracing.md.

import { add, clamp, cross, div, length, lift, max, mul, sub, type In } from "../expr.ts";
import { viewDir } from "../lib/wgsl.ts";
import type { Drawable } from "../compile/context.ts";
import { NO_SKELETON, type Curve, type Density, type F, type Field, type P3, type Paint, type Points, type Surface } from "../map/types.ts";
import { drawFlat, drawGlazed, drawSurface, spacing, type Grid } from "./surface.ts";
import { drawField } from "./field.ts";
import { drawRelief } from "./relief.ts";

/** A drawable that can be drawn once per point of a `Points`. */
export interface Instanceable {
  instanced(name: string, points: Points): Drawable;
}

/** A signature whose drawable is specified but not built yet. */
export class NotImplementedError extends Error {
  override readonly name = "NotImplementedError";
}

/** An axis-aligned box: the proxy a tracer starts and ends its march in. */
export type Bounds = { readonly min: readonly [number, number, number]; readonly max: readonly [number, number, number] };

export type { FieldOptions } from "./field.ts";
export type { ReliefOptions } from "./relief.ts";
export type DensityOptions = { readonly bounds: Bounds; readonly colour: In<"vec3">; readonly steps: number };

export type CurveOptions = {
  /** Half-width of the stroke at u, world units. */
  readonly width: (u: F) => In<"f32">;
  /** Colour along the stroke: (u, v) with v across it (0 one side, 1 the other). Unlit. */
  readonly paint: Paint;
  /** Segments along the curve. */
  readonly segments: number;
};

/** Parameter step for a curve's tangent. */
const DU = 1e-3;

/** A ribbon along the curve facing the camera: the stroke's surface (it reads the camera, so it is shader-only). */
function ribbon(curve: Curve, width: (u: F) => In<"f32">): Surface {
  const side = (u: F): P3 => {
    const t = sub(curve(clamp(add(u, DU), 0, 1)), curve(clamp(sub(u, DU), 0, 1)));
    const c = cross(t, viewDir(curve(u)));
    return div(c, max(length(c), 1e-12));
  };
  const point = (u: F, v: F): P3 => add(curve(u), mul(side(u), mul(sub(mul(v, 2), 1), lift<"f32">(width(u)))));
  return { point, normal: (u) => mul(viewDir(curve(u)), -1), topology: { u: ["free", "free"], v: "free" }, skeleton: NO_SKELETON };
}

const notYet = (what: string) => (): never => { throw new NotImplementedError(`${what} is specified (src/draw/index.ts) but not implemented: see docs/folding_and_tracing.md section 3`); };

export const draw = {
  /** A surface, lit by the scene's style: a grid in the vertex stage, paint per pixel, an outline as asked. */
  surface: drawSurface,
  /** A surface with its own colour and alpha (the sea, decals). */
  glazed: drawGlazed,
  /** A stroke: a ribbon of half-width `width(u)` along the curve, facing the camera, coloured by `paint` (unlit). */
  curve(name: string, curve: Curve, o: CurveOptions): Drawable {
    const grid: Grid = { u: spacing.even(o.segments), v: spacing.even(1) };
    return drawFlat(name, ribbon(curve, o.width), { paint: o.paint, grid });
  },
  /** One copy of `thing` per point, each in its point's frame and scale. */
  points(name: string, points: Points, thing: Instanceable): Drawable {
    return thing.instanced(name, points);
  },
  /** A distance field, sphere-traced inside its bounds (folded and recursive solids). */
  field: drawField,
  /** Relief shell-traced on a surface, in its own coordinates (scales, spikes, barnacles). */
  relief: drawRelief,
  /** A density integrated along the view ray (cloud, fog). */
  density: notYet("draw.density") as (name: string, density: Density, o: DensityOptions) => Drawable,
};

