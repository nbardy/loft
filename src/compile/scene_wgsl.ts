// The WGSL every shader shares (`Artefact.sceneWgsl`): the tables' bindings and accessors, the ground's height field
// (`sea_height`, whatever the ground: src/draw/ground.ts), the style's functions and small helpers. Module-scope WGSL is order-independent, so the parts are simply concatenated.

import coreWgsl from "../wgsl/core.wgsl" with { type: "text" };
import { Expr, lift, variable, vec2, withIdentities } from "../expr.ts";
import { SceneError } from "../errors.ts";
import { levelOf, type Ground } from "../draw/ground.ts";
import { shadeVariables, type Style } from "../style.ts";
import { foldAt, sunDir, sunOnly, type Lighting } from "../light.ts";
import { emitFn, type Slots } from "./bind.ts";

export const lit = (x: number): string => (Number.isInteger(x) ? `${x}.0` : String(x));
export const vec3Lit = (v: readonly number[]): string => `vec3<f32>(${v.map(lit).join(", ")})`;

/** The style's and the ground's functions. Emitted once, before the drawables' shared code is collected. */
export function styleWgsl(slots: Slots, style: Style, ground: Ground, sun: readonly [number, number, number], lighting: Lighting): string {
  const at = lighting.sun;
  const c = <T extends "f32" | "vec3">(name: string, ty: T) => variable(`c_${name}`, ty);
  const shadeCtx = shadeVariables(lighting, style);
  // A style that prints its shadow as a block afterwards (`Style.block`) gives the bodies its colour before the block;
  // `style.shade` stays the whole colour for whatever shades a fragment on its own (a smear).
  const body = withIdentities(() => (style.block === undefined ? style.shade(shadeCtx) : style.block.body(shadeCtx)));
  const shadeArgs = [["c_base", "vec3"], ["c_normal", "vec3"], ["c_view", "vec3"], ["c_world", "vec3"], ["c_depth", "f32"], ["c_uv", "vec2"], ["c_anchor", "vec3"], ["c_id", "f32"]] as const;
  const layerCtx = { world: c("world", "vec3"), view: c("view", "vec3"), distance: c("distance", "f32"), amount: c("amount", "f32") };
  const layerArgs = [["c_world", "vec3"], ["c_view", "vec3"], ["c_distance", "f32"], ["c_amount", "f32"]] as const;
  const xz = variable("xz", "vec2");
  // What the ground's surface is coloured by where a shadow lies on it: the sea's shadow layer (blended over the water),
  // or the plane's own colour at the shadow's coverage (src/draw/shadows.ts); nothing has no surface to colour.
  const onGround = (): string[] => {
    switch (ground.kind) {
      case "sea": return [emitFn(slots, "sea_shadow", layerArgs, "vec4<f32>", [style.shadow(layerCtx)], (v) => v[0])];
      case "plane": {
        const groundCtx = { world: c("world", "vec3"), view: c("view", "vec3"), distance: c("distance", "f32"), sun: lift<"vec3">(sunDir(at)), shadow: c("shadow", "f32"), lights: foldAt(lighting, { kind: "surface", at: c("world", "vec3"), normal: lift<"vec3">([0, 1, 0]) }) };
        return [emitFn(slots, "ground_colour", [["c_world", "vec3"], ["c_view", "vec3"], ["c_distance", "f32"], ["c_shadow", "f32"]], "vec3<f32>", [style.ground(groundCtx)], (v) => v[0])];
      }
      case "nothing": return [];
    }
  };
  const gradient = (): Expr<"vec2"> => {
    switch (ground.kind) {
      case "sea": return ground.gradient(xz);
      case "plane": case "nothing": return vec2(0, 0);
    }
  };
  return [
    `const SUN: vec3<f32> = ${vec3Lit(sun)};`,
    emitFn(slots, "sea_height", [["xz", "vec2"]], "f32", [levelOf(ground)(xz)], (v) => v[0]),
    emitFn(slots, "sea_gradient", [["xz", "vec2"]], "vec2<f32>", [gradient()], (v) => v[0]),
    emitFn(slots, "shade", shadeArgs, "vec3<f32>", [body], (v) => v[0]),
    emitFn(slots, "ink", shadeArgs, "vec3<f32>", [withIdentities(() => style.ink(shadeCtx))], (v) => v[0]),
    emitFn(slots, "mark", shadeArgs, "f32", [withIdentities(() => style.mark(shadeCtx))], (v) => v[0]),
    ...onGround(),
    emitFn(slots, "sky", [["c_dir", "vec3"]], "vec3<f32>", [style.sky({ dir: c("dir", "vec3"), sun: lift<"vec3">(sunDir(at)), lights: foldAt(sunOnly(lighting), { kind: "point", at: c("dir", "vec3") }) })], (v) => v[0]),
    emitFn(slots, "outline_world", [], "f32", [outlineOf(style)], (v) => v[0]),
  ].join("\n");
}

/**
 * The style's outline width as an expression. Friction S1 (2026-11-16): `{ ...toon(), outline: 0.02 }` reached the
 * emitter as a bare number and failed inside it (`n.op` of undefined), naming neither the style nor the field. A number
 * is lifted (the type allows one); anything else is refused here by name.
 */
function outlineOf(style: Style): Expr<"f32"> {
  const o: unknown = style.outline;
  if (typeof o === "number" && Number.isFinite(o)) return lift<"f32">(o);
  if (o instanceof Expr && o.ty === "f32") return o as Expr<"f32">;
  throw new SceneError(`style "${style.name}" outline`, o instanceof Expr ? `an expression of type ${o.ty}` : typeof o === "number" ? String(o) : typeof o, "the ink width in world units: a finite number or an f32 expression", "outline: 0.02 (or 0 for no ink)");
}

/** Helpers with no scene content: a surface vertex, grid corners, pixel sizes, the outline width, sea footprints. */
export const CORE_WGSL = coreWgsl;
