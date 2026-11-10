// Loft's public authoring API and compiler.
export * from "./expr.ts";
export { keys, ramp, through, wave, retarded, response, pulse, ease, type Curve, type Key } from "./curve.ts";
export { strand, frameOf, skinPoint, rotate, Strand, type StrandSpec, type TipSpec, type PaintSpec, type Rgb } from "./geom/strand.ts";
export { crest, Crest, type CrestSpec } from "./geom/crest.ts";
export { sea, Sea, type Chart, type SeaSpec, type SwellSpec } from "./chart.ts";
export { toon, palette, dorsalness, type Style, type ShadeCtx, type WaterCtx, type SkyCtx, type ToonSpec } from "./style.ts";
export { scene, type Scene, type SceneSpec } from "./scene.ts";
export { compile } from "./compile/index.ts";
export type { CompileContext, Drawable, Emitted, StationRange } from "./compile/context.ts";
export type { Artefact } from "./artefact.ts";
