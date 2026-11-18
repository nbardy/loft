// The artefact as files (docs/artefact_format.md, docs/gpu_native.md phase 2): the host, lowered. Every WebGPU object a
// loader creates is one entry in a list below, described with WebGPU's own field names, and every reference between them
// is an index into its list. The frame is a recipe over those objects. A loader executes it; it decides nothing.
//
// This file is types and constants only, and imports nothing that evaluates: the file loader's import closure must not
// reach the compiler (tools/check_loader_closure.ts).

import type { AuthorLines, Camera, Detail, InputSlot, ParamSlot, TableFillKind, TextureSize } from "../artefact.ts";
export type { TextureSize };

export const FORMAT = "loft-artefact";
export const FORMAT_VERSION = 1;
/**
 * The versions a loader of this file reads. 2 adds what a version-1 loader would draw wrongly: `frame.replay` (a scene
 * whose state keeps checkpoints) and pictures drawn off screen (src/frame/pass.ts: textures of a fixed or scaled size,
 * render passes with no colour attachment, pipelines with no fragment stage, `frame.copies`). A file with neither is
 * written as version 1, byte for byte what it was. Each version-2 field is optional; `formatVersion` (src/host/lower.ts)
 * decides which legacy version to write. 3 adds `instanceBase` on planned draws (shared draw shaders, src/host/share.ts):
 * a version-1 or -2 loader would ignore it and draw every shared draw with another draw's numbers.
 * 4 adds mixed shader and buffer alternatives (`precision`). 5 adds author compute (`build`, `load`, `frame.compute`,
 * compute texture extents and initializers), and may also carry the version-4 `precision` overlay.
 */
export const FORMAT_VERSIONS = [1, 2, 3, 4, 5] as const;
export type FormatVersion = (typeof FORMAT_VERSIONS)[number];
export type Precision = "f32" | "mixed";
export type FramePrecision = Precision | "f32-fallback";

/** Index into the list of its kind (`Manifest.buffers`, `Manifest.textures`, ...). */
export type Id = number;

/** GPUBufferUsage / GPUTextureUsage flag names; the value is their bitwise or. */
export type BufferUsage = "MAP_READ" | "COPY_SRC" | "COPY_DST" | "UNIFORM" | "STORAGE";
export type TextureUsage = "COPY_SRC" | "COPY_DST" | "STORAGE_BINDING" | "RENDER_ATTACHMENT" | "TEXTURE_BINDING";
export type Stage = "VERTEX" | "FRAGMENT" | "COMPUTE";

/** `init`: a blob uploaded once at load, at offset 0 (a blob of 0 bytes is not written). */
export type BufferDesc = { readonly label: string; readonly size: number; readonly usage: readonly BufferUsage[]; readonly init?: string };
/**
 * Created at the first frame and again whenever the view's width or height changes. `size` absent: the view's size;
 * `view`: the view's size times `scale`, each side rounded to the nearest pixel and at least 1; `fixed`: those pixels.
 */
export type TextureDesc = {
  readonly label: string; readonly size?: TextureSize; readonly format: GPUTextureFormat; readonly sampleCount: number; readonly usage: readonly TextureUsage[];
  /** Fixed compute texture, allocated at load. A two-element extent is 2D, a three-element extent is 3D. */
  readonly extent?: readonly [number, number] | readonly [number, number, number];
  /** Tightly packed texels, x fastest, then y, then z. */
  readonly init?: string;
};
/** A history stacks 2D states down the image and 3D states along z, keeping copy dimensions equal. */
export function computeHistorySize(size: NonNullable<TextureDesc["extent"]>, states: number): NonNullable<TextureDesc["extent"]> {
  return size.length === 2 ? [size[0], size[1] * states] : [size[0], size[1], size[2] * states];
}
export const COMPUTE_TEXEL_BYTES = { r32float: 4, rgba8unorm: 4, rgba16float: 8, rgba32float: 16 } as const;
export type SamplerDesc = {
  readonly label: string; readonly magFilter: GPUFilterMode; readonly minFilter: GPUFilterMode;
  readonly addressModeU: GPUAddressMode; readonly addressModeV: GPUAddressMode;
};

export type LayoutEntry =
  | { readonly binding: number; readonly visibility: readonly Stage[]; readonly buffer: { readonly type: GPUBufferBindingType } }
  | { readonly binding: number; readonly visibility: readonly Stage[]; readonly texture: { readonly sampleType: GPUTextureSampleType; readonly viewDimension: "2d" | "3d"; readonly multisampled: boolean } }
  | { readonly binding: number; readonly visibility: readonly Stage[]; readonly storageTexture: { readonly access: "write-only"; readonly format: GPUTextureFormat; readonly viewDimension: "2d" | "3d" } }
  | { readonly binding: number; readonly visibility: readonly Stage[]; readonly sampler: { readonly type: GPUSamplerBindingType } };
export type BindGroupLayoutDesc = { readonly label: string; readonly entries: readonly LayoutEntry[] };
export type PipelineLayoutDesc = { readonly label: string; readonly bindGroupLayouts: readonly Id[] };

/** A shader module's code: its chunks' texts concatenated in order, nothing between them. `chunks` index `Manifest.chunks`. */
export type ModuleDesc = { readonly label: string; readonly chunks: readonly Id[] };

export type ComputePipelineDesc = { readonly label: string; readonly layout: Id; readonly module: Id; readonly entryPoint: string };
export type ShaderStageDesc = { readonly module: Id; readonly entryPoint: string; readonly constants: Readonly<Record<string, number>> };
export type RenderPipelineDesc = {
  readonly label: string;
  readonly layout: Id;
  readonly vertex: ShaderStageDesc;
  /** Absent: no fragment stage (a depth pass's surfaces, whose rasterised triangles are the depth). */
  readonly fragment?: ShaderStageDesc & { readonly targets: readonly { readonly format: GPUTextureFormat; readonly blend?: GPUBlendState }[] };
  readonly primitive: { readonly topology: "triangle-list"; readonly frontFace: GPUFrontFace; readonly cullMode: GPUCullMode };
  readonly depthStencil?: { readonly format: GPUTextureFormat; readonly depthWriteEnabled: boolean; readonly depthCompare: GPUCompareFunction };
  readonly multisample: { readonly count: number };
};

/** A texture is bound through its default view. A bind group that names a texture is re-created with the textures. */
export type BindGroupEntry =
  | { readonly binding: number; readonly buffer: Id }
  | { readonly binding: number; readonly texture: Id }
  | { readonly binding: number; readonly sampler: Id };
export type BindGroupDesc = { readonly label: string; readonly layout: Id; readonly entries: readonly BindGroupEntry[] };

/**
 * The per-frame CPU work, all of it (docs/artefact_format.md says how each block is computed). Written with
 * `queue.writeBuffer` at offset 0, before the frame's compute pass is submitted.
 */
export type FrameWrite =
  /** 48 floats: view-projection, its inverse, eye, forward, viewport, time (PRELUDE_WGSL `struct Frame`). */
  | { readonly kind: "frame"; readonly buffer: Id }
  /** `Manifest.inputFloats` floats: every input's default at its slot, the view's values over them, the view's time in `time`. */
  | { readonly kind: "inputs"; readonly buffer: Id }
  /** `Manifest.paramFloats` floats: every param's default at its slot, the view's values over them. */
  | { readonly kind: "params"; readonly buffer: Id };

/** A dispatch of the pose pass: x workgroups of POSE_WORKGROUP invocations, bind groups set at 0, 1, ... */
export type Dispatch = { readonly label: string; readonly pipeline: Id; readonly bindGroups: readonly Id[]; readonly workgroups: number | readonly [number, number, number] };

export type RawDispatch = {
  readonly run: Dispatch;
  readonly clearBuffers?: readonly Id[];
  readonly clearTextures?: readonly { readonly from: Id; readonly to: Id }[];
};
export type ComputeRun =
  | ({ readonly kind: "once" } & RawDispatch)
  | {
    readonly kind: "steps";
    readonly start: RawDispatch;
    readonly odd: RawDispatch;
    readonly even: RawDispatch;
    readonly count: number;
    readonly keep: number;
    readonly history: readonly StepCopy[];
  };
export type StepCopy = { readonly a: Id; readonly b: Id; readonly to: Id } &
  ({ readonly kind: "buffer"; readonly bytes: number } | { readonly kind: "texture" });
export type BuildOutput = { readonly name: string } & ({ readonly kind: "buffer"; readonly id: Id; readonly bytes: number } | { readonly kind: "texture"; readonly id: Id });

/** `bytes` copied from `from` at byte `fromOffset` to `to` at byte 0 (copyBufferToBuffer), after the pose pass. */
export type BufferCopy = { readonly from: Id; readonly fromOffset: number; readonly to: Id; readonly bytes: number };
/**
 * A recurrence that keeps checkpoints (docs/artefact_format.md, "Replays"): each frame, from the view's time t, the
 * loader computes n = floor((t - start) rate) held in [0, steps - 1] (in f64) and k = n mod every, writes n as an f32 at
 * float `slot` of the inputs, and records `first` then k more dispatches alternating `ping` (odd) and `pong` (even) in
 * the pose pass, before its dispatches.
 */
export type Replay = {
  readonly label: string; readonly slot: number;
  readonly rate: number; readonly start: number; readonly steps: number; readonly every: number;
  readonly first: Dispatch; readonly ping: Dispatch; readonly pong: Dispatch;
};

/** Where the level-of-detail bounds come from: none (no grid draws), or copies into a buffer mapped after the pose. */
export type BoundsRead =
  | { readonly kind: "none" }
  | {
    readonly kind: "read";
    /** MAP_READ | COPY_DST. Grid after grid, one vec4 per instance. */
    readonly buffer: Id;
    /** Each grid slot's bounds: `instances` vec4s copied from `from` at byte `fromOffset` to byte `at * 16` of `buffer`. */
    readonly grids: readonly { readonly slot: number; readonly from: Id; readonly fromOffset: number; readonly at: number; readonly instances: number }[];
  };

/** An attachment: a texture of the manifest, or the frame's output (the caller's texture, COLOR_FORMAT). */
export type Target = { readonly kind: "texture"; readonly texture: Id } | { readonly kind: "output" };

export type ColorAttachment = {
  readonly view: Target; readonly resolveTarget?: Target;
  readonly loadOp: GPULoadOp; readonly storeOp: GPUStoreOp; readonly clearValue?: readonly [number, number, number, number];
};
export type DepthAttachment = { readonly view: Id; readonly depthLoadOp: GPULoadOp; readonly depthStoreOp: GPUStoreOp; readonly depthClearValue?: number };

/** A draw in a render pass: fixed counts, or the counts the level-of-detail plan chose for `Manifest.plan[draw]`. */
export type DrawCommand =
  | { readonly kind: "fixed"; readonly pipeline: Id; readonly vertices: number; readonly instances: number }
  /**
   * For each run of the plan for that draw: draw(run.vertices, run.instances, 0, instanceBase + run.first). No run:
   * nothing, not even setPipeline. `instanceBase` (version 3) picks a shared shader's row for this draw
   * (src/host/share.ts); absent means 0, so a file that shares nothing is written as it was.
   */
  | { readonly kind: "planned"; readonly pipeline: Id; readonly draw: number; readonly instanceBase?: number };

export type RenderPassDesc = {
  readonly label: string;
  /** Absent: no colour attachment (a depth pass). */
  readonly color?: ColorAttachment;
  readonly depth?: DepthAttachment;
  /** Set at group 0, 1, ... once, before the commands. */
  readonly bindGroups: readonly Id[];
  readonly commands: readonly DrawCommand[];
};

/** What a level-of-detail plan reads of a draw (`Detail` is src/artefact.ts's, measured at build). */
export type PlanDraw = { readonly label: string; readonly layer: "scene" | "construction"; readonly vertices: number; readonly instances: number; readonly detail: Detail };

/** A per-frame table, for tools that read the tables back: its bytes in a dynamic buffer, and how it is filled. */
export type TableEntry = { readonly name: string; readonly buffer: Id; readonly offset: number; readonly vec4s: number; readonly lanes: number; readonly fill: TableFillKind };

export type Manifest = {
  readonly format: typeof FORMAT;
  readonly version: FormatVersion;
  /** Version 4 (or 5) overlays the f32 descriptors when the device grants shader-f16. */
  readonly precision?: {
    readonly kind: "mixed";
    readonly fallback: "f32-fallback";
    readonly modules: Readonly<Record<number, readonly Id[]>>;
    readonly buffers: Readonly<Record<number, { readonly size: number; readonly init: string }>>;
  };
  /** Chunk names (a hash of the text); a file loader reads `wgsl/<name>.wgsl`. */
  readonly chunks: readonly string[];
  /**
   * The lines of a chunk the scene's author wrote (raw functions of `wgsl` text), by chunk name; absent, and a chunk
   * absent from it, the library wrote all of it. A host names the author's line when the GPU refuses one of these.
   */
  readonly authored?: Readonly<Record<string, readonly AuthorLines[]>>;
  /** Blob names; a file loader reads `bin/<name>.bin` (little-endian data). */
  readonly blobs: readonly { readonly name: string; readonly bytes: number }[];

  readonly inputs: readonly InputSlot[];
  readonly inputFloats: number;
  readonly params: readonly ParamSlot[];
  readonly paramFloats: number;
  readonly cameras: Readonly<Record<string, Camera>>;
  /** The curve rows' buffer and its float count (a host may replace its contents: `setCurves`). */
  readonly curves: { readonly buffer: Id; readonly floats: number };

  readonly buffers: readonly BufferDesc[];
  readonly textures: readonly TextureDesc[];
  readonly samplers: readonly SamplerDesc[];
  readonly bindGroupLayouts: readonly BindGroupLayoutDesc[];
  readonly pipelineLayouts: readonly PipelineLayoutDesc[];
  readonly modules: readonly ModuleDesc[];
  readonly computePipelines: readonly ComputePipelineDesc[];
  readonly renderPipelines: readonly RenderPipelineDesc[];
  readonly bindGroups: readonly BindGroupDesc[];

  /** Build-only WGSL, emitted by the GPU-less child and consumed by the GPU owner. Absent in a finished build. */
  readonly build?: { readonly runs: readonly RawDispatch[]; readonly outputs: readonly BuildOutput[] };
  readonly load?: readonly ComputeRun[];
  readonly frame: {
    readonly writes: readonly FrameWrite[];
    /** One compute pass, in order; empty: no pass. */
    readonly pose: readonly Dispatch[];
    readonly compute?: readonly RawDispatch[];
    /** Version 2: replays recorded in the pose pass before `pose`. Absent means none. */
    readonly replay?: readonly Replay[];
    readonly bounds: BoundsRead;
    /** `loft_detail`: the plan's slots, `floats` of them written after the plan. */
    readonly detail: { readonly buffer: Id; readonly floats: number };
    readonly passes: readonly RenderPassDesc[];
    /** After the pose pass, in the same encoder, in order: each pass's camera into its own frame uniform. Absent: none. */
    readonly copies?: readonly BufferCopy[];
  };
  /** One entry per planned draw (`DrawCommand` "planned" indexes it). */
  readonly plan: readonly PlanDraw[];
  /** The buffers the per-frame tables live in (bind group 1's dynamic buffers), in binding order; buffer labels are their names. */
  readonly dynamic: readonly Id[];
  /** Every per-frame table. A file holds no `cpu` table (`loft build` refuses them by name). */
  readonly tables: readonly TableEntry[];
  readonly stats: { readonly maxTriangles: number; readonly draws: number; readonly curveRows: number; readonly stations: number };
};

/** A manifest with the texts and bytes it names: what `loft build` writes and a loader reads back. */
export type Bundle = {
  readonly manifest: Manifest;
  readonly chunks: ReadonlyMap<string, string>;
  readonly blobs: ReadonlyMap<string, Uint8Array>;
};

/** The output's format: a canvas drawn through `draw`, and the readback of `frame`, use it. */
export const COLOR_FORMAT: GPUTextureFormat = "rgba8unorm";
/** Formats and the sample count `lower` chooses (a loader reads them from the descriptors, not from here). */
export const DEPTH_FORMAT: GPUTextureFormat = "depth32float";
export const SCENE_DEPTH_FORMAT: GPUTextureFormat = "r32float";
/** A pass that keeps light (values above 1), and a screen pass. */
export const LIGHT_FORMAT: GPUTextureFormat = "rgba16float";
export const SAMPLES = 4;
