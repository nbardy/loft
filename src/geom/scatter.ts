// Scatter (DESIGN.md 2.2): instanced copies of a small strand placed at body coordinates on a host. The thing is
// authored in an instance's local frame (x along the host's spine, y out of its skin, z = x cross y; see `BodyPoint`)
// and posed once; the vertex stage places copy i on the host's chart at `at(i)`, scaled by `scale(i)`. `at` and
// `scale` are expressions of the instance index (and may read time), evaluated per instance on the GPU.

import { lift, type Expr, type In } from "../expr.ts";
import type { CompileContext, Drawable, Emitted } from "../compile/context.ts";
import { scatterDraws } from "../draw/scatter.ts";
import type { Strand } from "./strand.ts";

export type ScatterSpec = {
  readonly name: string;
  readonly on: Strand;
  readonly count: number;
  /** Body coordinates of instance i (i = 0 .. count - 1, as a float). */
  readonly at: (i: Expr<"f32">) => { readonly m: In<"f32">; readonly v: In<"f32">; readonly rho: In<"f32"> };
  /** The copy, in the instance's local frame. Never drawn on its own unless the scene lists it too. */
  readonly thing: Strand;
  /** Uniform scale of instance i. Default 1. */
  readonly scale?: number | ((i: Expr<"f32">) => In<"f32">);
};

export type ScatterAt = { readonly m: Expr<"f32">; readonly v: Expr<"f32">; readonly rho: Expr<"f32">; readonly scale: Expr<"f32"> };

export class Scatter implements Drawable {
  constructor(
    readonly name: string,
    readonly on: Strand,
    readonly count: number,
    readonly thing: Strand,
    private readonly where: (i: Expr<"f32">) => ScatterAt,
  ) {}

  /** Placement of instance i, as expressions of i. */
  at(i: Expr<"f32">): ScatterAt { return this.where(i); }

  draws(ctx: CompileContext): Emitted { return scatterDraws(ctx, this); }
}

export function scatter(spec: ScatterSpec): Scatter {
  if (!/^[A-Za-z][A-Za-z0-9_]*$/.test(spec.name)) throw new Error(`scatter "${spec.name}": names must be identifiers (they name shaders)`);
  if (!(Number.isInteger(spec.count) && spec.count >= 1)) throw new Error(`scatter "${spec.name}": count must be a positive integer, got ${spec.count}`);
  if (spec.thing.placement.kind !== "world") throw new Error(`scatter "${spec.name}": the thing is placed by the scatter, so it must not be attached elsewhere`);
  const sc = spec.scale ?? 1;
  const scale = typeof sc === "number" ? () => lift<"f32">(sc) : (i: Expr<"f32">) => lift<"f32">(sc(i));
  return new Scatter(spec.name, spec.on, spec.count, spec.thing, (i) => {
    const a = spec.at(i);
    return { m: lift<"f32">(a.m), v: lift<"f32">(a.v), rho: lift<"f32">(a.rho), scale: scale(i) };
  });
}
