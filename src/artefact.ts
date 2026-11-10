// The contract between the compiler and a host.
//
// The compiler turns a scene description into an `Artefact`: shader source, tables of numbers, a draw list and a pose
// function. A host draws an Artefact and knows nothing about creatures, curves or charts. Today the host is
// TypeScript on WebGPU (browser, and bun headless); the same contract is what a Rust/wgpu host inside a game would
// implement. Anything a host must know is in this file.
//
// Conventions a host can rely on, and a compiler must follow:
// - World: y is up, the sea is near y = 0, units are arbitrary (a small ship is about 5 long).
// - Clip space is WebGPU's: depth 0 near, 1 far. Front faces wind counter-clockwise.
// - Colour: shaders write display-referred sRGB values to an `rgba8unorm` target. No automatic conversion anywhere.
// - Every draw is "V vertices x I instances" with no vertex buffers. The vertex shader derives everything from
//   `@builtin(vertex_index)` and `@builtin(instance_index)` and the bound tables.
// - Bind group 0 is the same for every artefact (PRELUDE_WGSL below). Bind group 1 is declared by the artefact
//   (`sceneWgsl`) and holds its per-frame tables. Hosts build EXPLICIT bind group layouts for both groups, with
//   every binding visible to vertex and fragment stages, because a shader may use any subset.

import { CURVE_N, type NumTy } from "./expr.ts";

/** Prepended by the host to every shader module, before `Artefact.sceneWgsl` and the shader itself. */
export const PRELUDE_WGSL = /* wgsl */ `
struct Frame {
  view_proj: mat4x4<f32>,
  inv_view_proj: mat4x4<f32>,
  eye: vec4<f32>,       // xyz: eye position in world space. w: 1 perspective, 0 orthographic
  forward: vec4<f32>,   // xyz: unit view direction (the ray direction for an orthographic camera)
  viewport: vec4<f32>,  // width, height, 1 / width, 1 / height, in pixels
  time: vec4<f32>,      // x: seconds
};
@group(0) @binding(0) var<uniform> frame: Frame;
@group(0) @binding(1) var<storage, read> inputs: array<f32>;        // host inputs, by slot
@group(0) @binding(2) var<storage, read> params: array<f32>;        // tunable numbers, by slot
@group(0) @binding(3) var<storage, read> curves: array<vec4<f32>>;  // rows of CURVE_N samples

const CURVE_N: u32 = ${CURVE_N}u;
/// Sample curve row \`r\` at x in [0, 1]: linear interpolation, clamped at the ends.
fn curve(r: u32, x: f32) -> vec4<f32> {
  let s = clamp(x, 0.0, 1.0) * f32(CURVE_N - 1u);
  let i = min(u32(floor(s)), CURVE_N - 2u);
  return mix(curves[r * CURVE_N + i], curves[r * CURVE_N + i + 1u], s - f32(i));
}
/// Unit direction from the eye toward a world point (the camera's forward direction when orthographic).
fn view_dir(world: vec3<f32>) -> vec3<f32> {
  return normalize(mix(frame.forward.xyz, world - frame.eye.xyz, frame.eye.w));
}
`;

export type Camera =
  | { readonly kind: "perspective"; readonly eye: readonly [number, number, number]; readonly target: readonly [number, number, number]; readonly fovY: number }
  | { readonly kind: "orthographic"; readonly eye: readonly [number, number, number]; readonly target: readonly [number, number, number]; readonly height: number };

/** `slot` is a float offset into the `inputs` array; an input occupies as many floats as its type has components. */
export type InputSlot = { readonly name: string; readonly ty: NumTy; readonly slot: number; readonly def: readonly number[] };
/** `slot` is a float offset into the `params` array. `min` and `max` are for editors: a host does not clamp. */
export type ParamSlot = { readonly name: string; readonly slot: number; readonly def: number; readonly min: number; readonly max: number };

/** A per-frame table in bind group 1, declared in `sceneWgsl` as `var<storage, read> NAME: array<vec4<f32>>`. */
export type DynamicBuffer = { readonly name: string; readonly binding: number; readonly vec4s: number };

export type Draw = {
  readonly label: string;
  /** Key into `Artefact.shaders`. The module has entry points `vs` and `fs`. */
  readonly shader: string;
  readonly vertices: number;
  readonly instances: number;
  /** Opaque draws run first, in list order; then transparent draws, in list order. */
  readonly pass: "opaque" | "transparent";
  readonly cull: "none" | "back" | "front";
  readonly depthWrite: boolean;
  /** alpha: straight alpha over (src-alpha, one-minus-src-alpha). add: additive scaled by alpha (src-alpha, one). */
  readonly blend: "none" | "alpha" | "add";
  /** Scene geometry, or construction lines the host can show and hide. */
  readonly layer: "scene" | "construction";
  /** WGSL `override` constants for this draw (for example the grid density). */
  readonly constants: Readonly<Record<string, number>>;
  /**
   * Transparent draws only. When set, the host prepends SCENE_DEPTH_WGSL to this draw's shader, so its fragment stage
   * can read the opaque scene behind it: `scene_depth(px)` (the depth buffer value, 0 near to 1 far) and
   * `scene_position(px)` (the world position of the opaque surface at that pixel). Used for foam where things meet,
   * soft volume edges and depth fades.
   */
  readonly sceneDepth?: boolean;
};

/**
 * A full-screen pass run after the scene is drawn and resolved, in list order. The module has one entry point, `fs`;
 * the host supplies the vertex stage and prepends POST_WGSL, which gives `post_color(uv)` (the picture so far),
 * `scene_depth(px)` and `scene_position(px)`. Used by styles: ink lines from depth edges, paper grain, bloom.
 */
export type PostPass = { readonly label: string; readonly shader: string };

/** What a host hands the pose function every frame. `inputs` and `params` are laid out by slot. */
export type FrameState = { readonly time: number; readonly inputs: Float32Array; readonly params: Float32Array };

export type Artefact = {
  readonly version: 1;
  /** Background colour, sRGB 0..1. */
  readonly clear: readonly [number, number, number];
  readonly inputs: readonly InputSlot[];
  readonly inputFloats: number;
  readonly params: readonly ParamSlot[];
  /** rows x CURVE_N x 4 floats. Static until a table-backed curve is edited. */
  readonly curves: Float32Array;
  readonly dynamic: readonly DynamicBuffer[];
  /** Bind group 1 declarations plus generated functions shared by every shader. */
  readonly sceneWgsl: string;
  readonly shaders: Readonly<Record<string, string>>;
  readonly draws: readonly Draw[];
  /** Full-screen passes after the scene. Absent means none. */
  readonly post?: readonly PostPass[];
  /**
   * Evaluate everything that is computed once per frame on the CPU: spine stations, waterlines, shadows.
   * Returns one Float32Array per dynamic buffer, keyed by name, of length `vec4s * 4`. Pure: same state, same output.
   * (v0 carries this as a JavaScript function. A serialisable pose program is planned so a non-JS host can run it.)
   */
  pose(state: FrameState): Readonly<Record<string, Float32Array>>;
  /** Views the scene suggests, by name. */
  readonly cameras: Readonly<Record<string, Camera>>;
  readonly stats: { readonly triangles: number; readonly draws: number; readonly curveRows: number; readonly stations: number };
};

/** One frame to draw. Missing inputs and params take the artefact's defaults; `time` also fills the `time` input. */
export type View = {
  readonly camera: Camera;
  readonly width: number;
  readonly height: number;
  readonly time: number;
  readonly inputs?: Readonly<Record<string, number | readonly number[]>>;
  readonly params?: Readonly<Record<string, number>>;
  readonly construction?: boolean;
};

/** What every host implements. `frame` returns tightly packed RGBA8, top row first. */
export interface Host {
  load(artefact: Artefact): Promise<Loaded>;
}
export interface Loaded {
  frame(view: View): Promise<Uint8Array>;
  /** Draw the same frame into a texture the caller owns (a canvas's current texture), sized as the view. */
  draw(view: View, target: GPUTexture): Promise<void>;
  /** Replace table-backed curve data without rebuilding pipelines. */
  setCurves(curves: Float32Array): void;
  destroy(): void;
}
