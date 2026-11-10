// Loft's public authoring API and compiler.
export * from "./expr.ts";
export { keys, ramp, through, wave, retarded, response, pulse, ease, type Curve, type Key } from "./curve.ts";
export { strand, rotate, Strand, type StrandSpec, type TipSpec, type PaintSpec, type Rgb, type Placement } from "./geom/strand.ts";
export { attach, type AttachSpec, type Attachable } from "./geom/attach.ts";
export { scatter, Scatter, type ScatterSpec } from "./geom/scatter.ts";
export { crest, Crest, type CrestSpec } from "./geom/crest.ts";
export { sea, Sea, type Chart, type SeaSpec, type SwellSpec } from "./chart.ts";
export { toon, palette, dorsalness, type Style, type ShadeCtx, type WaterCtx, type LayerCtx, type SkyCtx, type ToonSpec } from "./style.ts";
export { scene, type Scene, type SceneSpec } from "./scene.ts";
export { compile, compileScene, type Compiled } from "./compile/index.ts";
export { draw, isSkeleton, type BodyPoint, type ChartRef, type CompileContext, type CpuRef, type Drawable, type DrawSpec, type Emitted, type Phase, type PoseFrame, type PostSpec, type Skeleton, type StationRange, type TableRef, type TableSpec } from "./compile/context.ts";
export { emitFn } from "./compile/bind.ts";
export { TableBudgetError } from "./compile/tables.ts";
export type { Artefact } from "./artefact.ts";
