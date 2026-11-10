// A scene says what things are; `compile` decides where each part is evaluated.

import type { Camera } from "./artefact.ts";
import type { Sea } from "./chart.ts";
import type { Drawable } from "./compile/context.ts";
import { Crest } from "./geom/crest.ts";
import { Strand } from "./geom/strand.ts";
import { crestDraws, strandDraws } from "./compile/shaders.ts";
import type { Style } from "./style.ts";

export type SceneSpec = {
  readonly sea: Sea;
  /** Strands, crests, or any other `Drawable`. */
  readonly things: readonly (Strand | Crest | Drawable)[];
  readonly style: Style;
  /** Direction toward the sun (normalised here). */
  readonly sun: readonly [number, number, number];
  readonly cameras?: Readonly<Record<string, Camera>>;
};

export type Scene = {
  readonly kind: "scene";
  readonly sea: Sea;
  readonly things: readonly Drawable[];
  readonly style: Style;
  readonly sun: readonly [number, number, number];
  readonly cameras: Readonly<Record<string, Camera>>;
};

/** The built-in kinds as drawables. Anything else must already be one. */
function drawable(t: Strand | Crest | Drawable): Drawable {
  if (t instanceof Strand) return { name: t.name, draws: (ctx) => strandDraws(ctx, t) };
  if (t instanceof Crest) return { name: t.name, draws: (ctx) => crestDraws(ctx, t) };
  return t;
}

export function scene(spec: SceneSpec): Scene {
  const l = Math.hypot(...spec.sun);
  if (!(spec.sun[1] > 0.05 * l)) throw new Error(`scene: the sun must be above the horizon (sun.y > 0), got [${spec.sun.join(", ")}]`);
  const things = spec.things.map(drawable);
  const names = new Set<string>();
  for (const t of things) {
    if (names.has(t.name)) throw new Error(`scene: two things are named "${t.name}" (names key shaders)`);
    names.add(t.name);
  }
  return {
    kind: "scene", sea: spec.sea, things, style: spec.style,
    sun: [spec.sun[0] / l, spec.sun[1] / l, spec.sun[2] / l], cameras: spec.cameras ?? {},
  };
}
