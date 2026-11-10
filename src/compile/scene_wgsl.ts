// The WGSL every shader shares (`Artefact.sceneWgsl`): the tables' bindings and accessors, the station reader, the
// sea's height field, the style's functions, small helpers, and whatever drawables registered with `ctx.shared`
// (charts, their own helpers). Module-scope WGSL is order-independent, so the parts are simply concatenated.

import { lift, variable } from "../expr.ts";
import type { Sea } from "../chart.ts";
import { STATIONS_WGSL } from "../geom/stations.ts";
import type { Style } from "../style.ts";
import { emitFn, type Slots } from "./bind.ts";
import { BODY_POINT_WGSL } from "./charts.ts";

export const lit = (x: number): string => (Number.isInteger(x) ? `${x}.0` : String(x));
export const vec3Lit = (v: readonly number[]): string => `vec3<f32>(${v.map(lit).join(", ")})`;

/** The style's and the sea's functions. Emitted once, before the drawables' shared code is collected. */
export function styleWgsl(slots: Slots, style: Style, sea: Sea, sun: readonly [number, number, number]): string {
  const c = <T extends "f32" | "vec3">(name: string, ty: T) => variable(`c_${name}`, ty);
  const shadeCtx = { base: c("base", "vec3"), normal: c("normal", "vec3"), sun: lift<"vec3">(sun), view: c("view", "vec3"), world: c("world", "vec3"), depth: c("depth", "f32") };
  const shadeArgs = [["c_base", "vec3"], ["c_normal", "vec3"], ["c_view", "vec3"], ["c_world", "vec3"], ["c_depth", "f32"]] as const;
  const waterCtx = {
    normal: c("normal", "vec3"), sun: lift<"vec3">(sun), view: c("view", "vec3"), world: c("world", "vec3"),
    crest: c("crest", "f32"), distance: c("distance", "f32"), foam: c("foam", "f32"),
  };
  const waterArgs = [["c_normal", "vec3"], ["c_view", "vec3"], ["c_world", "vec3"], ["c_crest", "f32"], ["c_distance", "f32"], ["c_foam", "f32"]] as const;
  const layerCtx = { world: c("world", "vec3"), view: c("view", "vec3"), distance: c("distance", "f32"), amount: c("amount", "f32") };
  const layerArgs = [["c_world", "vec3"], ["c_view", "vec3"], ["c_distance", "f32"], ["c_amount", "f32"]] as const;
  const xz = variable("xz", "vec2");
  const amplitude = sea.waves.reduce((a, w) => a + w.amp, 0);
  return [
    `const SUN: vec3<f32> = ${vec3Lit(sun)};`,
    `const SEA_AMPLITUDE: f32 = ${lit(Math.max(amplitude, 1e-3))};`,
    `const SEA_SHORTEST: f32 = ${lit(sea.waves.reduce((l, w) => Math.min(l, (2 * Math.PI) / w.k), 1e3))};`,
    emitFn(slots, "sea_height", [["xz", "vec2"]], "f32", [sea.height(xz)], (v) => v[0]),
    emitFn(slots, "sea_gradient", [["xz", "vec2"]], "vec2<f32>", [sea.gradient(xz)], (v) => v[0]),
    emitFn(slots, "shade", shadeArgs, "vec3<f32>", [style.shade(shadeCtx)], (v) => v[0]),
    emitFn(slots, "ink", shadeArgs, "vec3<f32>", [style.ink(shadeCtx)], (v) => v[0]),
    emitFn(slots, "water", waterArgs, "vec4<f32>", [style.water(waterCtx)], (v) => v[0]),
    emitFn(slots, "sea_shadow", layerArgs, "vec4<f32>", [style.shadow(layerCtx)], (v) => v[0]),
    emitFn(slots, "sky", [["c_dir", "vec3"]], "vec3<f32>", [style.sky({ dir: c("dir", "vec3"), sun: lift<"vec3">(sun) })], (v) => v[0]),
    emitFn(slots, "outline_world", [], "f32", [style.outline], (v) => v[0]),
  ].join("\n");
}

/** Helpers with no scene content: grid corners, pixel sizes, the outline width, the sea grid's centre. */
export const CORE_WGSL = /* wgsl */ `
struct Surf { pos: vec3<f32>, normal: vec3<f32> };
${BODY_POINT_WGSL}
${STATIONS_WGSL}
/// Grid corner offsets of the six vertices of a quad: triangles (0,0)(1,0)(1,1) and (0,0)(1,1)(0,1).
fn quad_corner(c: u32) -> vec2<u32> {
  return vec2<u32>(select(0u, 1u, c == 1u || c == 2u || c == 4u), select(0u, 1u, c == 2u || c == 4u || c == 5u));
}
/// World size of one pixel at p, across the view (exact for perspective and orthographic cameras alike).
fn pixel_world(p: vec3<f32>) -> f32 {
  let clip = frame.view_proj * vec4<f32>(p, 1.0);
  // Row 1 of view_proj is the projection's y scale times a unit row of the view rotation (perspective and
  // orthographic alike), so its length is that scale. One pixel at depth w is then 2 w / (height * scale).
  let fy = length(vec3<f32>(frame.view_proj[0][1], frame.view_proj[1][1], frame.view_proj[2][1]));
  return 2.0 * clip.w / (frame.viewport.y * fy);
}
/// The outline's offset: the style's world width, but never thinner than 1.5 pixels (so far views keep their ink).
fn ink_width(p: vec3<f32>) -> f32 {
  return max(outline_world(), 1.5 * pixel_world(p));
}
/// Longest extent of one pixel's footprint on the sea at p: a pixel stretched by the grazing angle. Analytic, so it is
/// smooth across the sea grid's triangles. (It was fwidth of the interpolated position, which jumps from one large far
/// triangle to the next and drew their outlines into the water as light wedges.)
fn sea_footprint(p: vec3<f32>) -> f32 {
  return pixel_world(p) / max(abs(view_dir(p).y), 0.02);
}`;
