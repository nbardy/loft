// The WebGPU host: draws an Artefact (src/artefact.ts) headless (read back to RGBA8) or into a canvas texture.
//
// Both entry points share one path: `pose` writes the frame state and the `cpu` tables and submits the pose stages
// (compute dispatches that fill every other table on the GPU, docs/gpu_native.md), copying the level-of-detail bounds
// they wrote to a readback buffer; `record` records the passes that end in a caller-given texture view. `frame` waits
// for this frame's bounds and renders into its own texture and reads it back; `draw` renders into the caller's texture
// (a canvas's current texture) with the last bounds read, never waiting on the GPU.
//
// Queue order: every `writeBuffer` of a frame is issued before the submit of the pose stages that read it, and the
// level-of-detail slots are written before the submit of the draws (writes are ordered with submits, not with passes
// inside one encoder: a write between two passes of one encoder would land before both).
//
// The passes depend on the artefact, decided once at load (`Schedule`):
//   plain   no draw reads scene depth and there is no post: ONE multisampled pass, opaque then transparent, resolved
//           into the output. (The bytes of such artefacts are guarded by the determinism tests.)
//   split   some transparent draw sets `sceneDepth`, or there are post passes:
//             1. opaque pass: 4x colour and 4x depth, both stored, no resolve
//             2. depth resolve: a full-screen pass reads the 4x depth as texture_depth_multisampled_2d and writes the
//                nearest of each pixel's samples to a single-sampled r32float texture (SCENE_DEPTH_WGSL, group 2)
//             3. transparent pass: loads the stored colour and depth (so transparent draws still depth-test, and may
//                still write depth), binds the resolved depth as group 2, resolves colour into the output, or into
//                the first intermediate when there are post passes
//             4. post passes, ping-ponging between two single-sampled intermediates; the last writes the output.
// Why a resolve into a separate texture rather than sampling the depth attachment itself during the transparent
// pass: sampling an attachment needs it read-only, which would forbid transparent draws that write depth; a
// multisampled depth binding would make every reader pick or average samples itself; and post passes need a
// single-sampled depth after the scene pass has ended anyway. One small resolve serves draws and post alike, and a
// Rust/wgpu host can do exactly the same.
//
// This file imports no bun-only module, so a browser bundle of src/host (minus headless.ts) stays clean.

import {
  DETAIL_SLOTS, POSE_WORKGROUP, POST_WGSL, PRELUDE_WGSL, SCENE_DEPTH_WGSL, type Artefact, type Draw, type FrameState, type FrameStats, type PoseStage,
  type PostPass, type Host, type Loaded, type View,
} from "../artefact.ts";
import { CURVE_N, type NumTy } from "../expr.ts";
import { cameraMatrices } from "./camera.ts";
import { gridBounds, planFrame, slotsUsed, type Bounds, type Memory, type Plan } from "./detail.ts";
import { profileFrames, type Profile } from "./profile.ts";
export type { Profile, ProfileDraw, ProfileStage } from "./profile.ts";

/** The colour target format. A canvas drawn through `draw` must be configured with this format. */
export const COLOR_FORMAT: GPUTextureFormat = "rgba8unorm";
export const DEPTH_FORMAT: GPUTextureFormat = "depth32float";
export const SAMPLES = 4;

export type FrameTimings = {
  /** CPU: input layout, the `cpu` tables (`artefact.cpuPose`), their uploads and the pose stages' submit. */
  readonly pose: number;
  /** CPU: buffer uploads, command encoding and submit. */
  readonly encode: number;
  /** GPU execution plus the wait for the mapped readback and repacking rows. */
  readonly readback: number;
};

export interface GpuLoaded extends Loaded {
  /** `frame`, with milliseconds per stage and what was drawn. */
  frameTimed(view: View): Promise<{ readonly pixels: Uint8Array; readonly ms: FrameTimings; readonly drawn: FrameStats }>;
  /**
   * Browser path: draw the same frame into `target` (for example `context.getCurrentTexture()`), which must be
   * `COLOR_FORMAT` and exactly `view.width` x `view.height`. Resolves once validation of the submit is known.
   */
  draw(view: View, target: GPUTexture): Promise<FrameStats>;
  /**
   * GPU time per draw and per fixed stage, the minimum over `frames` frames of the same view (src/host/profile.ts).
   * Needs a device created with the "timestamp-query" feature; throws without it.
   */
  profile(view: View, frames: number): Promise<Profile>;
  /**
   * GPU milliseconds of the pose stages alone for this view: `repeats` runs of every stage in one timed compute pass,
   * divided by `repeats` (so the timestamp quantum and the pass's own cost divide out), the least of `rounds` rounds.
   */
  poseTime(view: View, repeats: number, rounds: number): Promise<{ readonly total: number; readonly stages: readonly number[] }>;
  /** Every dynamic buffer as the pose stages (and `cpu` uploads) left it for this view: for checks against `pose`. */
  readTables(view: View): Promise<Readonly<Record<string, Float32Array>>>;
}
export interface GpuHost extends Host {
  load(artefact: Artefact): Promise<GpuLoaded>;
}

const FLOATS: Record<NumTy, number> = { f32: 1, vec2: 2, vec3: 3, vec4: 4 };
/** Frame uniform: two mat4 then four vec4 (PRELUDE_WGSL `struct Frame`). */
const FRAME_FLOATS = 16 + 16 + 4 * 4;
/** WebGPU rejects zero-sized bindings; an artefact with no inputs or params still binds a buffer of one vec4. */
const MIN_BUFFER_BYTES = 16;
const VISIBILITY = 0x1 | 0x2; // GPUShaderStage.VERTEX | GPUShaderStage.FRAGMENT (literal: the globals are absent before bun-webgpu's setupGlobals)
const FRAGMENT = 0x2;
const COMPUTE = 0x4;

// ---- load-time canonicalisation -------------------------------------------------------------------------------

/** Every structural check on an artefact happens here, once; the per-frame path trusts what passes. */
function checkArtefact(a: Artefact): { paramFloats: number } {
  const fail = (msg: string): never => { throw new Error(`artefact: ${msg}`); };
  if (a.version !== 1) fail(`unsupported version ${a.version}`);
  for (const s of a.inputs) {
    if (s.def.length !== FLOATS[s.ty]) fail(`input "${s.name}" is ${s.ty} but its default has ${s.def.length} floats`);
    if (s.slot < 0 || s.slot + FLOATS[s.ty] > a.inputFloats) fail(`input "${s.name}" at slot ${s.slot} overruns inputFloats = ${a.inputFloats}`);
  }
  const paramFloats = a.params.reduce((n, p) => Math.max(n, p.slot + 1), 0);
  const seen = new Set<number>();
  for (const p of a.params) {
    if (seen.has(p.slot)) fail(`param "${p.name}" reuses slot ${p.slot}`);
    seen.add(p.slot);
  }
  if (a.curves.length % (CURVE_N * 4) !== 0) fail(`curves has ${a.curves.length} floats, not a multiple of CURVE_N * 4 = ${CURVE_N * 4}`);
  const names = new Set<string>(), bindings = new Set<number>();
  for (const d of a.dynamic) {
    if (names.has(d.name)) fail(`dynamic buffer "${d.name}" declared twice`);
    if (bindings.has(d.binding)) fail(`dynamic buffer "${d.name}" reuses binding ${d.binding}`);
    if (!(d.vec4s > 0)) fail(`dynamic buffer "${d.name}" has ${d.vec4s} vec4s`);
    names.add(d.name); bindings.add(d.binding);
  }
  for (const d of a.statics ?? []) {
    if (names.has(d.name)) fail(`static buffer "${d.name}" declared twice`);
    if (bindings.has(d.binding)) fail(`static buffer "${d.name}" reuses binding ${d.binding}`);
    if (!(d.data.length > 0 && d.data.length % 4 === 0)) fail(`static buffer "${d.name}" has ${d.data.length} floats (a positive multiple of 4)`);
    names.add(d.name); bindings.add(d.binding);
  }
  for (const d of a.draws) {
    if (!(d.shader in a.shaders)) fail(`draw "${d.label}" names unknown shader "${d.shader}"`);
    // Opaque draws are what scene depth is made of; one cannot read it while it is being written.
    if (d.sceneDepth === true && d.pass === "opaque") fail(`draw "${d.label}" sets sceneDepth but is opaque; only transparent draws can read the opaque scene's depth`);
  }
  const bySlot = new Map<number, string>();
  for (const d of a.draws) {
    const detail = d.detail;
    switch (detail.kind) {
      case "fixed": break;
      case "grid": {
        const buf = a.dynamic.find((b) => b.name === detail.bound.buffer);
        if (!buf) fail(`draw "${d.label}" reads its bounds from "${detail.bound.buffer}", which is not a dynamic buffer`);
        if (detail.bound.offset + d.instances > buf!.vec4s) fail(`draw "${d.label}": its bounds overrun "${detail.bound.buffer}"`);
        if (detail.levels.length === 0 || detail.levels[0]!.vertices !== d.vertices) fail(`draw "${d.label}": its finest level must have the draw's ${d.vertices} vertices`);
        if (detail.slot + d.instances > DETAIL_SLOTS) fail(`draw "${d.label}": slots ${detail.slot}..${detail.slot + d.instances} exceed ${DETAIL_SLOTS}`);
        break;
      }
      case "projected":
        if (detail.slot + 2 > DETAIL_SLOTS) fail(`draw "${d.label}": slot ${detail.slot} exceeds ${DETAIL_SLOTS}`);
        break;
    }
    // A slot is one choice: draws sharing it (a surface and its ink) must describe the same detail.
    if (detail.kind !== "fixed") {
      const key = JSON.stringify(detail), seen = bySlot.get(detail.slot);
      if (seen !== undefined && seen !== key) fail(`draw "${d.label}" shares level-of-detail slot ${detail.slot} with a draw whose detail differs`);
      bySlot.set(detail.slot, key);
    }
  }
  for (const p of a.post ?? []) if (!(p.shader in a.shaders)) fail(`post pass "${p.label}" names unknown shader "${p.shader}"`);
  for (const st of a.stages) if (!(Number.isInteger(st.invocations) && st.invocations >= 1)) fail(`pose stage "${st.label}" has ${st.invocations} invocations`);
  const sizes = new Map(a.dynamic.map((d) => [d.name, d.vec4s]));
  for (const t of a.tables) {
    const size = sizes.get(t.buffer);
    if (size === undefined) fail(`table "${t.name}" lives in "${t.buffer}", which is not a dynamic buffer`);
    if (t.offset < 0 || t.offset + t.vec4s > size!) fail(`table "${t.name}" overruns "${t.buffer}"`);
  }
  return { paramFloats };
}

// ---- pipelines -------------------------------------------------------------------------------------------------

const BLEND: Record<Draw["blend"], GPUBlendState | undefined> = {
  none: undefined,
  // Straight alpha over.
  alpha: { color: { srcFactor: "src-alpha", dstFactor: "one-minus-src-alpha" }, alpha: { srcFactor: "one", dstFactor: "one-minus-src-alpha" } },
  // Additive, scaled by the fragment's alpha (straight alpha, like `alpha`); the target's alpha is left as it is.
  add: { color: { srcFactor: "src-alpha", dstFactor: "one" }, alpha: { srcFactor: "zero", dstFactor: "one" } },
};

/** Which host WGSL a shader module is compiled with. A shader used in two ways is compiled once per way. */
type Variant = "draw" | "sceneDepth" | "post";
const VARIANT: Record<Variant, { readonly wgsl: string; readonly named: string }> = {
  draw: { wgsl: "", named: "" },
  sceneDepth: { wgsl: SCENE_DEPTH_WGSL, named: " (with SCENE_DEPTH_WGSL)" },
  post: { wgsl: POST_WGSL, named: " (as a post pass, with POST_WGSL)" },
};
const drawVariant = (d: Draw): Variant => (d.sceneDepth === true ? "sceneDepth" : "draw");
const moduleKey = (shader: string, v: Variant) => `${shader}|${v}`;

function pipelineKey(d: Draw): string {
  const constants = Object.keys(d.constants).sort().map((k) => `${k}=${d.constants[k]}`).join(",");
  return `${d.shader}|${drawVariant(d)}|${d.cull}|${d.blend}|${d.depthWrite}|${constants}`;
}

/** `what` names the module in the error, for example `shader "fin"`. */
async function compileShader(device: GPUDevice, what: string, code: string): Promise<GPUShaderModule> {
  device.pushErrorScope("validation");
  const module = device.createShaderModule({ label: what, code });
  const error = await device.popErrorScope();
  if (error) throw new Error(`${what} failed to compile:\n${error.message}`);
  return module;
}

/** A pose stage's compute pipeline, inside a validation scope. */
async function createPosePipeline(device: GPUDevice, stage: PoseStage, layout: GPUPipelineLayout): Promise<GPUComputePipeline> {
  const module = await compileShader(device, `pose stage "${stage.label}"`, PRELUDE_WGSL + stage.wgsl);
  device.pushErrorScope("validation");
  const result = await device.createComputePipelineAsync({ label: stage.label, layout, compute: { module, entryPoint: "pose_main" } })
    .then((p) => ({ ok: true as const, p }), (e: unknown) => ({ ok: false as const, message: e instanceof Error ? e.message : String(e) }));
  const error = await device.popErrorScope();
  const message = result.ok ? error?.message : result.message;
  if (message !== undefined || !result.ok) throw new Error(`pose stage "${stage.label}": pipeline creation failed:\n${message}`);
  return result.p;
}

/** The profile's label for every pose stage run in one pass. */
export const POSE_ONE_PASS = "pose stages, one pass";

/** A pose stage ready to dispatch. */
export type PoseRun = { readonly label: string; readonly pipeline: GPUComputePipeline; readonly workgroups: number };

/** Creates a pipeline inside a validation scope; `what` names it in the error. */
async function createPipeline(device: GPUDevice, what: string, descriptor: GPURenderPipelineDescriptor): Promise<GPURenderPipeline> {
  device.pushErrorScope("validation");
  const result = await device.createRenderPipelineAsync(descriptor)
    .then((p) => ({ ok: true as const, p }), (e: unknown) => ({ ok: false as const, message: e instanceof Error ? e.message : String(e) }));
  const error = await device.popErrorScope();
  const message = result.ok ? error?.message : result.message;
  if (message !== undefined || !result.ok) throw new Error(`${what}: pipeline creation failed:\n${message}`);
  return result.p;
}

function drawPipeline(layout: GPUPipelineLayout, module: GPUShaderModule, d: Draw): GPURenderPipelineDescriptor {
  const constants = { ...d.constants };
  const blend = BLEND[d.blend];
  return {
    label: `${d.shader} (${d.label})`,
    layout,
    vertex: { module, entryPoint: "vs", constants },
    fragment: { module, entryPoint: "fs", constants, targets: [blend ? { format: COLOR_FORMAT, blend } : { format: COLOR_FORMAT }] },
    primitive: { topology: "triangle-list", frontFace: "ccw", cullMode: d.cull },
    depthStencil: { format: DEPTH_FORMAT, depthWriteEnabled: d.depthWrite, depthCompare: "less" },
    multisample: { count: SAMPLES },
  };
}

/** A full-screen pass into one single-sampled target, no depth: post passes (POST_WGSL's `post_vs`) and the depth resolve. */
function fullScreenPipeline(label: string, layout: GPUPipelineLayout, module: GPUShaderModule, vs: string, format: GPUTextureFormat): GPURenderPipelineDescriptor {
  return {
    label, layout,
    vertex: { module, entryPoint: vs },
    fragment: { module, entryPoint: "fs", targets: [{ format }] },
    primitive: { topology: "triangle-list", cullMode: "none" },
  };
}

/** The single-sampled opaque depth that SCENE_DEPTH_WGSL reads. */
export const SCENE_DEPTH_FORMAT: GPUTextureFormat = "r32float";

// The nearest of each pixel's samples (SCENE_DEPTH_WGSL's comment says why the minimum and not the mean).
const DEPTH_RESOLVE_WGSL = /* wgsl */ `
@group(0) @binding(0) var depth_x4: texture_depth_multisampled_2d;
@vertex fn vs(@builtin(vertex_index) vi: u32) -> @builtin(position) vec4<f32> {
  let uv = vec2<f32>(f32((vi << 1u) & 2u), f32(vi & 2u));
  return vec4<f32>(uv.x * 2.0 - 1.0, 1.0 - uv.y * 2.0, 0.0, 1.0);
}
@fragment fn fs(@builtin(position) p: vec4<f32>) -> @location(0) vec4<f32> {
  let c = vec2<i32>(floor(p.xy));
  var d = textureLoad(depth_x4, c, 0);
  for (var i = 1; i < ${SAMPLES}; i++) { d = min(d, textureLoad(depth_x4, c, i)); }
  return vec4<f32>(d, 0.0, 0.0, 1.0);
}
`;

// ---- per-frame state -------------------------------------------------------------------------------------------

type Slots = {
  readonly inputs: ReadonlyMap<string, { readonly slot: number; readonly floats: number }>;
  readonly params: ReadonlyMap<string, number>;
};

function asFloats(name: string, value: number | readonly number[], floats: number): readonly number[] {
  const v = typeof value === "number" ? [value] : value;
  if (v.length !== floats) throw new Error(`view input "${name}": expected ${floats} floats, got ${v.length}`);
  return v;
}

/** Defaults from the artefact, overridden by the view. Unknown names throw: a typo must not silently do nothing. */
function frameState(a: Artefact, slots: Slots, paramFloats: number, curves: Float32Array, view: View): FrameState {
  const inputs = new Float32Array(a.inputFloats);
  for (const s of a.inputs) inputs.set(s.def, s.slot);
  for (const [name, value] of Object.entries(view.inputs ?? {})) {
    if (name === "time") throw new Error(`view input "time": set View.time instead`);
    const s = slots.inputs.get(name);
    if (!s) throw new Error(`view input "${name}": the artefact declares no such input`);
    inputs.set(asFloats(name, value, s.floats), s.slot);
  }
  const time = slots.inputs.get("time");
  if (time) inputs[time.slot] = view.time;
  const params = new Float32Array(paramFloats);
  for (const p of a.params) params[p.slot] = p.def;
  for (const [name, value] of Object.entries(view.params ?? {})) {
    const slot = slots.params.get(name);
    if (slot === undefined) throw new Error(`view param "${name}": the artefact declares no such param`);
    params[slot] = value;
  }
  return { time: view.time, inputs, params, curves };
}

/** The `FrameState` a host hands the pose pass for a view: the same mapping `frame` and `draw` use. */
export function viewState(a: Artefact, view: View, curves: Float32Array = a.curves): FrameState {
  return frameState(a, slotsOf(a), checkArtefact(a).paramFloats, curves, view);
}

function slotsOf(a: Artefact): Slots {
  return {
    inputs: new Map(a.inputs.map((s) => [s.name, { slot: s.slot, floats: FLOATS[s.ty] }])),
    params: new Map(a.params.map((p) => [p.name, p.slot])),
  };
}

export function frameUniform(view: View): Float32Array<ArrayBuffer> {
  const m = cameraMatrices(view.camera, view.width / view.height);
  const u = new Float32Array(FRAME_FLOATS);
  u.set(m.viewProj, 0);
  u.set(m.invViewProj, 16);
  u.set([...m.eye, m.perspective], 32);
  u.set([...m.forward, 0], 36);
  u.set([view.width, view.height, 1 / view.width, 1 / view.height], 40);
  u.set([view.time, 0, 0, 0], 44);
  return u;
}

/**
 * Everything sized to the view, for one schedule: the textures (destroyed on resize) and the recording of the passes
 * into an output view. Built by the schedule's handler, so the per-frame path never asks which schedule it is in.
 */
type Targets = {
  readonly width: number;
  readonly height: number;
  readonly textures: readonly GPUTexture[];
  readonly record: (encoder: GPUCommandEncoder, out: GPUTextureView, plan: Plan) => void;
};
/** The headless output texture and its readback buffer. */
type Readback = { readonly width: number; readonly height: number; readonly texture: GPUTexture; readonly buffer: GPUBuffer; readonly bytesPerRow: number };

/** Decided once at load from the artefact (see the header). */
export type Schedule =
  | { readonly kind: "plain" }
  | {
    readonly kind: "split";
    readonly resolve: GPURenderPipeline;
    readonly resolveLayout: GPUBindGroupLayout;
    readonly depthLayout: GPUBindGroupLayout;
    readonly postLayout: GPUBindGroupLayout;
    readonly sampler: GPUSampler;
    readonly post: readonly GPURenderPipeline[];
  };

export type Drawn = { readonly draw: Draw; readonly pipeline: GPURenderPipeline; readonly index: number };
export type { Plan, Run } from "./detail.ts";
/** What every schedule's recording shares: the artefact's clear colour, its draws and bind groups 0 and 1. */
export type Scene = {
  readonly clear: GPUColor;
  readonly opaque: readonly Drawn[];
  readonly transparent: readonly Drawn[];
  readonly group0: GPUBindGroup;
  readonly group1: GPUBindGroup;
  /** The pose stages and their bind group 1 (the dynamic buffers writable). */
  readonly pose: readonly PoseRun[];
  readonly poseGroup1: GPUBindGroup;
};

/**
 * The pose stages into `encoder`: one compute pass (timed by `whole` when given), or one pass per stage when `timed`
 * gives each its timestamps.
 * Dispatches within a pass see the writes of earlier ones (each is its own usage scope).
 */
export function recordPose(encoder: GPUCommandEncoder, scene: Scene, timed?: (label: string) => GPUComputePassTimestampWrites, whole?: GPUComputePassTimestampWrites): void {
  const dispatch = (pass: GPUComputePassEncoder, r: PoseRun): void => {
    pass.setPipeline(r.pipeline);
    pass.setBindGroup(0, scene.group0);
    pass.setBindGroup(1, scene.poseGroup1);
    pass.dispatchWorkgroups(r.workgroups);
  };
  if (timed === undefined) {
    if (scene.pose.length === 0) return;
    const pass = encoder.beginComputePass({ label: "loft pose", ...(whole === undefined ? {} : { timestampWrites: whole }) });
    for (const r of scene.pose) dispatch(pass, r);
    pass.end();
    return;
  }
  for (const r of scene.pose) {
    const pass = encoder.beginComputePass({ label: r.label, timestampWrites: timed(r.label) });
    dispatch(pass, r);
    pass.end();
  }
}

export function drawOne(pass: GPURenderPassEncoder, d: Drawn, plan: Plan): void {
  const runs = plan[d.index]!;
  if (runs.length === 0) return;
  pass.setPipeline(d.pipeline);
  for (const r of runs) pass.draw(r.vertices, r.instances, 0, r.first);
}
function drawAll(pass: GPURenderPassEncoder, list: readonly Drawn[], plan: Plan): void {
  for (const d of list) drawOne(pass, d, plan);
}


/** What a plan draws (FrameStats): scene triangles, and the draw calls issued. */
function statsOf(plan: Plan): FrameStats {
  let triangles = 0, draws = 0;
  for (const runs of plan) for (const r of runs) { triangles += (r.vertices / 3) * r.instances; draws++; }
  return { triangles: Math.round(triangles), draws };
}

/** plain: one 4x pass, opaque then transparent, resolved into the output. */
function plainTargets(device: GPUDevice, scene: Scene, size: { width: number; height: number }): Targets {
  const colour = device.createTexture({ label: "loft colour x4", size, format: COLOR_FORMAT, sampleCount: SAMPLES, usage: GPUTextureUsage.RENDER_ATTACHMENT });
  const depth = device.createTexture({ label: "loft depth x4", size, format: DEPTH_FORMAT, sampleCount: SAMPLES, usage: GPUTextureUsage.RENDER_ATTACHMENT });
  const colourView = colour.createView(), depthView = depth.createView();
  return {
    ...size,
    textures: [colour, depth],
    record: (encoder, out, plan) => {
      const pass = encoder.beginRenderPass({
        label: "loft scene",
        colorAttachments: [{ view: colourView, resolveTarget: out, loadOp: "clear", storeOp: "discard", clearValue: scene.clear }],
        depthStencilAttachment: { view: depthView, depthClearValue: 1, depthLoadOp: "clear", depthStoreOp: "discard" },
      });
      pass.setBindGroup(0, scene.group0);
      pass.setBindGroup(1, scene.group1);
      drawAll(pass, [...scene.opaque, ...scene.transparent], plan);
      pass.end();
    },
  };
}

/** split: opaque pass, depth resolve, transparent pass, post passes (see the header). */
function splitTargets(device: GPUDevice, scene: Scene, s: Extract<Schedule, { kind: "split" }>, size: { width: number; height: number }): Targets {
  const colour = device.createTexture({ label: "loft colour x4", size, format: COLOR_FORMAT, sampleCount: SAMPLES, usage: GPUTextureUsage.RENDER_ATTACHMENT });
  const depth = device.createTexture({ label: "loft depth x4", size, format: DEPTH_FORMAT, sampleCount: SAMPLES, usage: GPUTextureUsage.RENDER_ATTACHMENT | GPUTextureUsage.TEXTURE_BINDING });
  const sceneDepth = device.createTexture({ label: "loft scene depth", size, format: SCENE_DEPTH_FORMAT, usage: GPUTextureUsage.RENDER_ATTACHMENT | GPUTextureUsage.TEXTURE_BINDING });
  const colourView = colour.createView(), depthView = depth.createView(), sceneDepthView = sceneDepth.createView();
  const resolveGroup = device.createBindGroup({ label: "loft depth resolve", layout: s.resolveLayout, entries: [{ binding: 0, resource: depthView }] });
  const depthGroup = device.createBindGroup({ label: "loft scene depth", layout: s.depthLayout, entries: [{ binding: 0, resource: sceneDepthView }] });
  // Ping-pong: pass k reads intermediate (k - 1) % 2 and writes k % 2; one post pass needs one intermediate.
  const n = s.post.length;
  const intermediates = Array.from({ length: Math.min(n, 2) }, (_, i) => {
    const texture = device.createTexture({ label: `loft post ${i}`, size, format: COLOR_FORMAT, usage: GPUTextureUsage.RENDER_ATTACHMENT | GPUTextureUsage.TEXTURE_BINDING });
    const view = texture.createView();
    const group = device.createBindGroup({
      label: `loft post input ${i}`, layout: s.postLayout,
      entries: [{ binding: 0, resource: sceneDepthView }, { binding: 1, resource: view }, { binding: 2, resource: s.sampler }],
    });
    return { texture, view, group };
  });
  /** Stage 0 is the scene; stage k >= 1 is post pass k - 1. The last stage writes the output. */
  const outputOf = (k: number, out: GPUTextureView) => (k === n ? out : intermediates[k % 2]!.view);
  return {
    ...size,
    textures: [colour, depth, sceneDepth, ...intermediates.map((i) => i.texture)],
    record: (encoder, out, plan) => {
      const opaque = encoder.beginRenderPass({
        label: "loft scene opaque",
        colorAttachments: [{ view: colourView, loadOp: "clear", storeOp: "store", clearValue: scene.clear }],
        depthStencilAttachment: { view: depthView, depthClearValue: 1, depthLoadOp: "clear", depthStoreOp: "store" },
      });
      opaque.setBindGroup(0, scene.group0);
      opaque.setBindGroup(1, scene.group1);
      drawAll(opaque, scene.opaque, plan);
      opaque.end();

      const resolve = encoder.beginRenderPass({
        label: "loft depth resolve",
        colorAttachments: [{ view: sceneDepthView, loadOp: "clear", storeOp: "store", clearValue: { r: 1, g: 0, b: 0, a: 1 } }],
      });
      resolve.setPipeline(s.resolve);
      resolve.setBindGroup(0, resolveGroup);
      resolve.draw(3);
      resolve.end();

      const transparent = encoder.beginRenderPass({
        label: "loft scene transparent",
        colorAttachments: [{ view: colourView, resolveTarget: outputOf(0, out), loadOp: "load", storeOp: "discard" }],
        depthStencilAttachment: { view: depthView, depthLoadOp: "load", depthStoreOp: "discard" },
      });
      transparent.setBindGroup(0, scene.group0);
      transparent.setBindGroup(1, scene.group1);
      transparent.setBindGroup(2, depthGroup); // ignored by pipelines whose layout stops at group 1
      drawAll(transparent, scene.transparent, plan);
      transparent.end();

      s.post.forEach((pipeline, i) => {
        const k = i + 1;
        const pass = encoder.beginRenderPass({
          label: `loft post ${i}`,
          colorAttachments: [{ view: outputOf(k, out), loadOp: "clear", storeOp: "store", clearValue: { r: 0, g: 0, b: 0, a: 1 } }],
        });
        pass.setPipeline(pipeline);
        pass.setBindGroup(0, scene.group0);
        pass.setBindGroup(1, scene.group1);
        pass.setBindGroup(2, intermediates[(k - 1) % 2]!.group);
        pass.draw(3);
        pass.end();
      });
    },
  };
}

/** The split schedule's own pipelines: the depth resolve and one per post pass. */
async function splitPipelines(device: GPUDevice, postLayout: GPUPipelineLayout, post: readonly { pass: PostPass; module: GPUShaderModule }[]) {
  const resolveLayout = device.createBindGroupLayout({
    label: "loft depth resolve",
    entries: [{ binding: 0, visibility: FRAGMENT, texture: { sampleType: "depth", viewDimension: "2d", multisampled: true } }],
  });
  const resolveModule = await compileShader(device, "loft depth resolve", DEPTH_RESOLVE_WGSL);
  const resolve = await createPipeline(device, "loft depth resolve",
    fullScreenPipeline("loft depth resolve", device.createPipelineLayout({ bindGroupLayouts: [resolveLayout] }), resolveModule, "vs", SCENE_DEPTH_FORMAT));
  const pipelines: GPURenderPipeline[] = [];
  for (const { pass: p, module } of post) {
    pipelines.push(await createPipeline(device, `shader "${p.shader}" (post pass "${p.label}")`, fullScreenPipeline(`${p.shader} (${p.label})`, postLayout, module, "post_vs", COLOR_FORMAT)));
  }
  const sampler = device.createSampler({ label: "loft post", magFilter: "linear", minFilter: "linear", addressModeU: "clamp-to-edge", addressModeV: "clamp-to-edge" });
  return { resolve, resolveLayout, sampler, post: pipelines };
}

function makeTargets(device: GPUDevice, scene: Scene, schedule: Schedule, size: { width: number; height: number }): Targets {
  switch (schedule.kind) {
    case "plain": return plainTargets(device, scene, size);
    case "split": return splitTargets(device, scene, schedule, size);
  }
}

const sameSize = (t: { width: number; height: number }, v: View) => t.width === v.width && t.height === v.height;

// ---- the host --------------------------------------------------------------------------------------------------

export function createHost(device: GPUDevice): GpuHost {
  return { load: (artefact) => load(device, artefact) };
}

async function load(device: GPUDevice, a: Artefact): Promise<GpuLoaded> {
  const { paramFloats } = checkArtefact(a);
  const slots = slotsOf(a);
  const post = a.post ?? []; // absent means none (artefact.ts)
  const STORAGE = GPUBufferUsage.STORAGE | GPUBufferUsage.COPY_DST;
  const buffer = (label: string, bytes: number, usage: number) => device.createBuffer({ label, size: Math.max(MIN_BUFFER_BYTES, bytes), usage });

  const frameBuf = buffer("frame", FRAME_FLOATS * 4, GPUBufferUsage.UNIFORM | GPUBufferUsage.COPY_DST);
  const inputBuf = buffer("inputs", a.inputFloats * 4, STORAGE);
  const paramBuf = buffer("params", paramFloats * 4, STORAGE);
  const curveBuf = buffer("curves", a.curves.byteLength, STORAGE);
  const detailBuf = buffer("loft_detail", DETAIL_SLOTS * 16, GPUBufferUsage.UNIFORM | GPUBufferUsage.COPY_DST);
  const detail = new Float32Array(DETAIL_SLOTS * 4);
  // Only the slots the artefact uses are uploaded each frame (the rest of the 64 KiB array is never read).
  const detailFloats = Math.max(4, slotsUsed(a) * 4);
  // Each grid's level last frame, for hysteresis (src/host/detail.ts): the host's only memory between frames.
  const memory: Memory = new Map();
  if (a.curves.length > 0) device.queue.writeBuffer(curveBuf, 0, a.curves as Float32Array<ArrayBuffer>); // Dawn warns on empty writes
  // Dynamic buffers are also copied from: the level-of-detail bounds and `readTables` read them back.
  const dynamic = a.dynamic.map((d) => ({ ...d, gpu: buffer(d.name, d.vec4s * 16, STORAGE | GPUBufferUsage.COPY_SRC) }));
  // Static tables: written once, here (absent means none).
  const statics = (a.statics ?? []).map((d) => ({ binding: d.binding, gpu: buffer(d.name, d.data.byteLength, STORAGE) }));
  (a.statics ?? []).forEach((d, k) => device.queue.writeBuffer(statics[k]!.gpu, 0, d.data as Float32Array<ArrayBuffer>));
  const group1Buffers = [...dynamic, ...statics];

  // Explicit layouts, every binding visible to both stages (artefact.ts header): a shader may use any subset, and
  // `layout: "auto"` would drop the bindings a particular shader never reads.
  const storageEntry = (binding: number, visibility = VISIBILITY): GPUBindGroupLayoutEntry => ({ binding, visibility, buffer: { type: "read-only-storage" } });
  // Group 0 is also bound to the pose stages.
  const all = VISIBILITY | COMPUTE;
  const layout0 = device.createBindGroupLayout({
    label: "loft prelude",
    entries: [{ binding: 0, visibility: all, buffer: { type: "uniform" } }, storageEntry(1, all), storageEntry(2, all), storageEntry(3, all), { binding: 4, visibility: all, buffer: { type: "uniform" } }],
  });
  const layout1 = device.createBindGroupLayout({ label: "loft scene tables", entries: group1Buffers.map((d) => storageEntry(d.binding)) });
  // The pose stages' group 1: the dynamic buffers writable, the static ones read-only (PoseStage).
  const poseLayout1 = device.createBindGroupLayout({
    label: "loft pose tables",
    entries: [...dynamic.map((d): GPUBindGroupLayoutEntry => ({ binding: d.binding, visibility: COMPUTE, buffer: { type: "storage" } })), ...statics.map((d) => storageEntry(d.binding, COMPUTE))],
  });
  const poseGroup1 = device.createBindGroup({ label: "loft pose tables", layout: poseLayout1, entries: group1Buffers.map((d) => ({ binding: d.binding, resource: { buffer: d.gpu } })) });
  const poseLayout = device.createPipelineLayout({ bindGroupLayouts: [layout0, poseLayout1] });
  const poseRuns: PoseRun[] = [];
  for (const st of a.stages) poseRuns.push({ label: st.label, pipeline: await createPosePipeline(device, st, poseLayout), workgroups: Math.ceil(st.invocations / POSE_WORKGROUP) });
  const group0 = device.createBindGroup({
    layout: layout0,
    entries: [frameBuf, inputBuf, paramBuf, curveBuf, detailBuf].map((b, binding) => ({ binding, resource: { buffer: b } })),
  });
  const group1 = device.createBindGroup({ layout: layout1, entries: group1Buffers.map((d) => ({ binding: d.binding, resource: { buffer: d.gpu } })) });
  // Group 2 layouts: exactly the ones documented on SCENE_DEPTH_WGSL and POST_WGSL.
  const sceneDepthEntry: GPUBindGroupLayoutEntry = { binding: 0, visibility: VISIBILITY, texture: { sampleType: "unfilterable-float", viewDimension: "2d" } };
  const depthLayout = device.createBindGroupLayout({ label: "loft scene depth", entries: [sceneDepthEntry] });
  const postLayout = device.createBindGroupLayout({
    label: "loft post",
    entries: [
      sceneDepthEntry,
      { binding: 1, visibility: VISIBILITY, texture: { sampleType: "float", viewDimension: "2d" } },
      { binding: 2, visibility: VISIBILITY, sampler: { type: "filtering" } },
    ],
  });
  const layouts: Record<Variant, GPUPipelineLayout> = {
    draw: device.createPipelineLayout({ bindGroupLayouts: [layout0, layout1] }),
    sceneDepth: device.createPipelineLayout({ bindGroupLayouts: [layout0, layout1, depthLayout] }),
    post: device.createPipelineLayout({ bindGroupLayouts: [layout0, layout1, postLayout] }),
  };

  // One module per (shader, way it is used). A shader nothing uses is not compiled: which host WGSL it expects
  // depends on its use (a post shader does not compile as a draw), so there is nothing sound to check it against.
  const uses = new Map<string, { shader: string; variant: Variant }>();
  const use = (shader: string, variant: Variant) => uses.set(moduleKey(shader, variant), { shader, variant });
  for (const d of a.draws) use(d.shader, drawVariant(d));
  for (const p of post) use(p.shader, "post");
  const modules = new Map<string, GPUShaderModule>();
  for (const [key, { shader, variant }] of uses) {
    modules.set(key, await compileShader(device, `shader "${shader}"${VARIANT[variant].named}`, PRELUDE_WGSL + VARIANT[variant].wgsl + a.sceneWgsl + a.shaders[shader]!));
  }
  // Pipelines are built eagerly, so every error surfaces at load and a frame never stalls on compilation.
  const pipelines = new Map<string, GPURenderPipeline>();
  for (const d of a.draws) {
    const key = pipelineKey(d);
    const v = drawVariant(d);
    if (!pipelines.has(key)) pipelines.set(key, await createPipeline(device, `shader "${d.shader}" (draw "${d.label}")`, drawPipeline(layouts[v], modules.get(moduleKey(d.shader, v))!, d)));
  }
  const drawn = (pass: Draw["pass"]): Drawn[] => a.draws.flatMap((d, index) => (d.pass === pass ? [{ draw: d, pipeline: pipelines.get(pipelineKey(d))!, index }] : []));
  const scene: Scene = { clear: { r: a.clear[0], g: a.clear[1], b: a.clear[2], a: 1 }, opaque: drawn("opaque"), transparent: drawn("transparent"), group0, group1, pose: poseRuns, poseGroup1 };

  const split = post.length > 0 || a.draws.some((d) => d.sceneDepth === true);
  const postModules = post.map((p) => ({ pass: p, module: modules.get(moduleKey(p.shader, "post"))! }));
  const schedule: Schedule = split
    ? { kind: "split", depthLayout, postLayout, ...await splitPipelines(device, layouts.post, postModules) }
    : { kind: "plain" };
  const gpuOf = new Map(dynamic.map((d) => [d.name, d.gpu]));
  // The `cpu` tables, by place: each frame's uploads must be exactly these, each its table's size.
  const cpuTables = new Map(a.tables.filter((t) => t.fill.kind === "cpu").map((t) => [`${t.buffer}@${t.offset}`, t]));

  let targets: Targets | undefined;
  let readback: Readback | undefined;
  let reading = false;

  const targetsFor = (view: View): Targets => {
    if (targets && sameSize(targets, view)) return targets;
    for (const t of targets?.textures ?? []) t.destroy();
    targets = makeTargets(device, scene, schedule, { width: view.width, height: view.height });
    return targets;
  };
  const readbackFor = (view: View): Readback => {
    if (readback && sameSize(readback, view)) return readback;
    readback?.texture.destroy(); readback?.buffer.destroy();
    // Buffer copies of a texture need rows aligned to 256 bytes; `frame` repacks them tightly.
    const bytesPerRow = Math.ceil((view.width * 4) / 256) * 256;
    readback = {
      width: view.width, height: view.height, bytesPerRow,
      texture: device.createTexture({ label: "loft output", size: { width: view.width, height: view.height }, format: COLOR_FORMAT, usage: GPUTextureUsage.RENDER_ATTACHMENT | GPUTextureUsage.COPY_SRC }),
      buffer: device.createBuffer({ label: "loft readback", size: bytesPerRow * view.height, usage: GPUBufferUsage.MAP_READ | GPUBufferUsage.COPY_DST }),
    };
    return readback;
  };

  // Level-of-detail bounds: each grid's instances (anchor, scale), written by the pose stages, copied here each pose
  // and read back. Packed grid after grid; `at(slot)` is a view of one grid's.
  const grids = gridBounds(a);
  const boundsAt = new Map<number, number>();
  const boundVec4s = grids.reduce((n, g) => { boundsAt.set(g.slot, n); return n + g.instances; }, 0);
  const boundsBuf = device.createBuffer({ label: "loft bounds", size: Math.max(MIN_BUFFER_BYTES, boundVec4s * 16), usage: GPUBufferUsage.MAP_READ | GPUBufferUsage.COPY_DST });
  /** The read of `boundsBuf` in flight (interactive frames do not wait for it). */
  let boundsPending: Promise<Bounds> | undefined;
  /** What interactive frames plan with: the last bounds read. */
  let lastBounds: Bounds = { kind: "none" };
  const readBounds = async (): Promise<Bounds> => {
    await boundsBuf.mapAsync(GPUMapMode.READ);
    const data = new Float32Array(boundsBuf.getMappedRange().slice(0));
    boundsBuf.unmap();
    return { kind: "read", at: (slot) => { const o = boundsAt.get(slot)!; return data.subarray(o * 4); } };
  };

  // The curve data the GPU holds, so the CPU fills read the same curves (FrameState.curves).
  let curves = a.curves;
  /**
   * Write the frame's state and `cpu` tables, then submit the pose stages (and, when `copyBounds`, the copy of the
   * grids' bounds). Returns the CPU milliseconds spent.
   */
  const pose = (view: View, copyBounds: boolean): number => {
    const t0 = performance.now();
    const state = frameState(a, slots, paramFloats, curves, view);
    const uploads = a.cpuPose(state);
    for (const u of uploads) {
      const t = cpuTables.get(`${u.buffer}@${u.offset}`);
      if (t === undefined) throw new Error(`cpuPose returned data for "${u.buffer}" at vec4 ${u.offset}, where no cpu table lives`);
      if (u.data.length !== t.vec4s * 4) throw new Error(`cpuPose returned table "${t.name}" with ${u.data.length} floats; expected ${t.vec4s * 4} (${t.vec4s} vec4s)`);
      device.queue.writeBuffer(gpuOf.get(u.buffer)!, u.offset * 16, u.data as Float32Array<ArrayBuffer>);
    }
    if (uploads.length !== cpuTables.size) {
      const given = new Set(uploads.map((u) => `${u.buffer}@${u.offset}`));
      throw new Error(`cpuPose did not return cpu table${[...cpuTables].filter(([k]) => !given.has(k)).map(([, t]) => ` "${t.name}"`).join(",")}`);
    }
    device.queue.writeBuffer(frameBuf, 0, frameUniform(view));
    if (state.inputs.length > 0) device.queue.writeBuffer(inputBuf, 0, state.inputs as Float32Array<ArrayBuffer>);
    if (state.params.length > 0) device.queue.writeBuffer(paramBuf, 0, state.params as Float32Array<ArrayBuffer>);
    const encoder = device.createCommandEncoder({ label: "loft pose" });
    recordPose(encoder, scene);
    if (copyBounds) for (const g of grids) encoder.copyBufferToBuffer(gpuOf.get(g.buffer)!, g.offset * 16, boundsBuf, boundsAt.get(g.slot)! * 16, g.instances * 16);
    device.queue.submit([encoder.finish()]);
    return performance.now() - t0;
  };
  /** This frame's plan from these bounds, its level-of-detail slots written (before the draws' submit). */
  const plan = (view: View, bounds: Bounds): Plan => {
    const p = planFrame(a, view, bounds, memory, detail);
    device.queue.writeBuffer(detailBuf, 0, detail, 0, detailFloats);
    return p;
  };
  /** A frame that waits for its own bounds (headless frames, profiling): pose, read the bounds, plan. */
  const posedPlan = async (view: View): Promise<{ readonly plan: Plan; readonly poseMs: number }> => {
    if (boundsPending) await boundsPending;
    const copy = grids.length > 0;
    const poseMs = await validated("pose", () => pose(view, copy));
    const bounds = copy ? await readBounds() : lastBounds;
    const t0 = performance.now();
    const p = plan(view, bounds);
    return { plan: p, poseMs: poseMs + performance.now() - t0 };
  };

  /** The shared drawing code, for `frame` and `draw` alike: every pass of the schedule, ending in `out`. */
  const record = (encoder: GPUCommandEncoder, view: View, out: GPUTextureView, plan: Plan): void => {
    targetsFor(view).record(encoder, out, plan);
  };

  const checkView = (view: View): void => {
    if (!(Number.isInteger(view.width) && Number.isInteger(view.height) && view.width > 0 && view.height > 0)) {
      throw new Error(`view: size must be positive integers, got ${view.width} x ${view.height}`);
    }
  };

  /** Runs `body` inside a validation scope and throws the first validation error it raised. */
  const validated = async <T>(what: string, body: () => T): Promise<T> => {
    device.pushErrorScope("validation");
    let out: T;
    try { out = body(); } catch (e) { await device.popErrorScope(); throw e; }
    const error = await device.popErrorScope();
    if (error) throw new Error(`${what}: WebGPU validation error:\n${error.message}`);
    return out;
  };

  const frameTimed = async (view: View) => {
    // One readback buffer serves every frame; a second frame would remap or resize it under the first one's pending
    // mapAsync. So overlapping calls are rejected, and the rejected call leaves the pending one untouched.
    if (reading) throw new Error("frame: called again before the previous frame() resolved (its readback buffer is still being mapped); await each frame before starting the next");
    checkView(view);
    reading = true;
    try {
      const posed = await posedPlan(view);
      const t1 = performance.now();
      const rb = await validated("frame", () => {
        const rb = readbackFor(view);
        const encoder = device.createCommandEncoder({ label: "loft frame" });
        record(encoder, view, rb.texture.createView(), posed.plan);
        encoder.copyTextureToBuffer({ texture: rb.texture }, { buffer: rb.buffer, bytesPerRow: rb.bytesPerRow, rowsPerImage: view.height }, { width: view.width, height: view.height });
        device.queue.submit([encoder.finish()]);
        return rb;
      });
      const t2 = performance.now();
      await rb.buffer.mapAsync(GPUMapMode.READ);
      const mapped = new Uint8Array(rb.buffer.getMappedRange());
      const row = view.width * 4;
      const pixels = new Uint8Array(row * view.height);
      for (let y = 0; y < view.height; y++) pixels.set(mapped.subarray(y * rb.bytesPerRow, y * rb.bytesPerRow + row), y * row);
      rb.buffer.unmap();
      const t3 = performance.now();
      return { pixels, ms: { pose: posed.poseMs, encode: t2 - t1, readback: t3 - t2 }, drawn: statsOf(posed.plan) };
    } finally {
      reading = false;
    }
  };

  return {
    frameTimed,
    frame: async (view) => (await frameTimed(view)).pixels,
    draw: async (view, target) => {
      checkView(view);
      if (target.format !== COLOR_FORMAT) throw new Error(`draw: target format is ${target.format}; configure the canvas with ${COLOR_FORMAT}`);
      if (target.width !== view.width || target.height !== view.height) {
        throw new Error(`draw: target is ${target.width} x ${target.height} but the view is ${view.width} x ${view.height}`);
      }
      // Never waits on the GPU: plans with the last bounds read, and starts reading this frame's when none is in flight.
      const copy = grids.length > 0 && boundsPending === undefined;
      const stats = await validated("draw", () => {
        pose(view, copy);
        const p = plan(view, lastBounds);
        const encoder = device.createCommandEncoder({ label: "loft draw" });
        record(encoder, view, target.createView(), p);
        device.queue.submit([encoder.finish()]);
        return statsOf(p);
      });
      if (copy) boundsPending = readBounds().then((b) => { lastBounds = b; boundsPending = undefined; return b; });
      return stats;
    },
    profile: async (view, frames) => {
      checkView(view);
      if (!(Number.isInteger(frames) && frames >= 1)) throw new Error(`profile: frames must be a positive integer, got ${frames}`);
      return profileFrames({ device, scene, schedule, size: { width: view.width, height: view.height }, frames, upload: () => posedPlan(view) });
    },
    poseTime: async (view, repeats, rounds) => {
      checkView(view);
      if (!device.features.has("timestamp-query")) throw new Error("poseTime: the device was created without the \"timestamp-query\" feature");
      await posedPlan(view);
      const query = device.createQuerySet({ type: "timestamp", count: 2 });
      const resolve = device.createBuffer({ size: 16, usage: GPUBufferUsage.QUERY_RESOLVE | GPUBufferUsage.COPY_SRC });
      const read = device.createBuffer({ size: 16, usage: GPUBufferUsage.MAP_READ | GPUBufferUsage.COPY_DST });
      /** Least over the rounds of the per-repeat time of these runs, dispatched in order `repeats` times in one pass. */
      const time = async (runs: readonly PoseRun[]): Promise<number> => {
        let best = Infinity;
        for (let r = 0; r < rounds; r++) {
          const encoder = device.createCommandEncoder({ label: "loft pose time" });
          const pass = encoder.beginComputePass({ label: "pose x repeats", timestampWrites: { querySet: query, beginningOfPassWriteIndex: 0, endOfPassWriteIndex: 1 } });
          for (let k = 0; k < repeats; k++) {
            for (const run of runs) {
              pass.setPipeline(run.pipeline);
              pass.setBindGroup(0, scene.group0);
              pass.setBindGroup(1, scene.poseGroup1);
              pass.dispatchWorkgroups(run.workgroups);
            }
          }
          pass.end();
          encoder.resolveQuerySet(query, 0, 2, resolve, 0);
          encoder.copyBufferToBuffer(resolve, 0, read, 0, 16);
          device.queue.submit([encoder.finish()]);
          await read.mapAsync(GPUMapMode.READ);
          const ts = new BigUint64Array(read.getMappedRange().slice(0));
          read.unmap();
          best = Math.min(best, Number(ts[1]! - ts[0]!) / 1e6 / repeats);
        }
        return best;
      };
      // Warm up: an idle Apple GPU runs a short burst at a low clock (the same scene read 0.10 to 0.27 ms cold).
      for (let w = 0; w < 4; w++) await time(scene.pose);
      const total = await time(scene.pose);
      const stages: number[] = [];
      for (const run of scene.pose) stages.push(await time([run]));
      query.destroy(); resolve.destroy(); read.destroy();
      return { total, stages };
    },
    readTables: async (view) => {
      checkView(view);
      if (boundsPending) await boundsPending;
      await validated("readTables", () => pose(view, false));
      const staging = dynamic.map((d) => device.createBuffer({ label: `read ${d.name}`, size: d.vec4s * 16, usage: GPUBufferUsage.MAP_READ | GPUBufferUsage.COPY_DST }));
      const encoder = device.createCommandEncoder({ label: "loft read tables" });
      dynamic.forEach((d, k) => encoder.copyBufferToBuffer(d.gpu, 0, staging[k]!, 0, d.vec4s * 16));
      device.queue.submit([encoder.finish()]);
      const out: Record<string, Float32Array> = {};
      for (const [k, d] of dynamic.entries()) {
        await staging[k]!.mapAsync(GPUMapMode.READ);
        out[d.name] = new Float32Array(staging[k]!.getMappedRange().slice(0));
        staging[k]!.destroy();
      }
      return out;
    },
    setCurves: (next) => {
      if (next.length !== a.curves.length) throw new Error(`setCurves: got ${next.length} floats; the artefact has ${a.curves.length}`);
      if (next.length > 0) device.queue.writeBuffer(curveBuf, 0, next as Float32Array<ArrayBuffer>);
      // A copy: the caller may keep editing its array, and the pose must see what the GPU was given.
      curves = next.slice();
    },
    destroy: () => {
      for (const b of [frameBuf, inputBuf, paramBuf, curveBuf, detailBuf, boundsBuf, ...group1Buffers.map((d) => d.gpu)]) b.destroy();
      for (const t of targets?.textures ?? []) t.destroy();
      readback?.texture.destroy(); readback?.buffer.destroy();
      targets = undefined; readback = undefined;
    },
  };
}
