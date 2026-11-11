// A scene says what things are; `compile` decides where each part is evaluated.

import { OPEN, type Camera, type Loop } from "./artefact.ts";
import type { Drawable, V3 } from "./compile/context.ts";
import { shadows } from "./draw/shadows.ts";
import { sky } from "./draw/sky.ts";
import type { Sea } from "./geom/sea.ts";
import type { Style } from "./style.ts";

export type SceneSpec = {
  readonly sea: Sea;
  /** Strands, crests, drawn surfaces and points, or any other `Drawable`. */
  readonly things: readonly Drawable[];
  readonly style: Style;
  /** Direction toward the sun (normalised here). */
  readonly sun: readonly [number, number, number];
  readonly cameras?: Readonly<Record<string, Camera>>;
  /** How ink outlines are drawn (`OutlineMethod`). Default `DEFAULT_OUTLINE`. */
  readonly outline?: OutlineMethod;
  /** The scene comes round every this many seconds (its author has made every motion periodic in it). Default: it does not loop (`OPEN`). */
  readonly period?: number;
};

/**
 * hull: every outlined surface drawn a second time, pushed out along its normal, front faces culled.
 * screen: one full-screen pass after the scene finds the outlined surfaces' silhouettes in the depth (src/draw/ink.ts).
 */
export type OutlineMethod = "hull" | "screen";
export const DEFAULT_OUTLINE: OutlineMethod = "hull";

export type Scene = {
  readonly sea: Sea;
  /** Everything drawn: the sky, the sea and the shadows the scene derives, then the spec's things. */
  readonly things: readonly Drawable[];
  readonly style: Style;
  readonly sun: V3;
  readonly cameras: Readonly<Record<string, Camera>>;
  readonly outline: OutlineMethod;
  readonly loop: Loop;
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
  if (spec.period !== undefined && !(spec.period > 0 && Number.isFinite(spec.period))) throw new Error(`scene: the period must be a positive number of seconds, got ${spec.period}`);
  const loop: Loop = spec.period === undefined ? OPEN : { kind: "every", seconds: spec.period };
  return { sea: spec.sea, things, style: spec.style, sun: [spec.sun[0] / l, spec.sun[1] / l, spec.sun[2] / l], cameras: spec.cameras ?? {}, outline: spec.outline ?? DEFAULT_OUTLINE, loop };
}
