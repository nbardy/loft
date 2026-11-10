// What a drawable sees while it compiles itself (DESIGN.md section 4: `Drawable` is the compiler's extension point).

import type { Draw } from "../artefact.ts";
import type { Sea } from "../chart.ts";
import type { Strand } from "../geom/strand.ts";
import type { Style } from "../style.ts";
import type { Slots } from "./bind.ts";

/** Where a strand's stations sit in the `stations` table, in stations (not floats). */
export type StationRange = { readonly base: number; readonly count: number };

export interface CompileContext {
  readonly slots: Slots;
  readonly style: Style;
  readonly sea: Sea;
  /** The strand's range in `stations`; the first call registers the strand with the pose pass. */
  stations(strand: Strand): StationRange;
}

export type Emitted = { readonly shaders: Readonly<Record<string, string>>; readonly draws: readonly Draw[] };

export interface Drawable {
  readonly name: string;
  draws(ctx: CompileContext): Emitted;
}

/** A draw with the defaults most draws share; each field still has to be meant. */
export function draw(d: Pick<Draw, "label" | "shader" | "vertices"> & Partial<Draw>): Draw {
  return {
    instances: 1, pass: "opaque", cull: "back", depthWrite: true, blend: "none", layer: "scene", constants: {},
    ...d,
  };
}
