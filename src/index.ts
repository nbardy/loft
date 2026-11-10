// Loft's public authoring API and compiler.
export * from "./expr.ts";
export { keys, ramp, through, wave, retarded, response, pulse, ease, type Curve as KeyCurve, type Key } from "./curve.ts";
export * from "./map/index.ts";
export { draw, NotImplementedError, type Bounds, type CurveOptions, type DensityOptions, type FieldOptions, type Instanceable, type ReliefOptions } from "./draw/index.ts";
export { spacing, SurfaceDrawable, type Fragment, type Grid, type Look, type Outline, type Sampling, type SurfaceOptions } from "./draw/surface.ts";
export { strand, Strand, type StrandSpec, type TipSpec, type PaintSpec, type Rgb } from "./geom/strand.ts";
export { crest, type CrestSpec } from "./geom/crest.ts";
export { sea, Sea, type SeaSpec, type SwellSpec } from "./geom/sea.ts";
export { toon, palette, dorsalness, type Style, type ShadeCtx, type WaterCtx, type LayerCtx, type SkyCtx, type ToonSpec } from "./style.ts";
export { scene, type Scene, type SceneSpec } from "./scene.ts";
export { compile, compileScene, type Compiled, type CpuFn } from "./compile/index.ts";
export { drawSpec, type Bakeable, type CompileContext, type Drawable, type DrawSpec, type Emitted, type Phase, type PostSpec, type Shader, type TableRef } from "./compile/context.ts";
export { emitFn, emitShaderFn } from "./compile/bind.ts";
export { TableBudgetError } from "./compile/tables.ts";
export type { Artefact } from "./artefact.ts";
