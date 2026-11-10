// A scene says what things are; `compile` decides where each part is evaluated.

import type { Camera } from "./artefact.ts";
import type { Sea } from "./chart.ts";
import type { Drawable } from "./compile/context.ts";
import { shadows } from "./draw/shadows.ts";
import { sky } from "./draw/sky.ts";
import type { V3 } from "./geom/stations.ts";
import type { Style } from "./style.ts";

export type SceneSpec = {
  readonly sea: Sea;
  /** Strands, crests, scatters, or any other `Drawable`. */
  readonly things: readonly Drawable[];
  readonly style: Style;
  /** Direction toward the sun (normalised here). */
  readonly sun: readonly [number, number, number];
  readonly cameras?: Readonly<Record<string, Camera>>;
};

export type Scene = {
  readonly sea: Sea;
  /** Everything drawn: the sky, the sea and the shadows the scene derives, then the spec's things. */
  readonly things: readonly Drawable[];
  readonly style: Style;
  readonly sun: V3;
  readonly cameras: Readonly<Record<string, Camera>>;
};

export function scene(spec: SceneSpec): Scene {
  const l = Math.hypot(...spec.sun);
  if (!(spec.sun[1] > 0.05 * l)) throw new Error(`scene: the sun must be above the horizon (sun.y > 0), got [${spec.sun.join(", ")}]`);
  const things: Drawable[] = [sky, spec.sea, shadows, ...spec.things];
  const names = new Set<string>();
  for (const t of things) {
    if (names.has(t.name)) throw new Error(`scene: two things are named "${t.name}" (names key shaders)`);
    names.add(t.name);
  }
  return { sea: spec.sea, things, style: spec.style, sun: [spec.sun[0] / l, spec.sun[1] / l, spec.sun[2] / l], cameras: spec.cameras ?? {} };
}
