// `lower(artefact)`: the host's load-time decisions, made once and written down as WebGPU descriptors (src/host/manifest.ts,
// docs/artefact_format.md). Both the in-memory path (`createHost(device).load(artefact)`) and `loft build` go through
// here, and the executor (src/host/host.ts) runs the result; so a compiled scene and its files draw with the same
// objects. Every structural check on an artefact happens here, once; the executor trusts what passes.
//
// The passes, decided here from the artefact:
//   plain   no draw reads scene depth and there is no post: ONE multisampled pass, opaque then transparent, resolved
//           into the output.
//   split   some transparent draw sets `sceneDepth`, or there are post passes:
//             1. opaque pass: 4x colour and 4x depth, both stored, no resolve
//             2. depth resolve: a full-screen pass reads the 4x depth as texture_depth_multisampled_2d and writes the
//                nearest of each pixel's samples to a single-sampled r32float texture (SCENE_DEPTH_WGSL, group 2)
//          2a-2c. the clearance map (SCENE_CLEARANCE_WGSL): three passes drawing one fragment per 4 x 4 or 8 x 8 pixels
//                into the top left of view-sized textures: whether a cell holds an opaque pixel, then along each row
//                and down each column the distance to the nearest tile that does (docs/gpu_native.md "The sea")
//             3. transparent pass: loads the stored colour and depth (so transparent draws still depth-test, and may
//                still write depth), binds the resolved depth as group 2, resolves colour into the output, or into
//                the first intermediate when there are post passes
//             4. post passes, ping-ponging between two single-sampled intermediates; the last writes the output.
// Why a resolve into a separate texture rather than sampling the depth attachment itself during the transparent
// pass: sampling an attachment needs it read-only, which would forbid transparent draws that write depth; a
// multisampled depth binding would make every reader pick or average samples itself; and post passes need a
// single-sampled depth after the scene pass has ended anyway.
//
// Imports no bun-only module (the browser host uses it).

import { CLEARANCE_MAX, CLEARANCE_SCALE, CLEARANCE_TILE, CURVE_N, DETAIL_SLOTS, PASS_CAMERA_VEC4S, POSE_WORKGROUP, POST_WGSL, PRELUDE_WGSL, SCENE_CLEARANCE_WGSL, SCENE_DEPTH_WGSL, passBindingsWgsl, type Artefact, type ArtefactPass, type Draw, type DrawDepth, type TextureSize } from "../artefact.ts";
import { gridBounds, slotsUsed } from "./detail.ts";
import { ROWS_BINDING, share } from "./share.ts";
import {
  COLOR_FORMAT, DEPTH_FORMAT, FORMAT, FORMAT_VERSION, LIGHT_FORMAT, SAMPLES, SCENE_DEPTH_FORMAT,
  type BindGroupDesc, type BindGroupLayoutDesc, type BufferCopy, type BufferDesc, type Bundle, type ComputePipelineDesc, type Dispatch, type DrawCommand, type FormatVersion, type Id, type LayoutEntry, type Replay,
  type Manifest, type Precision, type ModuleDesc, type PipelineLayoutDesc, type RenderPassDesc, type RenderPipelineDesc, type SamplerDesc, type Stage, type TextureDesc,
} from "./manifest.ts";

const FLOATS = { f32: 1, vec2: 2, vec3: 3, vec4: 4 } as const;
/** Frame uniform: two mat4 then four vec4 (PRELUDE_WGSL `struct Frame`). */
export const FRAME_FLOATS = 16 + 16 + 4 * 4;
/** WebGPU rejects zero-sized bindings; an artefact with no inputs or params still binds a buffer of one vec4. */
const MIN_BUFFER_BYTES = 16;
const DRAWN: readonly Stage[] = ["VERTEX", "FRAGMENT"];
const ALL: readonly Stage[] = ["VERTEX", "FRAGMENT", "COMPUTE"];

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

/** cover_vs draws one full-screen triangle: these many vertices, in every clearance pass (view and colour passes). */
const CLEARANCE_VERTICES = 3;

/**
 * The clearance map's passes (SCENE_CLEARANCE_WGSL), one module with entry points per pass. Each covers a rectangle
 * at the top left of its view-sized target (sized from the input texture, which is view-sized too), one fragment per
 * cell of the pass's grid; the rest of the target keeps its clear value. The rectangle is a full-screen triangle whose
 * fragments past the grid are discarded, not two triangles sized in the vertex stage: the same fragments, and a
 * texture query in a vertex stage cost Metal about 70 ms per pipeline to compile (measured 2026-11-15, three per load).
 *   quads: per 4 x 4 pixels, 1 where any of the 16 depths is an opaque surface (r8unorm)
 *   rows:  per 8 x 8 tile, along its row, the offset in tiles to the nearest tile holding an opaque pixel within 7, 8
 *          for none (r8unorm, k / 255; one gather of a tile's four quads per tile looked at)
 *   clear: down the column, min over offsets dy within 7 of sqrt(gap(k)^2 + gap(dy)^2), gap(d) = max(0, 8 |d| - 7) the
 *          gap between two tiles' nearest pixel centres d tiles apart, capped at CLEARANCE_MAX, stored divided by
 *          CLEARANCE_SCALE (r8unorm). The minimum over a column of row minima is the 2D minimum: the Euclidean
 *          distance transform is separable (Felzenszwalb and Huttenlocher), 30 reads per tile instead of 225.
 * A tile beyond 7 has a gap of at least 8 * 8 - 7 = 57, over the cap, so the bound holds without looking farther.
 * Reads are branch-free and independent, so they issue together, and clamped (a cell at the image's edge reads its own
 * last texels again; a tile read through a clamped index is counted at the offset asked for, never nearer than its
 * own, and is read at its own offset too). Measured: with one fragment per 8 x 8 tile reading 64 depths the first pass
 * was the largest (16,000 fragments too few to hide the reads' latency); per 4 x 4, four times as many fragments.
 */
const CLEARANCE_WGSL = /* wgsl */ `
@group(0) @binding(0) var src: texture_2d<f32>;
@group(0) @binding(1) var near: sampler;
const TILE: i32 = ${CLEARANCE_TILE};
const QUAD: i32 = ${CLEARANCE_TILE / 2};
const REACH: i32 = 7;
fn view() -> vec2<i32> { return vec2<i32>(textureDimensions(src)); }
fn cells(side: i32) -> vec2<i32> { return (view() + side - 1) / side; }
fn tiles() -> vec2<i32> { return cells(TILE); }
fn gap(d: i32) -> f32 { return max(0.0, f32(TILE * abs(d) - (TILE - 1))); }
@vertex fn cover_vs(@builtin(vertex_index) vi: u32) -> @builtin(position) vec4<f32> {
  let uv = vec2<f32>(f32((vi << 1u) & 2u), f32(vi & 2u));
  return vec4<f32>(uv.x * 2.0 - 1.0, 1.0 - uv.y * 2.0, 0.0, 1.0);
}
@fragment fn quads_fs(@builtin(position) p: vec4<f32>) -> @location(0) vec4<f32> {
  if (any(vec2<i32>(floor(p.xy)) >= cells(QUAD))) { discard; }
  // Four gathers of 2 x 2 depths, at the corners between them; clamp-to-edge repeats the image's last texels.
  let o = floor(p.xy) * f32(QUAD);
  let size = vec2<f32>(view());
  let m = min(min(textureGather(0, src, near, (o + vec2<f32>(1.0, 1.0)) / size), textureGather(0, src, near, (o + vec2<f32>(3.0, 1.0)) / size)),
              min(textureGather(0, src, near, (o + vec2<f32>(1.0, 3.0)) / size), textureGather(0, src, near, (o + vec2<f32>(3.0, 3.0)) / size)));
  return vec4<f32>(select(0.0, 1.0, min(min(m.x, m.y), min(m.z, m.w)) < 1.0), 0.0, 0.0, 1.0);
}
@fragment fn rows_fs(@builtin(position) p: vec4<f32>) -> @location(0) vec4<f32> {
  let t = vec2<i32>(floor(p.xy));
  if (any(t >= tiles())) { discard; }
  let last = tiles() - 1;
  let size = vec2<f32>(view());
  var k = REACH + 1;
  for (var dx = -REACH; dx <= REACH; dx++) {
    // Tile s's four quads (quads past the image's edge were never drawn and read 0).
    let s = clamp(t + vec2<i32>(dx, 0), vec2<i32>(0), last);
    let q = textureGather(0, src, near, (vec2<f32>(s * 2) + 1.0) / size);
    let opaque = max(max(q.x, q.y), max(q.z, q.w)) > 0.5;
    k = min(k, select(REACH + 1, abs(dx), opaque));
  }
  // The tile offset, an integer, exact in 8 bits.
  return vec4<f32>(f32(k) / 255.0, 0.0, 0.0, 1.0);
}
@fragment fn clear_fs(@builtin(position) p: vec4<f32>) -> @location(0) vec4<f32> {
  let t = vec2<i32>(floor(p.xy));
  if (any(t >= tiles())) { discard; }
  let last = tiles() - 1;
  var d2 = 1e8;
  for (var dy = -REACH; dy <= REACH; dy++) {
    let k = i32(round(textureLoad(src, clamp(t + vec2<i32>(0, dy), vec2<i32>(0), last), 0).x * 255.0));
    let gx = gap(k);
    d2 = min(d2, select(1e8, gx * gx + gap(dy) * gap(dy), k <= REACH));
  }
  return vec4<f32>(min(sqrt(d2), ${CLEARANCE_MAX}.0) / ${CLEARANCE_SCALE}.0, 0.0, 0.0, 1.0);
}
`;

const BLEND: Record<Draw["blend"], GPUBlendState | undefined> = {
  none: undefined,
  // Straight alpha over.
  alpha: { color: { srcFactor: "src-alpha", dstFactor: "one-minus-src-alpha" }, alpha: { srcFactor: "one", dstFactor: "one-minus-src-alpha" } },
  // Additive, scaled by the fragment's alpha (straight alpha, like `alpha`); the target's alpha is left as it is.
  add: { color: { srcFactor: "src-alpha", dstFactor: "one" }, alpha: { srcFactor: "zero", dstFactor: "one" } },
  // A shadow block's tone, lowered: colour untouched, alpha the lesser (WebGPU wants both factors "one" with min).
  "alpha-min": { color: { srcFactor: "zero", dstFactor: "one" }, alpha: { operation: "min", srcFactor: "one", dstFactor: "one" } },
  // The block printed: the target's alpha is how much of its colour stays; alpha is then the fragment's.
  "alpha-pull": { color: { srcFactor: "one-minus-dst-alpha", dstFactor: "dst-alpha" }, alpha: { srcFactor: "one", dstFactor: "zero" } },
};

/** How a pass's picture is held, and how group 3 binds it (src/artefact.ts `passBindingsWgsl`). */
type PassTexture = "colour" | "depth" | "light" | "screen";
const PASS_TEXTURE: Readonly<Record<PassTexture, { readonly format: GPUTextureFormat; readonly sampleType: "float" | "unfilterable-float" }>> = {
  colour: { format: COLOR_FORMAT, sampleType: "float" },
  light: { format: LIGHT_FORMAT, sampleType: "float" },
  screen: { format: LIGHT_FORMAT, sampleType: "float" },
  depth: { format: DEPTH_FORMAT, sampleType: "unfilterable-float" },
};
const keepOf = (p: ArtefactPass): PassTexture => (p.what.kind === "scene" ? p.what.keep : "screen");
const ONE_PIXEL: TextureSize = { kind: "fixed", width: 1, height: 1 };

/** Which host WGSL a shader module is compiled with. A shader used in two ways is compiled once per way. */
type Variant = "draw" | "sceneDepth" | "post";
const VARIANT: Record<Variant, { readonly wgsl: string; readonly named: string }> = {
  draw: { wgsl: "", named: "" },
  sceneDepth: { wgsl: SCENE_DEPTH_WGSL + SCENE_CLEARANCE_WGSL, named: " (with SCENE_DEPTH_WGSL and SCENE_CLEARANCE_WGSL)" },
  post: { wgsl: POST_WGSL, named: " (as a post pass, with POST_WGSL)" },
};
const drawVariant = (d: Draw): Variant => (d.sceneDepth === true ? "sceneDepth" : "draw");

/** Every structural check on an artefact; returns the params' float count. */
export function checkArtefact(a: Artefact): { paramFloats: number } {
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
  // The baked states are the truth the files carry: an unbaked artefact would write zeros (src/host/bake.ts bakes).
  for (const r of a.recurrences ?? []) if (r.baked.done === undefined) fail(`the recurrence "${r.name}" is not baked; bake it on a GPU first (bakeRecurrences, which createHost's load runs)`);
  for (const st of a.stages) if (!(Number.isInteger(st.invocations) && st.invocations >= 1)) fail(`pose stage "${st.label}" has ${st.invocations} invocations`);
  checkPasses(a, fail);
  const sizes = new Map(a.dynamic.map((d) => [d.name, d.vec4s]));
  for (const t of a.tables) {
    const size = sizes.get(t.buffer);
    if (size === undefined) fail(`table "${t.name}" lives in "${t.buffer}", which is not a dynamic buffer`);
    if (t.offset < 0 || t.offset + t.vec4s > size!) fail(`table "${t.name}" overruns "${t.buffer}"`);
  }
  return { paramFloats };
}

/**
 * The file's format version, decided here and nowhere else (docs/artefact_format.md, "Versions"): 2 when it carries what
 * a version-1 loader would draw wrongly (a replay, a pass), else 1, so a file with neither is byte for byte what it was;
 * 3 when a draw command carries an `instanceBase` (shared draw shaders), which a loader for 1 or 2 would ignore.
 */
export function formatVersion(has: { readonly replay: boolean; readonly passes: boolean; readonly shared: boolean }): FormatVersion {
  if (has.shared) return 3;
  return has.replay || has.passes ? 2 : FORMAT_VERSION;
}

/** The passes' structure: names, what each draws, where its camera lives, and that it reads only passes before it. */
function checkPasses(a: Artefact, fail: (msg: string) => never): void {
  const before = new Set<string>();
  for (const p of a.passes ?? []) {
    if (!/^[A-Za-z][A-Za-z0-9_]*$/.test(p.name)) fail(`pass "${p.name}": a name must be letters, digits and _`);
    if (before.has(p.name)) fail(`pass "${p.name}" listed twice`);
    for (const r of p.reads) if (!before.has(r)) fail(`pass "${p.name}" reads "${r}", which is not drawn before it`);
    const w = p.what;
    switch (w.kind) {
      case "screen": if (!(w.shader in a.shaders)) fail(`pass "${p.name}" names unknown shader "${w.shader}"`); break;
      case "scene": {
        const buf = a.dynamic.find((b) => b.name === w.camera.buffer);
        if (!buf || w.camera.offset + PASS_CAMERA_VEC4S > buf.vec4s) fail(`pass "${p.name}": its camera is not a table of ${PASS_CAMERA_VEC4S} vec4s in a dynamic buffer`);
        for (const k of w.draws) {
          const d = a.draws[k];
          if (d === undefined) fail(`pass "${p.name}" draws draw ${k}, which does not exist`);
          if (w.keep === "depth" && (d!.depth === undefined || d!.pass !== "opaque")) fail(`pass "${p.name}" keeps depth but draw "${d!.label}" is not opaque with a depth entry`);
        }
        break;
      }
    }
    before.add(p.name);
  }
}

/**
 * A chunk's name: 64 bits of two FNV-1a hashes of its UTF-16 code units, hex. Only names within one artefact must
 * differ, and `lower` checks that two different texts never share a name. Pure JavaScript (the browser host uses it).
 */
function chunkName(text: string): string {
  let a = 0x811c9dc5, b = 0x050c5d1f;
  for (let i = 0; i < text.length; i++) {
    const c = text.charCodeAt(i);
    a = Math.imul(a ^ c, 0x01000193);
    b = Math.imul(b ^ c, 0x01000193) ^ (b >>> 15);
  }
  return (a >>> 0).toString(16).padStart(8, "0") + (b >>> 0).toString(16).padStart(8, "0");
}

/** A list that hands out ids. */
class List<T> {
  readonly items: T[] = [];
  add(x: T): Id { return this.items.push(x) - 1; }
}

export function lower(a: Artefact, precision: Precision = "f32"): Bundle {
  const mixed = precision === "mixed" ? a.mixed : undefined;
  const mixedModules: Record<number, readonly Id[]> = {};
  const mixedBuffers: Record<number, { size: number; init: string }> = {};
  const { paramFloats } = checkArtefact(a);
  const post = a.post ?? []; // absent means none (artefact.ts)
  const sharing = share(a);
  const statics = a.statics ?? [];

  const chunkNames: string[] = [], chunkIds = new Map<string, Id>(), chunkText = new Map<string, string>();
  const chunk = (text: string): Id => {
    const name = chunkName(text);
    const known = chunkIds.get(name);
    if (known !== undefined) {
      if (chunkText.get(name) !== text) throw new Error(`lower: two different WGSL texts hash to chunk ${name}`);
      return known;
    }
    chunkText.set(name, text);
    chunkIds.set(name, chunkNames.length);
    return chunkNames.push(name) - 1;
  };
  const blobs = new Map<string, Uint8Array>();
  const blob = (name: string, data: Float32Array): string => {
    blobs.set(name, new Uint8Array(data.buffer, data.byteOffset, data.byteLength));
    return name;
  };

  const buffers = new List<BufferDesc>(), textures = new List<TextureDesc>(), samplers = new List<SamplerDesc>();
  const bgls = new List<BindGroupLayoutDesc>(), pls = new List<PipelineLayoutDesc>(), modules = new List<ModuleDesc>();
  const computes = new List<ComputePipelineDesc>(), renders = new List<RenderPipelineDesc>(), groups = new List<BindGroupDesc>();
  const buffer = (label: string, bytes: number, usage: BufferDesc["usage"], init?: string): Id =>
    buffers.add({ label, size: Math.max(MIN_BUFFER_BYTES, bytes), usage, ...(init === undefined ? {} : { init }) });

  // ---- buffers --------------------------------------------------------------------------------------------------
  const STORAGE = ["STORAGE", "COPY_DST"] as const;
  const frameBuf = buffer("frame", FRAME_FLOATS * 4, ["UNIFORM", "COPY_DST"]);
  const inputBuf = buffer("inputs", a.inputFloats * 4, STORAGE);
  const paramBuf = buffer("params", paramFloats * 4, STORAGE);
  const curveBuf = buffer("curves", a.curves.byteLength, STORAGE, a.curves.length > 0 ? blob("curves", a.curves) : undefined);
  const detailBuf = buffer("loft_detail", DETAIL_SLOTS * 16, ["UNIFORM", "COPY_DST"]);
  // Dynamic buffers are also copied from: the level-of-detail bounds and `readTables` read them back.
  const dynamic = a.dynamic.map((d) => ({ binding: d.binding, name: d.name, id: buffer(d.name, d.vec4s * 16, ["STORAGE", "COPY_DST", "COPY_SRC"]) }));
  const staticBufs = statics.map((d) => ({ binding: d.binding, id: buffer(d.name, d.data.byteLength, STORAGE, blob(d.name, d.data)) }));
  for (const [i, d] of statics.entries()) {
    const data = mixed?.statics[d.name];
    if (data === undefined) continue;
    const init = `${d.name}.f16`;
    blobs.set(init, data);
    mixedBuffers[staticBufs[i]!.id] = { size: Math.max(MIN_BUFFER_BYTES, data.byteLength), init };
  }
  const group1Buffers = [...dynamic, ...staticBufs];
  // Shared shaders' rows (src/host/share.ts): bound at group 0 only when some shader is shared.
  const rowsBuf = sharing.rows.length > 0 ? [buffer("loft rows", sharing.rows.byteLength, STORAGE, blob("loft rows", sharing.rows))] : [];
  const bufferOf = new Map(dynamic.map((d) => [d.name, d.id]));

  // ---- layouts ----------------------------------------------------------------------------------------------------
  // Explicit layouts, every binding visible to both stages (artefact.ts header): a shader may use any subset, and
  // `layout: "auto"` would drop the bindings a particular shader never reads. Group 0 is also bound to the pose stages.
  const storage = (binding: number, visibility: readonly Stage[]): LayoutEntry => ({ binding, visibility, buffer: { type: "read-only-storage" } });
  const layout0 = bgls.add({
    label: "loft prelude",
    entries: [{ binding: 0, visibility: ALL, buffer: { type: "uniform" } }, storage(1, ALL), storage(2, ALL), storage(3, ALL), { binding: 4, visibility: ALL, buffer: { type: "uniform" } },
      ...rowsBuf.map(() => storage(ROWS_BINDING, ALL))],
  });
  const layout1 = bgls.add({ label: "loft scene tables", entries: group1Buffers.map((d) => storage(d.binding, DRAWN)) });
  // The pose stages' group 1: the dynamic buffers writable, the static ones read-only (PoseStage).
  const poseLayout1 = bgls.add({
    label: "loft pose tables",
    entries: [...dynamic.map((d): LayoutEntry => ({ binding: d.binding, visibility: ["COMPUTE"], buffer: { type: "storage" } })), ...staticBufs.map((d) => storage(d.binding, ["COMPUTE"]))],
  });
  const group0 = groups.add({ label: "loft prelude", layout: layout0, entries: [frameBuf, inputBuf, paramBuf, curveBuf, detailBuf, ...rowsBuf].map((b, binding) => ({ binding, buffer: b })) });
  const group1 = groups.add({ label: "loft scene tables", layout: layout1, entries: group1Buffers.map((d) => ({ binding: d.binding, buffer: d.id })) });
  const poseGroup1 = groups.add({ label: "loft pose tables", layout: poseLayout1, entries: group1Buffers.map((d) => ({ binding: d.binding, buffer: d.id })) });

  const split = post.length > 0 || a.draws.some((d) => d.sceneDepth === true);
  // Group 2 layouts: exactly the ones documented on SCENE_DEPTH_WGSL and POST_WGSL; only a split frame has them.
  const sceneDepthEntry: LayoutEntry = { binding: 0, visibility: DRAWN, texture: { sampleType: "unfilterable-float", viewDimension: "2d", multisampled: false } };
  const clearanceEntry: LayoutEntry = { binding: 1, visibility: DRAWN, texture: { sampleType: "unfilterable-float", viewDimension: "2d", multisampled: false } };
  const depthLayout = split ? bgls.add({ label: "loft scene depth", entries: [sceneDepthEntry, clearanceEntry] }) : -1;
  const postLayout = split ? bgls.add({
    label: "loft post",
    entries: [
      sceneDepthEntry,
      { binding: 1, visibility: DRAWN, texture: { sampleType: "float", viewDimension: "2d", multisampled: false } },
      { binding: 2, visibility: DRAWN, sampler: { type: "filtering" } },
    ],
  }) : -1;
  // A scene that reads passes binds them as group 3 of every draw and post pass (src/artefact.ts `passBindingsWgsl`).
  // Pipeline layouts number their groups without gaps, so a draw with nothing in group 2 takes an empty layout there.
  const passes = a.passes ?? []; // absent means none (artefact.ts)
  const withPasses = passes.length > 0;
  const passLayout = withPasses ? bgls.add({
    label: "loft passes",
    entries: [
      { binding: 0, visibility: DRAWN, sampler: { type: "filtering" } },
      ...passes.map((p, i): LayoutEntry => ({ binding: 1 + i, visibility: DRAWN, texture: { sampleType: PASS_TEXTURE[keepOf(p)].sampleType, viewDimension: "2d", multisampled: false } })),
    ],
  }) : -1;
  // Group 2 of a pipeline that reads no scene depth, in a scene with passes: in a split frame the scene depth's layout
  // (so the transparent pass, which binds the scene depth at 2, can draw it), else an empty one. Every pass binds a
  // group of that layout at 2 (the view's scene depth, or an empty group), read by nothing that it draws.
  const emptyLayout = withPasses && !split ? bgls.add({ label: "loft none", entries: [] }) : -1;
  const plainGroup2 = split ? depthLayout : emptyLayout;
  const pipelineLayout = new Map<Variant, Id>();
  const layoutOf = (v: Variant): Id => {
    const known = pipelineLayout.get(v);
    if (known !== undefined) return known;
    const g3 = withPasses ? [passLayout] : [];
    const third: Record<Variant, Id[]> = { draw: withPasses ? [plainGroup2, passLayout] : [], sceneDepth: [depthLayout, ...g3], post: [postLayout, ...g3] };
    const id = pls.add({ label: `loft ${v}`, bindGroupLayouts: [layout0, layout1, ...third[v]] });
    pipelineLayout.set(v, id);
    return id;
  };

  // ---- the pose stages ---------------------------------------------------------------------------------------------
  const preludeChunk = chunk(PRELUDE_WGSL);
  const enableF16 = () => chunk("enable f16;\n");
  const replayed = (a.recurrences ?? []).flatMap((r) => (r.replay === undefined ? [] : [{ r, replay: r.replay }]));
  const poseLayout = a.stages.length > 0 || replayed.length > 0 ? pls.add({ label: "loft pose", bindGroupLayouts: [layout0, poseLayout1] }) : -1;
  const dispatch = (st: Artefact["stages"][number], what: string, alternative?: string): Dispatch => {
    const module = modules.add({ label: `${what} "${st.label}"`, chunks: [preludeChunk, chunk(st.wgsl)] });
    if (alternative !== undefined && alternative !== st.wgsl) mixedModules[module] = [enableF16(), preludeChunk, chunk(alternative)];
    const pipeline = computes.add({ label: st.label, layout: poseLayout, module, entryPoint: "pose_main" });
    return { label: st.label, pipeline, bindGroups: [group0, poseGroup1], workgroups: Math.ceil(st.invocations / POSE_WORKGROUP) };
  };
  const pose = a.stages.map((st, i) => dispatch(st, "pose stage", mixed?.stages[i]));
  // A checkpointed recurrence's replay: three more pose-stage modules, recorded by the loader before the stages.
  const replay = replayed.map(({ r, replay: p }): Replay => ({
    label: `replay ${r.name}`, slot: p.slot, rate: r.rate, start: r.start, steps: r.steps, every: r.kept.kind === "checkpoints" ? r.kept.every : 1,
    first: dispatch(p.first, "replay stage"), ping: dispatch(p.ping, "replay stage"), pong: dispatch(p.pong, "replay stage"),
  }));

  // ---- modules and render pipelines ---------------------------------------------------------------------------------
  // One module per (shader, way it is used). A shader nothing uses is not written: which host WGSL it expects depends on
  // its use (a post shader does not compile as a draw), so there is nothing sound to check it against.
  const sceneChunk = chunk(a.sceneWgsl);
  const moduleIds = new Map<string, Id>();
  let passChunk: Id[] | undefined;
  const moduleOf = (shader: string, v: Variant): Id => {
    const shared = sharing.moduleOf(shader);
    const key = `${shared.key}|${v}|${withPasses ? "passes" : ""}`;
    const known = moduleIds.get(key);
    if (known !== undefined) return known;
    const variant = VARIANT[v].wgsl === "" ? [] : [chunk(VARIANT[v].wgsl)];
    // The pass bindings chunk is the same for every module of one artefact (all passes, or none), so a shared group's
    // module needs it exactly as an unshared one does; it is part of the key so the key names every chunk that varies.
    passChunk ??= withPasses ? [chunk(passBindingsWgsl(passes))] : [];
    const prefix = [preludeChunk, ...passChunk, ...variant];
    const id = modules.add({ label: `shader "${shared.key}"${VARIANT[v].named}`, chunks: [...prefix, sceneChunk, chunk(shared.wgsl)] });
    const alternative = mixed?.shaders[shader];
    if (alternative !== undefined || mixed?.sceneWgsl !== undefined) {
      mixedModules[id] = [enableF16(), ...prefix, mixed?.sceneWgsl === undefined ? sceneChunk : chunk(mixed.sceneWgsl), chunk(alternative ?? shared.wgsl)];
    }
    moduleIds.set(key, id);
    return id;
  };
  // Equal descriptors are one pipeline.
  const pipelineIds = new Map<string, Id>();
  // `k`: the draw's index, for its shared constants (sharing.drawOf); `format`: the colour target (a pass's own).
  const drawPipeline = (d: Draw, k: number, format: GPUTextureFormat = COLOR_FORMAT): Id => {
    const v = drawVariant(d);
    const module = moduleOf(d.shader, v);
    const shaderKey = sharing.moduleOf(d.shader).key;
    const { constants: c } = sharing.drawOf(k);
    const blend = BLEND[d.blend];
    const desc: RenderPipelineDesc = {
      label: `${d.shader} (${d.label})`,
      layout: layoutOf(v),
      vertex: { module, entryPoint: "vs", constants: { ...c } },
      fragment: { module, entryPoint: "fs", constants: { ...c }, targets: [blend ? { format, blend } : { format }] },
      primitive: { topology: "triangle-list", frontFace: "ccw", cullMode: d.cull },
      depthStencil: { format: DEPTH_FORMAT, depthWriteEnabled: d.depthWrite, depthCompare: "less" },
      multisample: { count: SAMPLES },
    };
    const constants = Object.keys(c).sort().map((n) => `${n}=${c[n]}`).join(",");
    const key = `${shaderKey}|${v}|${d.cull}|${d.blend}|${d.depthWrite}|${constants}${format === COLOR_FORMAT ? "" : `|${format}`}`;
    const known = pipelineIds.get(key);
    if (known !== undefined) return known;
    const id = renders.add(desc);
    pipelineIds.set(key, id);
    return id;
  };
  /** A draw's pipeline for a depth pass: no colour target, single-sampled, its depth drawn the way `depth` says. */
  // The draw's shared module and constants, as drawPipeline: its vs reads the draw's row from the instance index, so
  // every command drawn with it carries the draw's instanceBase (`plannedOf`).
  const depthPipeline = (d: Draw, k: number, depth: DrawDepth): Id => {
    const module = moduleOf(d.shader, "draw");
    const shaderKey = sharing.moduleOf(d.shader).key;
    const { constants: c } = sharing.drawOf(k);
    const constants = Object.keys(c).sort().map((n) => `${n}=${c[n]}`).join(",");
    const fragment = ((): RenderPipelineDesc["fragment"] => {
      switch (depth.kind) {
        case "vertex": return undefined;
        case "entry": return { module, entryPoint: depth.entry, constants: { ...c }, targets: [] };
      }
    })();
    const key = `depth|${shaderKey}|${d.cull}|${depth.kind === "entry" ? depth.entry : "vertex"}|${constants}`;
    const known = pipelineIds.get(key);
    if (known !== undefined) return known;
    const id = renders.add({
      label: `${d.shader} (${d.label}, depth)`,
      layout: layoutOf("draw"),
      vertex: { module, entryPoint: "vs", constants: { ...c } },
      ...(fragment === undefined ? {} : { fragment }),
      primitive: { topology: "triangle-list", frontFace: "ccw", cullMode: d.cull },
      depthStencil: { format: DEPTH_FORMAT, depthWriteEnabled: true, depthCompare: "less" },
      multisample: { count: 1 },
    });
    pipelineIds.set(key, id);
    return id;
  };
  /**
   * Draw `draw` with `pipeline`. Every planned command is made here, so none misses its instanceBase: a shared shader
   * reads its row from the instance index, and a command without the base would read another draw's row.
   */
  // Written only when not 0: a file that shares nothing keeps its bytes and its version (formatVersion).
  const plannedOf = (pipeline: Id, draw: number): DrawCommand => {
    const { instanceBase } = sharing.drawOf(draw);
    return instanceBase === 0 ? { kind: "planned", pipeline, draw } : { kind: "planned", pipeline, draw, instanceBase };
  };
  const planned = (pass: Draw["pass"]): DrawCommand[] =>
    a.draws.flatMap((d, draw) => (d.pass === pass ? [plannedOf(drawPipeline(d, draw), draw)] : []));
  const opaque = planned("opaque"), transparent = planned("transparent");
  const clearValue = [a.clear[0], a.clear[1], a.clear[2], 1] as const;

  // ---- the passes ---------------------------------------------------------------------------------------------------
  const RA = "RENDER_ATTACHMENT", TB = "TEXTURE_BINDING";
  const fullScreen = (label: string, layout: Id, module: Id, vs: string, format: GPUTextureFormat, fs = "fs"): Id => renders.add({
    label, layout,
    vertex: { module, entryPoint: vs, constants: {} },
    fragment: { module, entryPoint: fs, constants: {}, targets: [{ format }] },
    primitive: { topology: "triangle-list", frontFace: "ccw", cullMode: "none" },
    multisample: { count: 1 },
  });
  // The depth resolve's and the clearance map's shared objects, made once, by the first frame structure that needs
  // them (the view's, in the same order as before passes existed, so a scene without passes keeps its ids).
  let resolveShared: { readonly layout: Id; readonly pipeline: Id } | undefined;
  const depthResolve = () => resolveShared ??= (() => {
    const layout = bgls.add({
      label: "loft depth resolve",
      entries: [{ binding: 0, visibility: ["FRAGMENT"], texture: { sampleType: "depth", viewDimension: "2d", multisampled: true } }],
    });
    const module = modules.add({ label: "loft depth resolve", chunks: [chunk(DEPTH_RESOLVE_WGSL)] });
    return { layout, pipeline: fullScreen("loft depth resolve", pls.add({ label: "loft depth resolve", bindGroupLayouts: [layout] }), module, "vs", SCENE_DEPTH_FORMAT) };
  })();
  let clearanceShared: { readonly input: Id; readonly nearest: Id; readonly steps: readonly { readonly step: string; readonly pipeline: Id }[] } | undefined;
  const clearance = () => clearanceShared ??= (() => {
    // The clearance map: three passes, each reading the previous one's texture (CLEARANCE_WGSL).
    const input = bgls.add({
      label: "loft clearance input",
      entries: [{ binding: 0, visibility: DRAWN, texture: { sampleType: "unfilterable-float", viewDimension: "2d", multisampled: false } }, { binding: 1, visibility: DRAWN, sampler: { type: "non-filtering" } }],
    });
    const nearest = samplers.add({ label: "loft clearance", magFilter: "nearest", minFilter: "nearest", addressModeU: "clamp-to-edge", addressModeV: "clamp-to-edge" });
    const layout = pls.add({ label: "loft clearance", bindGroupLayouts: [input] });
    const module = modules.add({ label: "loft clearance", chunks: [chunk(CLEARANCE_WGSL)] });
    // One full-screen triangle per step (cover_vs, CLEARANCE_VERTICES); each fragment past the step's grid discards.
    const steps = (["quads", "rows", "clear"] as const).map((step) => ({ step, pipeline: fullScreen(`loft clearance ${step}`, layout, module, "cover_vs", "r8unorm", `${step}_fs`) }));
    return { input, nearest, steps };
  })();

  // Passes' own objects, made before the view's so the view's group 3 can name every pass's picture.
  const offscreen = withPasses ? lowerPassObjects() : undefined;
  /** Group 3 a view pass adds in a scene with passes; nothing in one without. */
  const g3 = offscreen === undefined ? [] : [offscreen.viewGroup];
  /** Groups 2 and 3 of a pass whose draws read no scene depth: `filler` is a group of `plainGroup2`'s layout. */
  const g23 = (filler: Id): Id[] => (offscreen === undefined ? [] : [filler, offscreen.viewGroup]);
  let filler = -1;

  const colour = textures.add({ label: "loft colour x4", format: COLOR_FORMAT, sampleCount: SAMPLES, usage: [RA] });
  const viewPasses: RenderPassDesc[] = [];
  if (!split) {
    const depth = textures.add({ label: "loft depth x4", format: DEPTH_FORMAT, sampleCount: SAMPLES, usage: [RA] });
    if (withPasses) filler = groups.add({ label: "loft none", layout: emptyLayout, entries: [] });
    viewPasses.push({
      label: "loft scene",
      color: { view: { kind: "texture", texture: colour }, resolveTarget: { kind: "output" }, loadOp: "clear", storeOp: "discard", clearValue },
      depth: { view: depth, depthClearValue: 1, depthLoadOp: "clear", depthStoreOp: "discard" },
      bindGroups: [group0, group1, ...g23(filler)],
      commands: [...opaque, ...transparent],
    });
  } else {
    const depth = textures.add({ label: "loft depth x4", format: DEPTH_FORMAT, sampleCount: SAMPLES, usage: [RA, TB] });
    const sceneDepth = textures.add({ label: "loft scene depth", format: SCENE_DEPTH_FORMAT, sampleCount: 1, usage: [RA, TB] });
    const resolve = depthResolve();
    const postPipelines = post.map((p) => fullScreen(`${p.shader} (${p.label})`, layoutOf("post"), moduleOf(p.shader, "post"), "post_vs", COLOR_FORMAT));
    const sampler = samplers.add({ label: "loft post", magFilter: "linear", minFilter: "linear", addressModeU: "clamp-to-edge", addressModeV: "clamp-to-edge" });
    const resolveGroup = groups.add({ label: "loft depth resolve", layout: resolve.layout, entries: [{ binding: 0, texture: depth }] });
    const cl = clearance();
    const clearanceSteps = cl.steps.map(({ step, pipeline }) => ({
      step,
      texture: textures.add({ label: `loft clearance ${step}`, format: "r8unorm", sampleCount: 1, usage: [RA, TB] }),
      pipeline,
    }));
    const clearanceMap = clearanceSteps[2]!.texture;
    const depthGroup = groups.add({ label: "loft scene depth", layout: depthLayout, entries: [{ binding: 0, texture: sceneDepth }, { binding: 1, texture: clearanceMap }] });
    filler = depthGroup;
    // Ping-pong: pass k reads intermediate (k - 1) % 2 and writes k % 2; one post pass needs one intermediate.
    const n = post.length;
    const intermediates = Array.from({ length: Math.min(n, 2) }, (_, i) => {
      const texture = textures.add({ label: `loft post ${i}`, format: COLOR_FORMAT, sampleCount: 1, usage: [RA, TB] });
      const group = groups.add({ label: `loft post input ${i}`, layout: postLayout, entries: [{ binding: 0, texture: sceneDepth }, { binding: 1, texture }, { binding: 2, sampler }] });
      return { texture, group };
    });
    /** Stage 0 is the scene; stage k >= 1 is post pass k - 1. The last stage writes the output. */
    const outputOf = (k: number) => (k === n ? { kind: "output" as const } : { kind: "texture" as const, texture: intermediates[k % 2]!.texture });
    viewPasses.push({
      label: "loft scene opaque",
      color: { view: { kind: "texture", texture: colour }, loadOp: "clear", storeOp: "store", clearValue },
      depth: { view: depth, depthClearValue: 1, depthLoadOp: "clear", depthStoreOp: "store" },
      bindGroups: [group0, group1, ...g23(filler)],
      commands: opaque,
    }, {
      label: "loft depth resolve",
      color: { view: { kind: "texture", texture: sceneDepth }, loadOp: "clear", storeOp: "store", clearValue: [1, 0, 0, 1] },
      bindGroups: [resolveGroup],
      commands: [{ kind: "fixed", pipeline: resolve.pipeline, vertices: 3, instances: 1 }],
    }, ...clearanceSteps.map((s, i): RenderPassDesc => ({
      label: `loft clearance ${s.step}`,
      color: { view: { kind: "texture", texture: s.texture }, loadOp: "clear", storeOp: "store", clearValue: [0, 0, 0, 0] },
      bindGroups: [groups.add({ label: `loft clearance ${s.step} input`, layout: cl.input, entries: [{ binding: 0, texture: i === 0 ? sceneDepth : clearanceSteps[i - 1]!.texture }, { binding: 1, sampler: cl.nearest }] })],
      commands: [{ kind: "fixed", pipeline: s.pipeline, vertices: CLEARANCE_VERTICES, instances: 1 }],
    })), {
      label: "loft scene transparent",
      color: { view: { kind: "texture", texture: colour }, resolveTarget: outputOf(0), loadOp: "load", storeOp: "discard" },
      depth: { view: depth, depthLoadOp: "load", depthStoreOp: "discard" },
      // Group 2 is ignored by pipelines whose layout stops at group 1.
      bindGroups: [group0, group1, depthGroup, ...g3],
      commands: transparent,
    }, ...postPipelines.map((pipeline, i): RenderPassDesc => {
      const k = i + 1;
      return {
        label: `loft post ${i}`,
        color: { view: outputOf(k), loadOp: "clear", storeOp: "store", clearValue: [0, 0, 0, 1] },
        bindGroups: [group0, group1, intermediates[(k - 1) % 2]!.group, ...g3],
        commands: [{ kind: "fixed", pipeline, vertices: 3, instances: 1 }],
      };
    }));
  }
  // Every pass is drawn before the view, in the artefact's order (each after the passes it reads).
  const renderPasses: RenderPassDesc[] = [...(offscreen?.record(filler) ?? []), ...viewPasses];

  /**
   * The passes' textures, stand-ins, camera buffers and bind groups, and a function that records their render passes
   * (called after the view's pipelines exist, which a colour pass reuses).
   */
  function lowerPassObjects() {
    const sampler = samplers.add({ label: "loft pass", magFilter: "linear", minFilter: "linear", addressModeU: "clamp-to-edge", addressModeV: "clamp-to-edge" });
    const pictures = passes.map((p) => textures.add({ label: `loft pass ${p.name}`, size: p.size, format: PASS_TEXTURE[keepOf(p)].format, sampleCount: 1, usage: [RA, TB] }));
    const standIn = {
      float: textures.add({ label: "loft pass stand-in", size: ONE_PIXEL, format: COLOR_FORMAT, sampleCount: 1, usage: [TB] }),
      "unfilterable-float": textures.add({ label: "loft pass stand-in (depth)", size: ONE_PIXEL, format: SCENE_DEPTH_FORMAT, sampleCount: 1, usage: [TB] }),
    };
    /** Group 3 inside pass `upTo` (its index), or the view's (every picture): a stand-in for the pass itself and those after it. */
    const group3 = (label: string, upTo: number): Id => groups.add({
      label, layout: passLayout,
      entries: [{ binding: 0, sampler }, ...passes.map((p, i) => ({ binding: 1 + i, texture: i < upTo ? pictures[i]! : standIn[PASS_TEXTURE[keepOf(p)].sampleType] }))],
    });
    const viewGroup = group3("loft passes (view)", passes.length);
    const copies: BufferCopy[] = [];
    const lowered = passes.map((p, i) => {
      const own = group3(`loft passes (in ${p.name})`, i);
      const w = p.what;
      switch (w.kind) {
        case "screen": return { p, i, own, frame: group0 };
        case "scene": {
          // The pass's own `frame`: its camera table, copied after the pose stages (Manifest.frame.copies).
          const buf = buffer(`loft pass ${p.name} frame`, FRAME_FLOATS * 4, ["UNIFORM", "COPY_DST"]);
          copies.push({ from: bufferOf.get(w.camera.buffer)!, fromOffset: w.camera.offset * 16, to: buf, bytes: FRAME_FLOATS * 4 });
          // Group 0's layout holds the shared rows at ROWS_BINDING when any shader is shared: the pass's group binds them too.
          const frame = groups.add({ label: `loft pass ${p.name} frame`, layout: layout0, entries: [buf, inputBuf, paramBuf, curveBuf, detailBuf, ...rowsBuf].map((b, binding) => ({ binding, buffer: b })) });
          return { p, i, own, frame };
        }
      }
    });
    /** `empty`: a group of the plain draws' group 2 layout (the view's scene depth in a split frame, else empty). */
    const record = (empty: Id): RenderPassDesc[] => lowered.flatMap(({ p, i, own, frame }): RenderPassDesc[] => {
      const picture = pictures[i]!;
      const w = p.what;
      switch (w.kind) {
        case "screen": {
          const pipeline = fullScreen(`${w.shader} (pass ${p.name})`, layoutOf("draw"), moduleOf(w.shader, "draw"), "vs", LIGHT_FORMAT);
          return [{
            label: `loft pass ${p.name}`,
            color: { view: { kind: "texture", texture: picture }, loadOp: "clear", storeOp: "store", clearValue: [0, 0, 0, 0] },
            bindGroups: [frame, group1, empty, own],
            commands: [{ kind: "fixed", pipeline, vertices: 3, instances: 1 }],
          }];
        }
        case "scene": {
          const drawn = w.draws.map((k) => ({ d: a.draws[k]!, k }));
          switch (w.keep) {
            case "depth": return [{
              label: `loft pass ${p.name}`,
              depth: { view: picture, depthClearValue: 1, depthLoadOp: "clear", depthStoreOp: "store" },
              bindGroups: [frame, group1, empty, own],
              commands: drawn.map(({ d, k }) => plannedOf(depthPipeline(d, k, d.depth!), k)),
            }];
            case "colour": case "light": {
              const format = PASS_TEXTURE[w.keep].format;
              const cmds = (pass: Draw["pass"]): DrawCommand[] => drawn.filter(({ d }) => d.pass === pass).map(({ d, k }) => plannedOf(drawPipeline(d, k, format), k));
              return colourPass(p, picture, format, frame, own, empty, cmds("opaque"), cmds("transparent"), drawn.some(({ d }) => d.sceneDepth === true));
            }
          }
        }
      }
    });
    return { viewGroup, copies, record };
  }

  /**
   * A colour pass: the view's frame structure at the pass's size, resolved into its picture. One multisampled pass;
   * or, when a transparent draw it holds reads scene depth, opaque, the depth resolve and the clearance map, then
   * transparent, as the view's split frame (with no post passes).
   */
  function colourPass(p: ArtefactPass, picture: Id, format: GPUTextureFormat, frame: Id, own: Id, empty: Id, opaqueCmds: DrawCommand[], transparentCmds: DrawCommand[], split: boolean): RenderPassDesc[] {
    const label = `loft pass ${p.name}`;
    const x4 = textures.add({ label: `${label} colour x4`, size: p.size, format, sampleCount: SAMPLES, usage: [RA] });
    const depth = textures.add({ label: `${label} depth x4`, size: p.size, format: DEPTH_FORMAT, sampleCount: SAMPLES, usage: split ? [RA, TB] : [RA] });
    if (!split) return [{
      label,
      color: { view: { kind: "texture", texture: x4 }, resolveTarget: { kind: "texture", texture: picture }, loadOp: "clear", storeOp: "discard", clearValue },
      depth: { view: depth, depthClearValue: 1, depthLoadOp: "clear", depthStoreOp: "discard" },
      bindGroups: [frame, group1, empty, own],
      commands: [...opaqueCmds, ...transparentCmds],
    }];
    const sceneDepth = textures.add({ label: `${label} scene depth`, size: p.size, format: SCENE_DEPTH_FORMAT, sampleCount: 1, usage: [RA, TB] });
    const resolve = depthResolve(), cl = clearance();
    const steps = cl.steps.map(({ step, pipeline }) => ({ step, pipeline, texture: textures.add({ label: `${label} clearance ${step}`, size: p.size, format: "r8unorm", sampleCount: 1, usage: [RA, TB] }) }));
    const depthGroup = groups.add({ label: `${label} scene depth`, layout: depthLayout, entries: [{ binding: 0, texture: sceneDepth }, { binding: 1, texture: steps[2]!.texture }] });
    return [{
      label: `${label} opaque`,
      color: { view: { kind: "texture", texture: x4 }, loadOp: "clear", storeOp: "store", clearValue },
      depth: { view: depth, depthClearValue: 1, depthLoadOp: "clear", depthStoreOp: "store" },
      bindGroups: [frame, group1, empty, own],
      commands: opaqueCmds,
    }, {
      label: `${label} depth resolve`,
      color: { view: { kind: "texture", texture: sceneDepth }, loadOp: "clear", storeOp: "store", clearValue: [1, 0, 0, 1] },
      bindGroups: [groups.add({ label: `${label} depth resolve`, layout: resolve.layout, entries: [{ binding: 0, texture: depth }] })],
      commands: [{ kind: "fixed", pipeline: resolve.pipeline, vertices: 3, instances: 1 }],
    }, ...steps.map((s, i): RenderPassDesc => ({
      label: `${label} clearance ${s.step}`,
      color: { view: { kind: "texture", texture: s.texture }, loadOp: "clear", storeOp: "store", clearValue: [0, 0, 0, 0] },
      bindGroups: [groups.add({ label: `${label} clearance ${s.step} input`, layout: cl.input, entries: [{ binding: 0, texture: i === 0 ? sceneDepth : steps[i - 1]!.texture }, { binding: 1, sampler: cl.nearest }] })],
      commands: [{ kind: "fixed", pipeline: s.pipeline, vertices: CLEARANCE_VERTICES, instances: 1 }],
    })), {
      label: `${label} transparent`,
      color: { view: { kind: "texture", texture: x4 }, resolveTarget: { kind: "texture", texture: picture }, loadOp: "load", storeOp: "discard" },
      depth: { view: depth, depthLoadOp: "load", depthStoreOp: "discard" },
      bindGroups: [frame, group1, depthGroup, own],
      commands: transparentCmds,
    }];
  }

  // ---- level-of-detail bounds ---------------------------------------------------------------------------------------
  const grids = gridBounds(a.draws);
  let at = 0;
  const copies = grids.map((g) => {
    const c = { slot: g.slot, from: bufferOf.get(g.buffer)!, fromOffset: g.offset * 16, at, instances: g.instances };
    at += g.instances;
    return c;
  });
  const bounds: Manifest["frame"]["bounds"] = grids.length === 0
    ? { kind: "none" }
    : { kind: "read", buffer: buffer("loft bounds", at * 16, ["MAP_READ", "COPY_DST"]), grids: copies };

  const manifest: Manifest = {
    format: FORMAT,
    version: precision === "mixed" ? 4 : formatVersion({ replay: replay.length > 0, passes: withPasses, shared: sharing.rows.length > 0 }),
    ...(precision === "mixed" ? { precision: { kind: "mixed", fallback: "f32-fallback", modules: mixedModules, buffers: mixedBuffers } as const } : {}),
    chunks: chunkNames,
    blobs: [...blobs].map(([name, b]) => ({ name, bytes: b.byteLength })),
    inputs: a.inputs,
    inputFloats: a.inputFloats,
    params: a.params,
    paramFloats,
    cameras: a.cameras,
    curves: { buffer: curveBuf, floats: a.curves.length },
    buffers: buffers.items,
    textures: textures.items,
    samplers: samplers.items,
    bindGroupLayouts: bgls.items,
    pipelineLayouts: pls.items,
    modules: modules.items,
    computePipelines: computes.items,
    renderPipelines: renders.items,
    bindGroups: groups.items,
    frame: {
      writes: [{ kind: "frame", buffer: frameBuf }, { kind: "inputs", buffer: inputBuf }, { kind: "params", buffer: paramBuf }],
      pose,
      bounds,
      // Only the slots the artefact uses are written each frame (the rest of the 64 KiB array is never read).
      detail: { buffer: detailBuf, floats: Math.max(4, slotsUsed(a.draws) * 4) },
      passes: renderPasses,
      ...(replay.length > 0 ? { replay } : {}),
      ...(offscreen === undefined ? {} : { copies: offscreen.copies }),
    },
    dynamic: dynamic.map((d) => d.id),
    plan: a.draws.map((d) => ({ label: d.label, layer: d.layer, vertices: d.vertices, instances: d.instances, detail: d.detail })),
    tables: a.tables.map((t) => ({ name: t.name, buffer: bufferOf.get(t.buffer)!, offset: t.offset, vec4s: t.vec4s, lanes: t.lanes, fill: t.fill })),
    stats: a.stats,
  };
  return { manifest, chunks: new Map(chunkNames.map((name) => [name, chunkText.get(name)!])), blobs };
}
