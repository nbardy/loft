// The WebGPU host: draws an Artefact (src/artefact.ts) headless (read back to RGBA8) or into a canvas texture.
//
// Both entry points share one path: `record` fills the frame state, runs the pose function, uploads every table and
// records one multisampled render pass that resolves into a caller-given texture view. `frame` resolves into its own
// texture and reads it back; `draw` resolves into the caller's texture (a canvas's current texture).
//
// This file imports no bun-only module, so a browser bundle of src/host (minus headless.ts) stays clean.

import { PRELUDE_WGSL, type Artefact, type Draw, type FrameState, type Host, type Loaded, type View } from "../artefact.ts";
import { CURVE_N, type NumTy } from "../expr.ts";
import { cameraMatrices } from "./camera.ts";

/** The colour target format. A canvas drawn through `draw` must be configured with this format. */
export const COLOR_FORMAT: GPUTextureFormat = "rgba8unorm";
export const DEPTH_FORMAT: GPUTextureFormat = "depth32float";
export const SAMPLES = 4;

export type FrameTimings = {
  /** CPU: input layout and `artefact.pose`. */
  readonly pose: number;
  /** CPU: buffer uploads, command encoding and submit. */
  readonly encode: number;
  /** GPU execution plus the wait for the mapped readback and repacking rows. */
  readonly readback: number;
};

export interface GpuLoaded extends Loaded {
  /** `frame`, with milliseconds per stage. */
  frameTimed(view: View): Promise<{ readonly pixels: Uint8Array; readonly ms: FrameTimings }>;
  /**
   * Browser path: draw the same frame into `target` (for example `context.getCurrentTexture()`), which must be
   * `COLOR_FORMAT` and exactly `view.width` x `view.height`. Resolves once validation of the submit is known.
   */
  draw(view: View, target: GPUTexture): Promise<void>;
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
  for (const d of a.draws) if (!(d.shader in a.shaders)) fail(`draw "${d.label}" names unknown shader "${d.shader}"`);
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

function pipelineKey(d: Draw): string {
  const constants = Object.keys(d.constants).sort().map((k) => `${k}=${d.constants[k]}`).join(",");
  return `${d.shader}|${d.cull}|${d.blend}|${d.depthWrite}|${constants}`;
}

async function compileShader(device: GPUDevice, name: string, code: string): Promise<GPUShaderModule> {
  device.pushErrorScope("validation");
  const module = device.createShaderModule({ label: name, code });
  const error = await device.popErrorScope();
  if (error) throw new Error(`shader "${name}" failed to compile:\n${error.message}`);
  return module;
}

async function createPipeline(device: GPUDevice, layout: GPUPipelineLayout, module: GPUShaderModule, d: Draw): Promise<GPURenderPipeline> {
  const constants = { ...d.constants };
  const blend = BLEND[d.blend];
  device.pushErrorScope("validation");
  const created = device.createRenderPipelineAsync({
    label: `${d.shader} (${d.label})`,
    layout,
    vertex: { module, entryPoint: "vs", constants },
    fragment: { module, entryPoint: "fs", constants, targets: [blend ? { format: COLOR_FORMAT, blend } : { format: COLOR_FORMAT }] },
    primitive: { topology: "triangle-list", frontFace: "ccw", cullMode: d.cull },
    depthStencil: { format: DEPTH_FORMAT, depthWriteEnabled: d.depthWrite, depthCompare: "less" },
    multisample: { count: SAMPLES },
  }).then((p) => ({ ok: true as const, p }), (e: unknown) => ({ ok: false as const, message: e instanceof Error ? e.message : String(e) }));
  const result = await created;
  const error = await device.popErrorScope();
  const message = result.ok ? error?.message : result.message;
  if (message !== undefined || !result.ok) throw new Error(`shader "${d.shader}" (draw "${d.label}"): pipeline creation failed:\n${message}`);
  return result.p;
}

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
function frameState(a: Artefact, slots: Slots, paramFloats: number, view: View): FrameState {
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
  return { time: view.time, inputs, params };
}

function frameUniform(view: View): Float32Array<ArrayBuffer> {
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

/** Render targets for one size: the 4x colour and depth attachments, plus (headless only) a resolve texture and its readback buffer. */
type Targets = { readonly width: number; readonly height: number; readonly colour: GPUTexture; readonly depth: GPUTexture };
type Readback = { readonly width: number; readonly height: number; readonly texture: GPUTexture; readonly buffer: GPUBuffer; readonly bytesPerRow: number };

const sameSize = (t: { width: number; height: number }, v: View) => t.width === v.width && t.height === v.height;

// ---- the host --------------------------------------------------------------------------------------------------

export function createHost(device: GPUDevice): GpuHost {
  return { load: (artefact) => load(device, artefact) };
}

async function load(device: GPUDevice, a: Artefact): Promise<GpuLoaded> {
  const { paramFloats } = checkArtefact(a);
  const slots: Slots = {
    inputs: new Map(a.inputs.map((s) => [s.name, { slot: s.slot, floats: FLOATS[s.ty] }])),
    params: new Map(a.params.map((p) => [p.name, p.slot])),
  };
  const STORAGE = GPUBufferUsage.STORAGE | GPUBufferUsage.COPY_DST;
  const buffer = (label: string, bytes: number, usage: number) => device.createBuffer({ label, size: Math.max(MIN_BUFFER_BYTES, bytes), usage });

  const frameBuf = buffer("frame", FRAME_FLOATS * 4, GPUBufferUsage.UNIFORM | GPUBufferUsage.COPY_DST);
  const inputBuf = buffer("inputs", a.inputFloats * 4, STORAGE);
  const paramBuf = buffer("params", paramFloats * 4, STORAGE);
  const curveBuf = buffer("curves", a.curves.byteLength, STORAGE);
  if (a.curves.length > 0) device.queue.writeBuffer(curveBuf, 0, a.curves as Float32Array<ArrayBuffer>); // Dawn warns on empty writes
  const dynamic = a.dynamic.map((d) => ({ ...d, gpu: buffer(d.name, d.vec4s * 16, STORAGE) }));

  // Explicit layouts, every binding visible to both stages (artefact.ts header): a shader may use any subset, and
  // `layout: "auto"` would drop the bindings a particular shader never reads.
  const storageEntry = (binding: number): GPUBindGroupLayoutEntry => ({ binding, visibility: VISIBILITY, buffer: { type: "read-only-storage" } });
  const layout0 = device.createBindGroupLayout({
    label: "loft prelude",
    entries: [{ binding: 0, visibility: VISIBILITY, buffer: { type: "uniform" } }, storageEntry(1), storageEntry(2), storageEntry(3)],
  });
  const layout1 = device.createBindGroupLayout({ label: "loft scene tables", entries: dynamic.map((d) => storageEntry(d.binding)) });
  const group0 = device.createBindGroup({
    layout: layout0,
    entries: [frameBuf, inputBuf, paramBuf, curveBuf].map((b, binding) => ({ binding, resource: { buffer: b } })),
  });
  const group1 = device.createBindGroup({ layout: layout1, entries: dynamic.map((d) => ({ binding: d.binding, resource: { buffer: d.gpu } })) });
  const pipelineLayout = device.createPipelineLayout({ bindGroupLayouts: [layout0, layout1] });

  const modules = new Map<string, GPUShaderModule>();
  for (const [name, code] of Object.entries(a.shaders)) modules.set(name, await compileShader(device, name, PRELUDE_WGSL + a.sceneWgsl + code));
  // Pipelines are built eagerly, so every error surfaces at load and a frame never stalls on compilation.
  const pipelines = new Map<string, GPURenderPipeline>();
  for (const d of a.draws) {
    const key = pipelineKey(d);
    if (!pipelines.has(key)) pipelines.set(key, await createPipeline(device, pipelineLayout, modules.get(d.shader)!, d));
  }
  const ordered = [...a.draws.filter((d) => d.pass === "opaque"), ...a.draws.filter((d) => d.pass === "transparent")]
    .map((d) => ({ draw: d, pipeline: pipelines.get(pipelineKey(d))! }));
  const declared = new Set(a.dynamic.map((d) => d.name));

  let targets: Targets | undefined;
  let readback: Readback | undefined;
  let reading = false;

  const targetsFor = (view: View): Targets => {
    if (targets && sameSize(targets, view)) return targets;
    targets?.colour.destroy(); targets?.depth.destroy();
    const size = { width: view.width, height: view.height };
    targets = {
      ...size,
      colour: device.createTexture({ label: "loft colour x4", size, format: COLOR_FORMAT, sampleCount: SAMPLES, usage: GPUTextureUsage.RENDER_ATTACHMENT }),
      depth: device.createTexture({ label: "loft depth x4", size, format: DEPTH_FORMAT, sampleCount: SAMPLES, usage: GPUTextureUsage.RENDER_ATTACHMENT }),
    };
    return targets;
  };
  const readbackFor = (view: View): Readback => {
    if (readback && sameSize(readback, view)) return readback;
    readback?.texture.destroy(); readback?.buffer.destroy();
    // Buffer copies of a texture need rows aligned to 256 bytes; `frame` repacks them tightly.
    const bytesPerRow = Math.ceil((view.width * 4) / 256) * 256;
    readback = {
      width: view.width, height: view.height, bytesPerRow,
      texture: device.createTexture({ label: "loft resolve", size: { width: view.width, height: view.height }, format: COLOR_FORMAT, usage: GPUTextureUsage.RENDER_ATTACHMENT | GPUTextureUsage.COPY_SRC }),
      buffer: device.createBuffer({ label: "loft readback", size: bytesPerRow * view.height, usage: GPUBufferUsage.MAP_READ | GPUBufferUsage.COPY_DST }),
    };
    return readback;
  };

  /** Pose and upload. Every `writeBuffer` precedes the one submit that reads it, so queue ordering is not a concern. */
  const upload = (view: View): void => {
    const state = frameState(a, slots, paramFloats, view);
    const tables = a.pose(state);
    for (const name of Object.keys(tables)) if (!declared.has(name)) throw new Error(`pose returned "${name}", which is not a declared dynamic buffer`);
    for (const d of dynamic) {
      const t = tables[d.name];
      if (!t) throw new Error(`pose did not return dynamic buffer "${d.name}"`);
      if (t.length !== d.vec4s * 4) throw new Error(`pose returned dynamic buffer "${d.name}" with ${t.length} floats; expected ${d.vec4s * 4} (${d.vec4s} vec4s)`);
      device.queue.writeBuffer(d.gpu, 0, t as Float32Array<ArrayBuffer>);
    }
    device.queue.writeBuffer(frameBuf, 0, frameUniform(view));
    if (state.inputs.length > 0) device.queue.writeBuffer(inputBuf, 0, state.inputs as Float32Array<ArrayBuffer>);
    if (state.params.length > 0) device.queue.writeBuffer(paramBuf, 0, state.params as Float32Array<ArrayBuffer>);
  };

  /** The shared drawing code: one render pass, opaque then transparent, resolved into `resolveTarget`. */
  const record = (encoder: GPUCommandEncoder, view: View, resolveTarget: GPUTextureView): void => {
    const t = targetsFor(view);
    const pass = encoder.beginRenderPass({
      label: "loft scene",
      colorAttachments: [{
        view: t.colour.createView(), resolveTarget, loadOp: "clear", storeOp: "discard",
        clearValue: { r: a.clear[0], g: a.clear[1], b: a.clear[2], a: 1 },
      }],
      depthStencilAttachment: { view: t.depth.createView(), depthClearValue: 1, depthLoadOp: "clear", depthStoreOp: "discard" },
    });
    pass.setBindGroup(0, group0);
    pass.setBindGroup(1, group1);
    const construction = view.construction === true;
    for (const { draw, pipeline } of ordered) {
      if (draw.layer === "construction" && !construction) continue;
      pass.setPipeline(pipeline);
      pass.draw(draw.vertices, draw.instances);
    }
    pass.end();
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
    checkView(view);
    if (reading) throw new Error("frame: called while the previous frame is still reading back; await each frame");
    reading = true;
    try {
      const t0 = performance.now();
      let t1 = t0;
      const rb = await validated("frame", () => {
        upload(view);
        t1 = performance.now();
        const rb = readbackFor(view);
        const encoder = device.createCommandEncoder({ label: "loft frame" });
        record(encoder, view, rb.texture.createView());
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
      return { pixels, ms: { pose: t1 - t0, encode: t2 - t1, readback: t3 - t2 } };
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
      await validated("draw", () => {
        upload(view);
        const encoder = device.createCommandEncoder({ label: "loft draw" });
        record(encoder, view, target.createView());
        device.queue.submit([encoder.finish()]);
      });
    },
    setCurves: (curves) => {
      if (curves.length !== a.curves.length) throw new Error(`setCurves: got ${curves.length} floats; the artefact has ${a.curves.length}`);
      if (curves.length > 0) device.queue.writeBuffer(curveBuf, 0, curves as Float32Array<ArrayBuffer>);
    },
    destroy: () => {
      for (const b of [frameBuf, inputBuf, paramBuf, curveBuf, ...dynamic.map((d) => d.gpu)]) b.destroy();
      targets?.colour.destroy(); targets?.depth.destroy();
      readback?.texture.destroy(); readback?.buffer.destroy();
      targets = undefined; readback = undefined;
    },
  };
}
