// Core layer: the expression language, curves, the map algebra, drawables, relief, the scene, the sun and the compiler.
// Everything a scene needs whatever it draws.
export * from "./expr.ts";
export { keys, ramp, through, wave, retarded, response, pulse, ease, type Curve as KeyCurve, type Key } from "./curve.ts";
export * from "./map/index.ts";
export { draw, type Bounds, type CurveOptions, type DensityOptions, type FieldOptions, type Instanceable, type Poseable, type ReliefOptions } from "./draw/index.ts";
export { spacing, SurfaceDrawable, type Fragment, type Grid, type Look, type Outline, type Sampling, type SurfaceOptions } from "./draw/surface.ts";
export { scene, DEFAULT_OUTLINE, type OutlineMethod, type Scene, type SceneSpec } from "./scene.ts";
export { plane, nothing, GroundError, type Ground, type Plane, type Nothing } from "./draw/ground.ts";
export { compile, compileScene, type Compiled, type CpuFn } from "./compile/index.ts";
export { drawSpec, type Bakeable, type CompileContext, type Drawable, type DrawSpec, type Emitted, type Phase, type PostSpec, type Shader, type TableRef } from "./compile/context.ts";
export { emitFn, emitShaderFn } from "./compile/bind.ts";
export { TableBudgetError } from "./compile/tables.ts";
export type { Artefact } from "./artefact.ts";
export { bound, type TraceBound, type TraceOutline } from "./draw/trace.ts";
export { checkField, FieldDrawable, fieldPixel, FieldSafetyError, LIPSCHITZ_LIMIT, measureField, type FieldHit, type FieldSafetyFailure } from "./draw/field.ts";
export { impostors, Impostors, type Cell, type ImpostorHit, type ImpostorOptions, type ImpostorShape } from "./draw/impostors.ts";
export { Copies, drawCopies, type CopiesHit, type CopiesOptions } from "./draw/copies.ts";
export * from "./relief/index.ts";
export { drawRelief, ReliefDrawable, type ReliefPoint, type ReliefSite } from "./draw/relief.ts";
export { movable, type Sun } from "./sun.ts";

// What recipes under examples/ need of the core (examples/README.md).
export { noise2, type Branch } from "./lib/index.ts";
export type { Camera } from "./artefact.ts";
