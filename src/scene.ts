// A scene says what things are; `compile` decides where each part is evaluated.

import { OPEN, type Camera, type Loop } from "./artefact.ts";
import type { Drawable, V3 } from "./compile/context.ts";
import { light } from "./draw/light.ts";
import { shadows } from "./draw/shadows.ts";
import { sky } from "./draw/sky.ts";
import type { Ground } from "./draw/ground.ts";
import type { Style } from "./style.ts";
import type { In } from "./expr.ts";
import type { Light, Lighting, Sun, SunAmount } from "./light.ts";
import { SceneError } from "./errors.ts";
import { refuseScene } from "./mistakes.ts";

export type SceneSpec = {
  /** What the scene stands on: `sea({ swell })`, `plane` (flat, y = 0) or `nothing` (src/draw/ground.ts). */
  readonly ground: Ground;
  /** Strands, crests, drawn surfaces and points, or any other `Drawable`. */
  readonly things: readonly Drawable[];
  readonly style: Style;
  /**
   * Direction toward the sun (normalised here): a vector, fixed, or `movable([x, y, z])` (src/light.ts), which makes it
   * the host input `sun` with that direction as its default. Either is the sun at full strength. `{ toward, amount }`:
   * a dimmer or coloured sun, a moon, or none (`amount: [0, 0, 0]`); `amount` is a linear colour, 1 the sun at noon.
   */
  readonly sun: SunDirection | { readonly toward: SunDirection; readonly amount: In<"vec3"> };
  /** Lights besides the sun, each a function of a world point (`Light`, `lamp`). At most 7. */
  readonly lights?: readonly Light[];
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
/** A direction toward the sun: a vector, or `movable(dir)`. */
type SunDirection = readonly [number, number, number] | Extract<Sun, { kind: "input" }>;

/** Lights besides the sun a scene may carry: each is a term in every lit fragment's shader (docs/lighting.md). */
export const MOST_LIGHTS = 7;

export type OutlineMethod = "hull" | "screen";
export const DEFAULT_OUTLINE: OutlineMethod = "hull";

export type Scene = {
  readonly ground: Ground;
  /** Everything drawn: the sky, the ground and the shadows the scene derives, then the spec's things. */
  readonly things: readonly Drawable[];
  readonly style: Style;
  /** The sun as stated: the fixed direction, or a movable sun's default. */
  readonly sun: V3;
  /** The sun as shaders read it: that literal, or a host input. */
  readonly sunAt: Sun;
  /** The sun, its amount and the scene's lights, as styles receive them (src/light.ts). */
  readonly lighting: Lighting;
  readonly cameras: Readonly<Record<string, Camera>>;
  readonly outline: OutlineMethod;
  readonly loop: Loop;
};

/**
 * Assembles a scene: a ground, the things drawn (each a `Drawable`), a style, a sun direction above the horizon, and
 * optionally named cameras, an outline method and a loop period. Returns a `Scene`, which `compile` turns into an
 * artefact a host draws. The sky, ground and shadows are added to `things` for you. Throws if two things share a name,
 * the sun is not above the horizon, or the period is not a positive number of seconds.
 */
export function scene(spec: SceneSpec): Scene {
  refuseScene(spec);
  // Classified once: a direction alone is the sun at full strength; `{ toward, amount }` states its amount.
  const given = "toward" in spec.sun ? spec.sun.toward : (spec.sun as SunDirection);
  const amount: SunAmount = "toward" in spec.sun ? { kind: "given", amount: spec.sun.amount } : { kind: "full" };
  const stated: readonly [number, number, number] = "kind" in given ? given.dir : given;
  const l = Math.hypot(...stated);
  if (!(stated[1] > 0.05 * l)) throw new Error(`scene: the sun must be above the horizon (sun.y > 0), got [${stated.join(", ")}]`);
  const sun: V3 = [stated[0] / l, stated[1] / l, stated[2] / l];
  const sunAt: Sun = "kind" in given ? { kind: "input", name: given.name, dir: sun } : { kind: "fixed", dir: sun };
  const lights = spec.lights ?? [];
  if (lights.length > MOST_LIGHTS) {
    throw new SceneError("scene()", `lights: ${lights.length} lights`, `at most ${MOST_LIGHTS} besides the sun (each is a term in every lit fragment)`,
      "combine sources in one function, as a row by repetition: (p) => lamp(vec3(mul(floor(add(mul(p.x, 1 / 4), 0.5)), 4), 3, 0), o)(p)");
  }
  lights.forEach((l, k) => {
    if (typeof l !== "function") throw new SceneError("scene()", `lights[${k}]: ${typeof l}`, "a Light: a function of a world point returning { toward, amount }", "lights: [lamp([0, 2, 0], { colour: [1, 0.8, 0.5], reach: 1.5 })]");
  });
  const things: Drawable[] = [sky, spec.ground, shadows, ...blockOf(spec.style), ...spec.things];
  const names = new Set<string>();
  for (const t of things) {
    if (names.has(t.name)) {
      const own = [sky, spec.ground, shadows, light].some((d) => d.name === t.name);
      throw new SceneError("scene()", `two things named "${t.name}"`, own ? `a name the scene's own ${t.name} does not take` : "a different name for each thing (names key shaders)", `rename one: "${t.name}2"`);
    }
    names.add(t.name);
  }
  if (spec.period !== undefined && !(spec.period > 0 && Number.isFinite(spec.period))) throw new Error(`scene: the period must be a positive number of seconds, got ${spec.period}`);
  const loop: Loop = spec.period === undefined ? OPEN : { kind: "every", seconds: spec.period };
  return { ground: spec.ground, things, style: spec.style, sun, sunAt, lighting: { sun: sunAt, amount, lights, sampling: { kind: "point" }, shadows: new Map() }, cameras: spec.cameras ?? {}, outline: spec.outline ?? DEFAULT_OUTLINE, loop };
}

/** A style that prints its shadow as a block after the bodies (`Style.block`) has it drawn by `light` (src/draw/light.ts). */
const blockOf = (style: Style): readonly Drawable[] => (style.block === undefined ? [] : [light]);

/** The same scene in another style: the things as written, and the new style's shadow block in place of the old one's. */
export function restyled(scene: Scene, style: Style): Scene {
  const written = scene.things.filter((t) => t !== light);
  const at = written.indexOf(shadows) + 1;
  return { ...scene, style, things: [...written.slice(0, at), ...blockOf(style), ...written.slice(at)] };
}
