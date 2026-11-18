// The WebGPU host: an executor of a lowered artefact (src/host/manifest.ts, docs/artefact_format.md). It creates the
// objects the manifest lists and, per frame, runs its recipe: the frame writes, the `cpu` tables (in memory only), the
// pose pass, the level-of-detail bounds read and plan, the render passes. It decides nothing about the scene: the
// decisions were made by `lower` (src/host/lower.ts), from a compile or, earlier, by `loft build`.
//
// Two ways in, one executor: `createHost(device).load(artefact)` lowers a compiled artefact in memory;
// `loadBundle(device, bundle, { kind: "none" })` runs one read from files (src/host/files.ts), without the compiler.
//
// Frames: `frame` waits for this frame's bounds and renders into its own texture and reads it back; `draw` renders into
// the caller's texture (a canvas's current texture) with the last bounds read, never waiting on the GPU.
//
// Queue order: every `writeBuffer` of a frame is issued before the submit of the pose pass that reads it, and the
// level-of-detail slots are written before the submit of the draws (writes are ordered with submits, not with passes
// inside one encoder: a write between two passes of one encoder would land before both).
//
// This file imports no bun-only module, so a browser bundle of src/host (minus headless.ts and files.ts) stays clean.

import { EnvironmentError, SceneError } from "../errors.ts";
import { authorLineAt, LIGHT_SAMPLE_INPUT, replayClocks, replayStep, type ReplayClock, type Artefact, type CpuUpload, type FrameState, type FrameStats, type Host, type Loaded, type View } from "../artefact.ts";
import { bakeRecurrences, readBake, recordSteps } from "./bake.ts";
import { checkComputeData } from "./compute_data.ts";
import { cameraMatrices } from "./camera.ts";
import { planFrame, type Bounds, type Memory, type Plan } from "./detail.ts";
import { FRAME_FLOATS, lower } from "./lower.ts";
import { COLOR_FORMAT, COMPUTE_TEXEL_BYTES, type Bundle, type ComputeRun, type Dispatch, type RawDispatch, type TextureDesc, type FrameWrite, type FramePrecision, type Precision, type Manifest, type ModuleDesc, type RenderPassDesc, type Replay, type Target, type TextureSize } from "./manifest.ts";
import { selectPrecision } from "./precision.ts";
import { profileFrames, type Profile } from "./profile.ts";
import { stillFrame, type StillSpec } from "./still.ts";
export type { Profile, ProfileDraw, ProfileStage } from "./profile.ts";
export { COLOR_FORMAT, DEPTH_FORMAT, SAMPLES, SCENE_DEPTH_FORMAT } from "./manifest.ts";

export type FrameTimings = {
  /** CPU: frame state, the `cpu` tables, uploads, the pose pass's submit and the level-of-detail plan. */
  readonly pose: number;
  /** CPU: buffer uploads, command encoding and submit. */
  readonly encode: number;
  /** GPU execution plus the wait for the mapped readback and repacking rows. */
  readonly readback: number;
};

export interface GpuLoaded extends Loaded {
  /** The precision used for every frame, including a named f32 fallback. */
  readonly precision: FramePrecision;
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
  /**
   * GPU milliseconds of a normal frame (the one `frame` runs, unmodified), least of `frames`: its two submits, the pose
   * pass (timestamps at its start and end) and the render passes (at the start of the first, the end of the last), the
   * two spans summed. The gap between the
   * submits (the CPU waiting for the level-of-detail bounds and planning) is not GPU time and is not counted. `tick`:
   * the greatest common divisor of the timestamp differences seen: the timer's resolution as observed.
   */
  frameGpu(view: View, frames: number): Promise<{ readonly ms: number; readonly pose: number; readonly draws: number; readonly tick: number }>;
  /** Every dynamic buffer as the pose stages (and `cpu` uploads) left it for this view, by name: for checks against `pose`. */
  readTables(view: View): Promise<Readonly<Record<string, Float32Array>>>;
  /** A still of this view: many samples of the instant averaged (src/host/still.ts), read back as `frameTimed` reads a frame. */
  stillTimed(view: View, still: Extract<StillSpec, { kind: "still" }>): Promise<{ readonly pixels: Uint8Array; readonly ms: FrameTimings; readonly drawn: FrameStats }>;
}
/** Timestamp indices a timed frame writes at the start and end of a submit's passes (frame timing). */
type Stamps = { readonly querySet: GPUQuerySet; readonly begin: number; readonly end: number };

export interface GpuHost extends Host {
  load(artefact: Artefact): Promise<GpuLoaded>;
}

/**
 * The `cpu` tables' fills, which are JavaScript and so never in a file: `none` (from files; the bundle must then have
 * no `cpu` table), or the compiled artefact's `cpuPose` (in memory).
 */
export type CpuTables = { readonly kind: "none" } | { readonly kind: "js"; readonly uploads: (state: FrameState) => readonly CpuUpload[] };

/** A bundle names a `cpu` table but nothing can fill it: a file artefact cannot hold JavaScript. */
export class NotDataError extends SceneError {
  override readonly name = "NotDataError";
}

export function createHost(device: GPUDevice, options: { readonly precision?: Precision } = {}): GpuHost {
  // async: an artefact that fails lower's checks rejects the returned promise, as every load error does, never throws.
  // The first host to load an artefact bakes its recurrences (src/host/bake.ts); lower then writes the baked states.
  return {
    load: async (artefact) => {
      await bakeRecurrences(device, artefact);
      return loadBundle(device, lower(artefact, options.precision), { kind: "js", uploads: (state) => artefact.cpuPose(state) });
    },
  };
}

// ---- per-frame state -------------------------------------------------------------------------------------------

const FLOATS = { f32: 1, vec2: 2, vec3: 3, vec4: 4 } as const;
type StateSpec = Pick<Manifest, "inputs" | "inputFloats" | "params" | "paramFloats"> & { readonly frame?: { readonly replay?: readonly ReplayClock[] } };

function asFloats(name: string, value: number | readonly number[], floats: number): readonly number[] {
  const v = typeof value === "number" ? [value] : value;
  if (v.length !== floats) throw new Error(`view input "${name}": expected ${floats} floats, got ${v.length}`);
  return v;
}

/** Defaults, overridden by the view; the view's time fills the `time` input. Unknown names throw: a typo must not silently do nothing. */
function frameState(s: StateSpec, curves: Float32Array, view: View): FrameState {
  const inputs = new Float32Array(s.inputFloats);
  for (const i of s.inputs) inputs.set(i.def, i.slot);
  const replays = s.frame?.replay ?? [];
  for (const [name, value] of Object.entries(view.inputs ?? {})) {
    if (name === "time") throw new Error(`view input "time": set View.time instead`);
    if (replays.some((r) => s.inputs.find((x) => x.slot === r.slot)?.name === name)) throw new Error(`view input "${name}": a replay's step number, which the host sets from View.time`);
    const i = s.inputs.find((x) => x.name === name);
    if (!i) throw new Error(`view input "${name}": the artefact declares no such input`);
    inputs.set(asFloats(name, value, FLOATS[i.ty]), i.slot);
  }
  const time = s.inputs.find((x) => x.name === "time");
  if (time) inputs[time.slot] = view.time;
  // The step each replay replays to, from the time, so the GPU and the CPU agree on it exactly.
  for (const r of replays) inputs[r.slot] = replayStep(r, view.time).n;
  const params = new Float32Array(s.paramFloats);
  for (const p of s.params) params[p.slot] = p.def;
  for (const [name, value] of Object.entries(view.params ?? {})) {
    const p = s.params.find((x) => x.name === name);
    if (!p) throw new Error(`view param "${name}": the artefact declares no such param`);
    params[p.slot] = value;
  }
  return { time: view.time, inputs, params, curves };
}

/** The `FrameState` a host hands the pose pass for a view: the same mapping `frame` and `draw` use. */
export function viewState(a: Pick<Artefact, "inputs" | "inputFloats" | "params" | "curves" | "recurrences">, view: View, curves: Float32Array = a.curves): FrameState {
  return frameState({ ...a, paramFloats: a.params.reduce((n, p) => Math.max(n, p.slot + 1), 0), frame: { replay: replayClocks(a.recurrences) } }, curves, view);
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

/** One frame write's contents (FrameWrite: the closed list of per-frame CPU blocks); `frame`: a still sample's own uniform. */
function writeData(w: FrameWrite, view: View, state: FrameState, frame?: Float32Array<ArrayBuffer>): Float32Array<ArrayBuffer> {
  switch (w.kind) {
    case "frame": return frame ?? frameUniform(view);
    case "inputs": return state.inputs as Float32Array<ArrayBuffer>;
    case "params": return state.params as Float32Array<ArrayBuffer>;
  }
}

// ---- objects ---------------------------------------------------------------------------------------------------

const BUFFER_USAGE = { MAP_READ: 0x1, COPY_SRC: 0x4, COPY_DST: 0x8, UNIFORM: 0x40, STORAGE: 0x80 } as const;
const TEXTURE_USAGE = { COPY_SRC: 0x1, COPY_DST: 0x2, TEXTURE_BINDING: 0x4, STORAGE_BINDING: 0x8, RENDER_ATTACHMENT: 0x10 } as const;
const STAGE = { VERTEX: 0x1, FRAGMENT: 0x2, COMPUTE: 0x4 } as const;
const bits = <K extends string>(table: Record<K, number>, names: readonly K[]): number => names.reduce((n, k) => n | table[k], 0);

type Settled<T> = { readonly ok: true; readonly v: T } | { readonly ok: false; readonly message: string };
const settle = <T>(create: () => Promise<T>): Promise<Settled<T>> =>
  new Promise<T>((resolve) => resolve(create())).then((v) => ({ ok: true as const, v }), (e: unknown) => ({ ok: false as const, message: e instanceof Error ? e.message : String(e) }));
// Except at the author's own lines (`Manifest.authored`, mapped by `authorRefusal`; raw compute text is registered
// there too), the WGSL and descriptors are the compiler's, already accepted from the scene: a refusal is the GPU
// stack's (or a compiler bug), never the scene's, and a fresh device may pass (src/cli/main.ts retries).
const refused = (what: string, message: string) => new EnvironmentError(`${what}:\n${message}`, true);

/**
 * The GPU refused a line of WGSL the scene's author wrote (a raw function of `wgsl` text): the scene's mistake, at the
 * author's line. `site` is that line (`examples/eel.ts:14`); the message quotes it as the GPU read it (holes replaced),
 * the line of the module it sits at, and the GPU's own text.
 */
export class ShaderTextError extends SceneError {
  constructor(readonly site: string, what: string, error: string, quoted: string, module: string, line: number, gpu: string) {
    super(site, `${what} with a line the GPU refused (${error})`, "WGSL the GPU compiles: only the GPU reads a raw function", "correct that line (quoted above, with the GPU's message)");
    this.message += `\n  ${quoted.trim()}\n  (line ${line} of ${module} as compiled; \`loft show <scene>\` prints it with your lines marked)\n${gpu.trim()}`;
  }
}

/**
 * The author's line a GPU refusal of a module points at, as a scene error; undefined when the refusal names no line,
 * or a line the library wrote. The GPU (Tint) names the first error as `:line:column error: text` within the module,
 * whose code is its chunks concatenated with nothing between (ModuleDesc).
 */
function authorRefusal(b: Bundle, mod: ModuleDesc, message: string): ShaderTextError | undefined {
  const hit = /:(\d+):\d+ error: ([^\n]*)/.exec(message);
  if (hit === null) return undefined;
  const line = Number(hit[1]);
  let before = 0;
  for (const c of mod.chunks) {
    const name = b.manifest.chunks[c]!, text = b.chunks.get(name) ?? "";
    const lines = text.split("\n");
    // Line `line` of the module starts in this chunk when it is at most `lines.length - 1` lines past the chunk's
    // first full line (a chunk not ending in a newline shares its last line with the next chunk's first).
    const k = line - 1 - before;
    if (k < lines.length - (text.endsWith("\n") ? 1 : 0) || c === mod.chunks.at(-1)) {
      const at = authorLineAt(b.manifest.authored?.[name] ?? [], k);
      return at === undefined ? undefined : new ShaderTextError(`${at.file}:${at.line}`, at.what, hit[2]!, lines[k] ?? "", mod.label, line, message);
    }
    before += lines.length - 1;
  }
  return undefined;
}

/**
 * Creates every shader module, each inside its own validation scope; `what` names the module in the error.
 * createShaderModule is synchronous, so the scopes nest without overlap: module k's errors land in scope k, and
 * popping in reverse order hands each module its own scope. Reports the first failure in array order.
 */
async function scopedModules(device: GPUDevice, creations: readonly { what: string; create: () => GPUShaderModule; author: (message: string) => ShaderTextError | undefined }[]): Promise<GPUShaderModule[]> {
  const results = creations.map((c): Settled<GPUShaderModule> => {
    device.pushErrorScope("validation");
    try { return { ok: true, v: c.create() }; } catch (e) { return { ok: false, message: e instanceof Error ? e.message : String(e) }; }
  });
  const errors = (await Promise.all(creations.map(() => device.popErrorScope()))).reverse();
  creations.forEach((c, k) => {
    const r = results[k]!, error = errors[k];
    const message = !r.ok ? r.message : error?.message;
    if (message !== undefined) throw c.author(message) ?? refused(c.what, message);
  });
  return results.map((r) => (r as { v: GPUShaderModule }).v);
}

/**
 * Requests every pipeline at once and awaits them together, so the GPU stack compiles them concurrently instead of
 * one after another (16 s of a cold load was this queue). WebGPU error scopes are a stack per device: overlapping
 * per-pipeline push/pop pairs would pop each other's scopes, so one scope covers the whole batch. A refused pipeline
 * rejects its own promise, so it is still named by its `what`, the first in array order as the old loop reported;
 * an error only the shared scope saw names the batch, and the GPU's message names the object.
 */
async function scopedPipelines<T>(device: GPUDevice, batch: string, creations: readonly { what: string; create: () => Promise<T> }[]): Promise<T[]> {
  device.pushErrorScope("validation");
  const results = await Promise.all(creations.map((c) => settle(c.create)));
  const error = await device.popErrorScope();
  creations.forEach((c, k) => {
    const r = results[k]!;
    if (!r.ok) throw refused(c.what, r.message);
  });
  if (error) throw refused(batch, error.message);
  return results.map((r) => (r as { v: T }).v);
}

/** The objects a manifest describes, created; textures and the bind groups that name them come per view size (`sized`). */
export type Objects = {
  readonly manifest: Manifest;
  readonly buffers: readonly GPUBuffer[];
  readonly textures: readonly (GPUTexture | undefined)[];
  destroy(): void;
  readonly computePipelines: readonly GPUComputePipeline[];
  readonly renderPipelines: readonly GPURenderPipeline[];
  /** Bind groups naming no texture; undefined at the others' ids. */
  readonly fixedGroups: readonly (GPUBindGroup | undefined)[];
  /** The view-sized textures and every bind group, for one size. */
  sized(size: { width: number; height: number }): Sized;
};
export type Sized = {
  readonly width: number;
  readonly height: number;
  readonly textures: readonly GPUTexture[];
  readonly transient: readonly GPUTexture[];
  readonly views: readonly GPUTextureView[];
  readonly groups: readonly GPUBindGroup[];
};

function extent(t: TextureDesc): GPUExtent3DDict {
  const [width, height, depthOrArrayLayers = 1] = t.extent!;
  return { width, height, depthOrArrayLayers };
}

function rowBytes(t: TextureDesc): number {
  return t.extent![0] * COMPUTE_TEXEL_BYTES[t.format as keyof typeof COMPUTE_TEXEL_BYTES];
}

async function createObjects(device: GPUDevice, b: Bundle, mode: "all" | "bake" = "all"): Promise<Objects> {
  checkComputeData(b);
  const m = b.manifest;
  const buffers: GPUBuffer[] = [];
  const textures: (GPUTexture | undefined)[] = [];
  const destroy = () => {
    for (const buffer of buffers) buffer.destroy();
    for (const texture of textures) texture?.destroy();
  };
  try {
    for (const d of m.buffers) {
      const buffer = device.createBuffer({ label: d.label, size: d.size, usage: bits(BUFFER_USAGE, d.usage) });
      buffers.push(buffer);
      if (d.init === undefined) continue;
      const bytes = b.blobs.get(d.init);
      if (bytes === undefined) throw new Error(`artefact: buffer "${d.label}" starts from blob "${d.init}", which the bundle lacks`);
      if (bytes.byteLength > 0) device.queue.writeBuffer(buffer, 0, bytes as Uint8Array<ArrayBuffer>);
    }
    for (const d of m.textures) {
      if (d.extent === undefined) { textures.push(undefined); continue; }
      const t = device.createTexture({ label: d.label, size: extent(d), dimension: d.extent.length === 2 ? "2d" : "3d", format: d.format, sampleCount: 1, usage: bits(TEXTURE_USAGE, d.usage) });
      textures.push(t);
      if (d.init === undefined) continue;
      const bytes = b.blobs.get(d.init);
      if (bytes === undefined) throw new Error(`artefact: texture "${d.label}" starts from blob "${d.init}", which the bundle lacks`);
      device.queue.writeTexture({ texture: t }, bytes as Uint8Array<ArrayBuffer>, { bytesPerRow: rowBytes(d), rowsPerImage: d.extent[1] }, extent(d));
    }
    const textureViews = textures.map((t) => t?.createView());
    const samplers = m.samplers.map((s) => device.createSampler(s));
    const layouts = m.bindGroupLayouts.map((l) => device.createBindGroupLayout({
      label: l.label,
      entries: l.entries.map((e): GPUBindGroupLayoutEntry => ({ ...e, visibility: bits(STAGE, e.visibility) })),
    }));
    const pipelineLayouts = m.pipelineLayouts.map((p) => device.createPipelineLayout({ label: p.label, bindGroupLayouts: p.bindGroupLayouts.map((i) => layouts[i]!) }));
    // Build compiles only the kernels it runs. Rendering and load pipelines are the eventual loader's work.
    const buildPipelines = new Set(m.build?.runs.map((d) => d.run.pipeline));
    const buildModules = new Set([...buildPipelines].map((id) => m.computePipelines[id]!.module));
    const moduleSpecs = m.modules.flatMap((mod, i) => mode === "all" || buildModules.has(i) ? [{ mod, i }] : []);
    const made = await scopedModules(device, moduleSpecs.map(({ mod }) => {
      const code = mod.chunks.map((c) => {
        const text = b.chunks.get(m.chunks[c]!);
        if (text === undefined) throw new Error(`artefact: module ${mod.label} names chunk ${m.chunks[c]}, which the bundle lacks`);
        return text;
      }).join("");
      return { what: `${mod.label} failed to compile`, create: () => device.createShaderModule({ label: mod.label, code }), author: (message: string) => authorRefusal(b, mod, message) };
    }));
    const modules = new Map(moduleSpecs.map(({ i }, j) => [i, made[j]!]));
    const computeSpecs = m.computePipelines.flatMap((p, i) => mode === "all" || buildPipelines.has(i) ? [{ p, i }] : []);
    const renderSpecs = mode === "all" ? m.renderPipelines : [];
    // Pipelines are built eagerly and concurrently, so the first frame never stalls on compilation.
    const pipelines = await scopedPipelines<GPUComputePipeline | GPURenderPipeline>(device, `${computeSpecs.length + renderSpecs.length} pipelines: creation failed`, [
      ...computeSpecs.map(({ p }) => ({
        what: p.label.startsWith('compute "') ? `${p.label}: pipeline creation failed` : `pose stage "${p.label}": pipeline creation failed`,
        create: () => device.createComputePipelineAsync({
          label: p.label, layout: pipelineLayouts[p.layout]!, compute: { module: modules.get(p.module)!, entryPoint: p.entryPoint },
        }),
      })),
      ...renderSpecs.map((p) => ({ what: `${p.label}: pipeline creation failed`, create: () => device.createRenderPipelineAsync({
        label: p.label,
        layout: pipelineLayouts[p.layout]!,
        vertex: { module: modules.get(p.vertex.module)!, entryPoint: p.vertex.entryPoint, constants: p.vertex.constants },
        ...(p.fragment === undefined ? {} : { fragment: { module: modules.get(p.fragment.module)!, entryPoint: p.fragment.entryPoint, constants: p.fragment.constants, targets: p.fragment.targets.map((t) => ({ ...t })) } }),
        primitive: p.primitive,
        ...(p.depthStencil === undefined ? {} : { depthStencil: p.depthStencil }),
        multisample: p.multisample,
      }) })),
    ]);
    const computePipelines: GPUComputePipeline[] = [];
    computeSpecs.forEach(({ i }, k) => { computePipelines[i] = pipelines[k] as GPUComputePipeline; });
    const renderPipelines = pipelines.slice(computeSpecs.length) as GPURenderPipeline[];
    const group = (k: number, views: readonly (GPUTextureView | undefined)[]): GPUBindGroup => {
      const g = m.bindGroups[k]!;
      return device.createBindGroup({
        label: g.label, layout: layouts[g.layout]!,
        entries: g.entries.map((e): GPUBindGroupEntry => {
          if ("buffer" in e) return { binding: e.binding, resource: { buffer: buffers[e.buffer]! } };
          if ("texture" in e) return { binding: e.binding, resource: views[e.texture]! };
          return { binding: e.binding, resource: samplers[e.sampler]! };
        }),
      });
    };
    // A group that reads only persistent textures is created once, even if the viewport changes.
    const fixedGroups = m.bindGroups.map((g, k) => g.entries.some((e) => "texture" in e && textureViews[e.texture] === undefined) ? undefined : group(k, textureViews));
    return {
      manifest: m, buffers, textures, computePipelines, renderPipelines, fixedGroups, destroy,
      sized: (size) => {
        const transient: GPUTexture[] = [];
        const all = m.textures.map((t, i) => {
          const fixed = textures[i];
          if (fixed !== undefined) return fixed;
          const texture = device.createTexture({ label: t.label, size: textureSize(t.size, size), format: t.format, sampleCount: t.sampleCount, usage: bits(TEXTURE_USAGE, t.usage) });
          transient.push(texture);
          return texture;
        });
        const views = all.map((t, i) => textureViews[i] ?? t.createView());
        const groups = m.bindGroups.map((g, k) => fixedGroups[k] ?? group(k, views));
        return { ...size, textures: all, transient, views, groups };
      },
    };
  } catch (e) {
    destroy();
    throw e;
  }
}

/** A texture's pixels for a view of this size (TextureDesc.size: absent is the view's). */
export function textureSize(t: TextureSize | undefined, view: { readonly width: number; readonly height: number }): { width: number; height: number } {
  if (t === undefined) return { width: view.width, height: view.height };
  switch (t.kind) {
    case "view": return { width: Math.max(1, Math.round(view.width * t.scale)), height: Math.max(1, Math.round(view.height * t.scale)) };
    case "fixed": return { width: t.width, height: t.height };
  }
}

// ---- recording -------------------------------------------------------------------------------------------------

/**
 * How a frame gets the two states n and n + 1 of one checkpointed recurrence into its even and odd tables. One decision,
 * made once (`advance`), executed by one dispatcher (`dispatchReplay`).
 *
 * kept    the tables already hold n and n + 1 (the same step as the last frame: a pause, or a seek inside one step).
 * resume  step `steps` times on from the last frame's pair (n_last, n_last + 1), no checkpoint read.
 * replay  `first` from checkpoint `checkpoint`, then `steps` more (docs/artefact_format.md, "Replays").
 */
export type Advance =
  | { readonly kind: "kept" }
  | { readonly kind: "resume"; readonly from: number; readonly steps: number }
  | { readonly kind: "replay"; readonly checkpoint: number; readonly steps: number };

/**
 * Choose how to reach step n (src/artefact.ts `replayStep`) given the step the tables hold from the last frame (`last`,
 * undefined before the first frame or after anything that may have disturbed them).
 *
 * Resume when d = n - last satisfies 0 < d <= k = n mod every, i.e. when it costs no more dispatches than the replay
 * (d against k + 1) and steps from the checkpoint no further than the replay would. Resume needs an even `every`: the
 * two tables then hold state j in table j mod 2 whichever way they were reached (the compiler keeps `every` even), so
 * the step to take next depends on j alone. An odd `every` (an older file) always replays.
 *
 * Why this is exact: a step reads only the state and its own clock entry, and writes only the other table, so state
 * n_last + 1 stepped d times is the very computation the replay or the bake ran, in the same module: the same bytes.
 */
export function advance(r: ReplayClock, last: number | undefined, t: number): Advance {
  const { n, k } = replayStep(r, t);
  if (last === n) return { kind: "kept" };
  const d = last === undefined ? 0 : n - last;
  if (last !== undefined && d > 0 && d <= k && r.every % 2 === 0) return { kind: "resume", from: last, steps: d };
  return { kind: "replay", checkpoint: Math.floor(n / r.every), steps: k };
}

/** One replay's dispatches for an `Advance`, into an open compute pass. */
export function dispatchReplay(pass: GPUComputePassEncoder, o: Objects, r: Replay, a: Advance): void {
  switch (a.kind) {
    case "kept": return;
    case "resume":
      // The source of step j is state from + j, which lives in table (from + j) mod 2: ping reads odd, pong reads even.
      for (let j = 1; j <= a.steps; j++) dispatchAll(pass, o, [(a.from + j) % 2 === 1 ? r.ping : r.pong]);
      return;
    case "replay":
      dispatchAll(pass, o, [r.first]);
      for (let j = 1; j <= a.steps; j++) dispatchAll(pass, o, [j % 2 === 1 ? r.ping : r.pong]);
      return;
  }
}

/**
 * The replays of a frame at time t, into an open compute pass before its dispatches. `last` is the step each replay's
 * tables hold; the steps this frame leaves them at are returned, for the caller to record once the frame is submitted.
 */
export function dispatchReplays(pass: GPUComputePassEncoder, o: Objects, replays: readonly Replay[], t: number, last: ReadonlyMap<Replay, number>): Map<Replay, number> {
  const now = new Map<Replay, number>();
  for (const r of replays) {
    const a = advance(r, last.get(r), t);
    dispatchReplay(pass, o, r, a);
    now.set(r, replayStep(r, t).n);
  }
  return now;
}

/** The pose pass's dispatches, into an open compute pass. */
export function dispatchAll(pass: GPUComputePassEncoder, o: Objects, list: readonly Dispatch[]): void {
  for (const d of list) {
    pass.setPipeline(o.computePipelines[d.pipeline]!);
    d.bindGroups.forEach((g, i) => pass.setBindGroup(i, o.fixedGroups[g]!));
    if (typeof d.workgroups === "number") pass.dispatchWorkgroups(d.workgroups);
    else pass.dispatchWorkgroups(...d.workgroups);
  }
}

function clearCompute(encoder: GPUCommandEncoder, o: Objects, cmd: RawDispatch): void {
  for (const id of cmd.clearBuffers ?? []) encoder.clearBuffer(o.buffers[id]!);
  for (const { from, to } of cmd.clearTextures ?? []) {
    encoder.copyTextureToTexture({ texture: o.textures[from]! }, { texture: o.textures[to]! }, extent(o.manifest.textures[to]!));
  }
}

/** One author dispatch with its zero initial state. Used for load, steps and profile attribution. */
export function recordCompute(encoder: GPUCommandEncoder, o: Objects, cmd: RawDispatch, timestampWrites?: GPUComputePassTimestampWrites): void {
  clearCompute(encoder, o, cmd);
  const pass = encoder.beginComputePass({ label: cmd.run.label, ...(timestampWrites === undefined ? {} : { timestampWrites }) });
  dispatchAll(pass, o, [cmd.run]);
  pass.end();
}

function recordLoad(encoder: GPUCommandEncoder, o: Objects, job: ComputeRun): void {
  switch (job.kind) {
    case "once": return recordCompute(encoder, o, job);
    case "steps": return recordSteps(job.count, job.start, job.odd, job.even, (run, step) => {
      recordCompute(encoder, o, run);
      if (step % job.keep !== 0 && step !== job.count) return;
      const kept = Math.ceil(step / job.keep);
      for (const copy of job.history) {
        const from = step % 2 === 0 ? copy.a : copy.b;
        switch (copy.kind) {
          case "buffer": encoder.copyBufferToBuffer(o.buffers[from]!, 0, o.buffers[copy.to]!, kept * copy.bytes, copy.bytes); break;
          case "texture": {
            const d = o.manifest.textures[from]!, size = extent(d);
            const origin = d.extent!.length === 2 ? { y: kept * size.height! } : { z: kept * size.depthOrArrayLayers! };
            encoder.copyTextureToTexture({ texture: o.textures[from]! }, { texture: o.textures[copy.to]!, origin }, size);
            break;
          }
        }
      }
    });
  }
}

async function submitCompute<T>(device: GPUDevice, label: string, record: (encoder: GPUCommandEncoder) => T): Promise<T> {
  device.pushErrorScope("validation");
  try {
    const encoder = device.createCommandEncoder({ label });
    const out = record(encoder);
    device.queue.submit([encoder.finish()]);
    return out;
  } finally {
    const error = await device.popErrorScope();
    if (error) throw new EnvironmentError(`${label}: WebGPU refused the compute commands:\n${error.message}`, true);
  }
}

/** Finish a normal build's deferred jobs and return tightly packed bytes for its second lowering. */
export async function bakeRawBuild(device: GPUDevice, b: Bundle): Promise<ReadonlyMap<string, Uint8Array>> {
  const build = b.manifest.build;
  if (build === undefined) return new Map();
  const o = await createObjects(device, b, "bake");
  const staging: GPUBuffer[] = [];
  try {
    const reads = await submitCompute(device, "loft compute build", (encoder) => {
      for (const run of build.runs) recordCompute(encoder, o, run);
      return build.outputs.map((out) => {
        const texture = out.kind === "texture" ? b.manifest.textures[out.id]! : undefined;
        const row = texture === undefined ? 0 : rowBytes(texture);
        const pitch = Math.ceil(row / 256) * 256;
        const rows = texture === undefined ? 0 : texture.extent![1] * (texture.extent![2] ?? 1);
        const size = out.kind === "buffer" ? out.bytes : pitch * rows;
        const read = device.createBuffer({ label: `bake ${out.name} readback`, size, usage: GPUBufferUsage.MAP_READ | GPUBufferUsage.COPY_DST });
        staging.push(read);
        if (texture === undefined) encoder.copyBufferToBuffer(o.buffers[out.id]!, 0, read, 0, size);
        else encoder.copyTextureToBuffer({ texture: o.textures[out.id]! }, { buffer: read, bytesPerRow: pitch, rowsPerImage: texture.extent![1] }, extent(texture));
        return { name: out.name, read, row, pitch, rows };
      });
    });
    const result = new Map<string, Uint8Array>();
    for (const { name, read, row, pitch, rows } of reads) {
      const bytes = await readBake(read);
      if (row === pitch) { result.set(name, bytes); continue; }
      const packed = new Uint8Array(row * rows);
      for (let i = 0; i < rows; i++) packed.set(bytes.subarray(i * pitch, i * pitch + row), i * row);
      result.set(name, packed);
    }
    return result;
  } finally {
    for (const read of staging) read.destroy();
    o.destroy();
  }
}

/** Draw calls for one command (a planned draw with no run issues nothing). */
export function command(pass: GPURenderPassEncoder, o: Objects, c: RenderPassDesc["commands"][number], plan: Plan): void {
  switch (c.kind) {
    case "fixed":
      pass.setPipeline(o.renderPipelines[c.pipeline]!);
      pass.draw(c.vertices, c.instances);
      return;
    case "planned": {
      const runs = plan[c.draw]!;
      if (runs.length === 0) return;
      pass.setPipeline(o.renderPipelines[c.pipeline]!);
      for (const r of runs) pass.draw(r.vertices, r.instances, 0, (c.instanceBase ?? 0) + r.first);
      return;
    }
  }
}

export const targetView = (s: Sized, t: Target, out: GPUTextureView): GPUTextureView => {
  switch (t.kind) {
    case "texture": return s.views[t.texture]!;
    case "output": return out;
  }
};

const colour = (c: readonly [number, number, number, number] | undefined): GPUColor | undefined => (c === undefined ? undefined : { r: c[0], g: c[1], b: c[2], a: c[3] });

/** A render pass's descriptor, resolved against the objects of one size. */
export function passDescriptor(p: RenderPassDesc, s: Sized, out: GPUTextureView): GPURenderPassDescriptor {
  const c = p.color, d = p.depth;
  return {
    label: p.label,
    colorAttachments: c === undefined ? [] : [{
      view: targetView(s, c.view, out),
      ...(c.resolveTarget === undefined ? {} : { resolveTarget: targetView(s, c.resolveTarget, out) }),
      loadOp: c.loadOp, storeOp: c.storeOp,
      ...(c.clearValue === undefined ? {} : { clearValue: colour(c.clearValue) }),
    }],
    ...(d === undefined ? {} : {
      depthStencilAttachment: {
        view: s.views[d.view]!, depthLoadOp: d.depthLoadOp, depthStoreOp: d.depthStoreOp,
        ...(d.depthClearValue === undefined ? {} : { depthClearValue: d.depthClearValue }),
      },
    }),
  };
}

/**
 * The frame's render passes. `stamps` (frame timing): timestamp `begin` at the start of the first pass and `end` at the
 * end of the last (on the real passes: Dawn on Metal writes no timestamps for an empty pass).
 */
function recordPasses(encoder: GPUCommandEncoder, o: Objects, s: Sized, out: GPUTextureView, plan: Plan, stamps?: { readonly querySet: GPUQuerySet; readonly begin: number; readonly end: number }, range?: readonly [number, number]): void {
  const passes = o.manifest.frame.passes;
  for (const [k, p] of passes.entries()) {
    if (range !== undefined && (k < range[0] || k >= range[1])) continue;
    const writes = stamps === undefined ? undefined : {
      querySet: stamps.querySet,
      ...(k === 0 ? { beginningOfPassWriteIndex: stamps.begin } : {}),
      ...(k === passes.length - 1 ? { endOfPassWriteIndex: stamps.end } : {}),
    };
    const pass = encoder.beginRenderPass({ ...passDescriptor(p, s, out), ...(writes !== undefined && (k === 0 || k === passes.length - 1) ? { timestampWrites: writes } : {}) });
    p.bindGroups.forEach((g, i) => pass.setBindGroup(i, s.groups[g]!));
    for (const c of p.commands) command(pass, o, c, plan);
    pass.end();
  }
}

/** What a plan draws (FrameStats): scene triangles, and the draw calls issued. */
function statsOf(plan: Plan): FrameStats {
  let triangles = 0, draws = 0;
  for (const runs of plan) for (const r of runs) { triangles += (r.vertices / 3) * r.instances; draws++; }
  return { triangles: Math.round(triangles), draws };
}

const sameSize = (t: { width: number; height: number }, v: View) => t.width === v.width && t.height === v.height;
/** The headless output texture and its readback buffer. */
type Readback = { readonly width: number; readonly height: number; readonly texture: GPUTexture; readonly buffer: GPUBuffer; readonly bytesPerRow: number };

// ---- the executor ----------------------------------------------------------------------------------------------

/** Load a lowered artefact: create its objects, and return the frame functions. */
export async function loadBundle(device: GPUDevice, bundle: Bundle, cpu: CpuTables, ask?: Precision): Promise<GpuLoaded> {
  const { bundle: b, precision } = selectPrecision(bundle, device.features.has("shader-f16"), ask);
  const m = b.manifest;
  // The `cpu` tables, by place: each frame's uploads must be exactly these, each its table's size.
  const cpuTables = new Map(m.tables.filter((t) => t.fill.kind === "cpu").map((t) => [`${m.buffers[t.buffer]!.label}@${t.offset}`, t]));
  if (cpu.kind === "none" && cpuTables.size > 0) {
    throw new NotDataError(`artefact: ${cpuTables.size} table(s) are filled by JavaScript, which a file cannot hold:${[...cpuTables.values()].map((t) => ` "${t.name}"`).join(",")}`);
  }
  const o = await createObjects(device, b);
  try {
    if (m.build !== undefined || (m.load?.length ?? 0) > 0) {
      await submitCompute(device, "loft compute at load", (encoder) => {
        for (const run of m.build?.runs ?? []) recordCompute(encoder, o, run);
        for (const run of m.load ?? []) recordLoad(encoder, o, run);
      });
      await device.queue.onSubmittedWorkDone();
    }
  } catch (e) { o.destroy(); throw e; }
  const detail = new Float32Array(m.frame.detail.floats);
  // Each grid's level last frame, for hysteresis (src/host/detail.ts): the host's only memory between frames. Only the
  // interactive `draw` reads it (a dolly must not flicker); headless frames plan without it (`posedPlan`).
  const memory: Memory = new Map();
  const bounds = m.frame.bounds;

  let sized: Sized | undefined;
  let readback: Readback | undefined;
  let reading = false;
  const sizedFor = (view: View): Sized => {
    if (sized && sameSize(sized, view)) return sized;
    for (const t of sized?.transient ?? []) t.destroy();
    sized = o.sized({ width: view.width, height: view.height });
    return sized;
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

  /** The read of the bounds buffer in flight (interactive frames do not wait for it). */
  let boundsPending: Promise<Bounds> | undefined;
  /** What interactive frames plan with: the last bounds read. */
  let lastBounds: Bounds = { kind: "none" };
  const readBounds = async (): Promise<Bounds> => {
    switch (bounds.kind) {
      case "none": return { kind: "none" };
      case "read": {
        const buf = o.buffers[bounds.buffer]!;
        await buf.mapAsync(GPUMapMode.READ);
        const data = new Float32Array(buf.getMappedRange().slice(0));
        buf.unmap();
        const at = new Map(bounds.grids.map((g) => [g.slot, g.at]));
        return { kind: "read", at: (slot) => data.subarray(at.get(slot)! * 4) };
      }
    }
  };

  // The curve data the GPU holds, so the CPU fills read the same curves (FrameState.curves).
  const curveBlob = m.buffers[m.curves.buffer]!.init;
  let curves = curveBlob === undefined ? new Float32Array(0) : new Float32Array(b.blobs.get(curveBlob)!.slice().buffer);
  const cpuUploads = (state: FrameState): readonly CpuUpload[] => {
    switch (cpu.kind) {
      case "none": return [];
      case "js": return cpu.uploads(state);
    }
  };
  const bufferNamed = new Map(m.dynamic.map((k) => [m.buffers[k]!.label, o.buffers[k]!]));

  /** The step each checkpointed recurrence's even and odd tables hold (as the last submitted frame left them): what `advance` resumes from. */
  const replayedAt = new Map<Replay, number>();
  /** Write the frame's state and `cpu` tables, then submit the pose pass (and, when `copyBounds`, the copy of the bounds). */
  const pose = (view: View, copyBounds: boolean, stamps?: Stamps, frame?: Float32Array<ArrayBuffer>, cold = false): number => {
    // A measuring frame (profile, frameGpu) is repeated at one time to time it: replay it every time, as a seek does.
    if (cold) replayedAt.clear();
    const t0 = performance.now();
    const state = frameState(m, curves, view);
    const uploads = cpuUploads(state);
    for (const u of uploads) {
      const t = cpuTables.get(`${u.buffer}@${u.offset}`);
      if (t === undefined) throw new Error(`cpuPose returned data for "${u.buffer}" at vec4 ${u.offset}, where no cpu table lives`);
      if (u.data.length !== t.vec4s * 4) throw new Error(`cpuPose returned table "${t.name}" with ${u.data.length} floats; expected ${t.vec4s * 4} (${t.vec4s} vec4s)`);
      device.queue.writeBuffer(bufferNamed.get(u.buffer)!, u.offset * 16, u.data as Float32Array<ArrayBuffer>);
    }
    if (uploads.length !== cpuTables.size) {
      const given = new Set(uploads.map((u) => `${u.buffer}@${u.offset}`));
      throw new Error(`cpuPose did not return cpu table${[...cpuTables].filter(([k]) => !given.has(k)).map(([, t]) => ` "${t.name}"`).join(",")}`);
    }
    for (const w of m.frame.writes) {
      const data = writeData(w, view, state, frame);
      if (data.length > 0) device.queue.writeBuffer(o.buffers[w.buffer]!, 0, data);
    }
    const encoder = device.createCommandEncoder({ label: "loft pose" });
    let replayed: ReadonlyMap<Replay, number> | undefined;
    const replays = m.frame.replay ?? [];
    const compute = m.frame.compute ?? [];
    // Frame outputs have distinct writers. Clear all of them up front, then dispatch into one pass with pose.
    // Each dispatch has its own usage scope, so an output texture can be sampled by a later dispatch.
    for (const cmd of compute) clearCompute(encoder, o, cmd);
    if (compute.length > 0 || m.frame.pose.length > 0 || replays.length > 0) {
      const pass = encoder.beginComputePass({ label: "loft pose", ...(stamps === undefined ? {} : { timestampWrites: {
        querySet: stamps.querySet, beginningOfPassWriteIndex: stamps.begin, endOfPassWriteIndex: stamps.end,
      } }) });
      for (const cmd of compute) dispatchAll(pass, o, [cmd.run]);
      replayed = dispatchReplays(pass, o, replays, view.time, replayedAt);
      dispatchAll(pass, o, m.frame.pose);
      pass.end();
    }
    // Each pass's camera, from its table into its frame uniform (Manifest.frame.copies), before any pass draws.
    for (const c of m.frame.copies ?? []) encoder.copyBufferToBuffer(o.buffers[c.from]!, c.fromOffset, o.buffers[c.to]!, 0, c.bytes);
    if (copyBounds && bounds.kind === "read") {
      for (const g of bounds.grids) encoder.copyBufferToBuffer(o.buffers[g.from]!, g.fromOffset, o.buffers[bounds.buffer]!, g.at * 16, g.instances * 16);
    }
    // Forget the kept pair first and record the new one only once this frame is submitted: a frame that throws before
    // then (or fails validation) leaves the tables in an unknown state, and the next frame must replay from a checkpoint.
    replayedAt.clear();
    device.queue.submit([encoder.finish()]);
    for (const [r, n] of replayed ?? []) replayedAt.set(r, n);
    return performance.now() - t0;
  };
  /** This frame's plan from these bounds, its level-of-detail slots written (before the draws' submit). */
  const plan = (view: View, read: Bounds, levels: Memory = memory): Plan => {
    const p = planFrame(m.plan, view, read, levels, detail);
    device.queue.writeBuffer(o.buffers[m.frame.detail.buffer]!, 0, detail);
    return p;
  };
  /** A frame that waits for its own bounds (headless frames, profiling): pose, read the bounds, plan. */
  const posedPlan = async (view: View, stamps?: Stamps, cold = false): Promise<{ readonly plan: Plan; readonly poseMs: number }> => {
    if (boundsPending) await boundsPending;
    const copy = bounds.kind === "read";
    const poseMs = await validated("pose", () => pose(view, copy, stamps, undefined, cold));
    const read = copy ? await readBounds() : lastBounds;
    const t0 = performance.now();
    // An empty memory: a headless frame's levels come from its own view, never from the frame drawn before it. With the
    // hysteresis memory, `render --views front,side` drew `side` of examples/strike.ts with 63313 triangles, and
    // `--views side` alone with 62321; a warm server (loft serve) would make every frame depend on its predecessors.
    const p = plan(view, read, new Map());
    return { plan: p, poseMs: poseMs + performance.now() - t0 };
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
        recordPasses(encoder, o, sizedFor(view), rb.texture.createView(), posed.plan);
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
    precision,
    frameTimed,
    frame: async (view) => (await frameTimed(view)).pixels,
    draw: async (view, target) => {
      checkView(view);
      if (target.format !== COLOR_FORMAT) throw new Error(`draw: target format is ${target.format}; configure the canvas with ${COLOR_FORMAT}`);
      if (target.width !== view.width || target.height !== view.height) {
        throw new Error(`draw: target is ${target.width} x ${target.height} but the view is ${view.width} x ${view.height}`);
      }
      // Never waits on the GPU: plans with the last bounds read, and starts reading this frame's when none is in flight.
      const copy = bounds.kind === "read" && boundsPending === undefined;
      const stats = await validated("draw", () => {
        pose(view, copy);
        const p = plan(view, lastBounds);
        const encoder = device.createCommandEncoder({ label: "loft draw" });
        recordPasses(encoder, o, sizedFor(view), target.createView(), p);
        device.queue.submit([encoder.finish()]);
        return statsOf(p);
      });
      if (copy) boundsPending = readBounds().then((r) => { lastBounds = r; boundsPending = undefined; return r; });
      return stats;
    },
    profile: async (view, frames) => {
      checkView(view);
      if (!(Number.isInteger(frames) && frames >= 1)) throw new Error(`profile: frames must be a positive integer, got ${frames}`);
      return profileFrames({ device, objects: o, size: { width: view.width, height: view.height }, frames, upload: () => posedPlan(view, undefined, true) });
    },
    poseTime: async (view, repeats, rounds) => {
      checkView(view);
      if (!device.features.has("timestamp-query")) throw new Error("poseTime: the device was created without the \"timestamp-query\" feature");
      await posedPlan(view, undefined, true);
      const query = device.createQuerySet({ type: "timestamp", count: 2 });
      const resolve = device.createBuffer({ size: 16, usage: GPUBufferUsage.QUERY_RESOLVE | GPUBufferUsage.COPY_SRC });
      const read = device.createBuffer({ size: 16, usage: GPUBufferUsage.MAP_READ | GPUBufferUsage.COPY_DST });
      /** Least over the rounds of the per-repeat time of these dispatches, run in order `repeats` times in one pass. */
      const time = async (list: readonly Dispatch[]): Promise<number> => {
        let best = Infinity;
        for (let r = 0; r < rounds; r++) {
          const encoder = device.createCommandEncoder({ label: "loft pose time" });
          const pass = encoder.beginComputePass({ label: "pose x repeats", timestampWrites: { querySet: query, beginningOfPassWriteIndex: 0, endOfPassWriteIndex: 1 } });
          for (let k = 0; k < repeats; k++) dispatchAll(pass, o, list);
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
      for (let w = 0; w < 4; w++) await time(m.frame.pose);
      const total = await time(m.frame.pose);
      const stages: number[] = [];
      for (const d of m.frame.pose) stages.push(await time([d]));
      query.destroy(); resolve.destroy(); read.destroy();
      return { total, stages };
    },
    frameGpu: async (view, frames) => {
      checkView(view);
      if (!device.features.has("timestamp-query")) throw new Error("frameGpu: the device was created without the \"timestamp-query\" feature");
      const query = device.createQuerySet({ type: "timestamp", count: 4 });
      const resolve = device.createBuffer({ size: 32, usage: GPUBufferUsage.QUERY_RESOLVE | GPUBufferUsage.COPY_SRC });
      const read = device.createBuffer({ size: 32, usage: GPUBufferUsage.MAP_READ | GPUBufferUsage.COPY_DST });
      // Timestamps come in whole ticks (0.065536 ms on the Apple GPUs measured): their differences' common divisor.
      const gcd = (a: number, b: number): number => (a === 0 ? b : gcd(b % a, a));
      let best = { ms: Infinity, pose: 0, draws: 0 }, tick = 0;
      for (let f = 0; f < frames + 2; f++) {
        const posed = await posedPlan(view, { querySet: query, begin: 0, end: 1 }, true);
        const rb = readbackFor(view);
        const encoder = device.createCommandEncoder({ label: "loft frame (timed)" });
        recordPasses(encoder, o, sizedFor(view), rb.texture.createView(), posed.plan, { querySet: query, begin: 2, end: 3 });
        encoder.resolveQuerySet(query, 0, 4, resolve, 0);
        encoder.copyBufferToBuffer(resolve, 0, read, 0, 32);
        device.queue.submit([encoder.finish()]);
        await read.mapAsync(GPUMapMode.READ);
        const ts = [...new BigUint64Array(read.getMappedRange().slice(0))].map(Number);
        read.unmap();
        for (const d of [ts[1]! - ts[0]!, ts[3]! - ts[2]!, ts[2]! - ts[1]!]) if (d > 0) tick = gcd(tick, d);
        const p = (ts[1]! - ts[0]!) / 1e6, dr = (ts[3]! - ts[2]!) / 1e6;
        // The first two frames warm the clocks (an idle Apple GPU starts a burst at a low clock).
        if (f >= 2 && p + dr < best.ms) best = { ms: p + dr, pose: p, draws: dr };
      }
      query.destroy(); resolve.destroy(); read.destroy();
      return { ...best, tick: tick / 1e6 };
    },
    stillTimed: async (view, still) => {
      if (reading) throw new Error("still: called again before the previous frame resolved");
      checkView(view);
      reading = true;
      try {
        if (boundsPending) await boundsPending;
        const t0 = performance.now();
        const rb = readbackFor(view), sz = sizedFor(view);
        const drawn = await stillFrame({
          device, passes: m.frame.passes, views: sz.views, textures: sz.textures, out: rb.texture,
          lightSampled: m.inputs.some((i) => i.name === LIGHT_SAMPLE_INPUT),
          // One bounds read (sample 0's); each sample plans from it with no memory: a still depends on no frame before it.
          bounds: async (v, frame) => {
            const copy = bounds.kind === "read";
            await validated("still", () => pose(v, copy, undefined, frame));
            return copy ? await readBounds() : lastBounds;
          },
          sample: (v, frame, read) => {
            pose(v, false, undefined, frame);
            return plan(v, read, new Map());
          },
          record: (encoder, out, p, from, to) => recordPasses(encoder, o, sz, out, p, undefined, [from, to]),
        }, view, still);
        const t1 = performance.now();
        const encoder = device.createCommandEncoder({ label: "loft still readback" });
        encoder.copyTextureToBuffer({ texture: rb.texture }, { buffer: rb.buffer, bytesPerRow: rb.bytesPerRow, rowsPerImage: view.height }, { width: view.width, height: view.height });
        device.queue.submit([encoder.finish()]);
        await rb.buffer.mapAsync(GPUMapMode.READ);
        const mapped = new Uint8Array(rb.buffer.getMappedRange());
        const row = view.width * 4;
        const pixels = new Uint8Array(row * view.height);
        for (let y = 0; y < view.height; y++) pixels.set(mapped.subarray(y * rb.bytesPerRow, y * rb.bytesPerRow + row), y * row);
        rb.buffer.unmap();
        return { pixels, ms: { pose: t1 - t0, encode: 0, readback: performance.now() - t1 }, drawn };
      } finally {
        reading = false;
      }
    },
    readTables: async (view) => {
      checkView(view);
      if (boundsPending) await boundsPending;
      await validated("readTables", () => pose(view, false));
      const dynamic = m.dynamic;
      const staging = dynamic.map((k) => device.createBuffer({ label: `read ${m.buffers[k]!.label}`, size: m.buffers[k]!.size, usage: GPUBufferUsage.MAP_READ | GPUBufferUsage.COPY_DST }));
      const encoder = device.createCommandEncoder({ label: "loft read tables" });
      dynamic.forEach((k, i) => encoder.copyBufferToBuffer(o.buffers[k]!, 0, staging[i]!, 0, m.buffers[k]!.size));
      device.queue.submit([encoder.finish()]);
      const out: Record<string, Float32Array> = {};
      for (const [i, k] of dynamic.entries()) {
        await staging[i]!.mapAsync(GPUMapMode.READ);
        out[m.buffers[k]!.label] = new Float32Array(staging[i]!.getMappedRange().slice(0));
        staging[i]!.destroy();
      }
      return out;
    },
    setCurves: (next) => {
      if (next.length !== m.curves.floats) throw new Error(`setCurves: got ${next.length} floats; the artefact has ${m.curves.floats}`);
      if (next.length > 0) device.queue.writeBuffer(o.buffers[m.curves.buffer]!, 0, next as Float32Array<ArrayBuffer>);
      // A copy: the caller may keep editing its array, and the pose must see what the GPU was given.
      curves = next.slice();
    },
    destroy: () => {
      o.destroy();
      for (const t of sized?.transient ?? []) t.destroy();
      readback?.texture.destroy(); readback?.buffer.destroy();
      sized = undefined; readback = undefined;
    },
  };
}
