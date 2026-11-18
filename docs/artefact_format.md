# The Loft artefact as files (format versions 1 through 4)

A built Loft scene is a directory that a client draws with no JavaScript from the compiler and no scene module. It is
the host, lowered: every WebGPU object a loader creates is an entry in a list of `manifest.json`, described with
WebGPU's own field names, and the frame is a recipe over those objects. A loader executes it; it decides nothing about
the scene. Why it is shaped this way, and what it replaced, is `docs/gpu_native.md` (phase 2).

The normative types are `src/host/manifest.ts`. The reference loader is `src/host/host.ts` (`loadBundle`), with
`src/host/files.ts` (reading), `src/host/camera.ts` (camera matrices) and `src/host/detail.ts` (the level-of-detail
plan). `loft build <scene> --out <dir>` writes one; `loft render --artefact <dir>` draws one.

## Directory

```
manifest.json         the lowered host, JSON (UTF-8)
wgsl/<chunk>.wgsl     one file per WGSL chunk, UTF-8, named by `manifest.chunks[i]`
bin/<blob>.bin        one file per blob, little-endian data, named by `manifest.blobs[i].name`
```

Nothing else. A chunk's name is a 64-bit hash of its text in hex; a loader never computes it, it only opens the file.

## Conventions

- **Ids.** Every reference is an index into the list of its kind: `buffers`, `textures`, `samplers`,
  `bindGroupLayouts`, `pipelineLayouts`, `modules`, `computePipelines`, `renderPipelines`, `bindGroups`, `chunks`.
  Create every list in order and keep the objects in arrays.
- **Names are WebGPU's.** Every string field (formats, load and store ops, blend factors, compare functions, cull
  modes, filter and address modes, binding types, sample types) is the WebGPU IDL enum value. wgpu's enums are the
  same values in Rust casing (`"one-minus-src-alpha"` is `BlendFactor::OneMinusSrcAlpha`).
- **Flags** are lists of names: buffer `usage` from `MAP_READ COPY_SRC COPY_DST UNIFORM STORAGE`, texture `usage`
  from `COPY_SRC TEXTURE_BINDING RENDER_ATTACHMENT`, layout `visibility` from `VERTEX FRAGMENT COMPUTE`. Or them.
- **Limits and features.** WebGPU's default limits suffice (the compiler packs tables into at most 5 storage buffers
  per stage besides the prelude's 3, and `loft_detail` is a 64 KiB uniform). Version 4 may use
  [`shader-f16`](https://gpuweb.github.io/gpuweb/#shader-f16). A loader that profiles may ask for `timestamp-query`;
  ordinary drawing never uses timestamps.
- **Output.** A frame ends in a texture the caller owns: `rgba8unorm`, the view's size, usage `RENDER_ATTACHMENT` (and
  whatever the caller needs, such as `COPY_SRC` for a readback). Shaders write display-referred sRGB values; there is
  no conversion anywhere. A canvas must be configured `rgba8unorm`.
- **Coordinates.** World y is up. Clip space is WebGPU's (depth 0 near, 1 far); front faces wind counter-clockwise.

## Pictures drawn off screen (version 2)

A scene with a pass (src/frame/pass.ts) adds four things a loader must act on, all optional fields (the version
rule is under "Versions" below):

- `textures[i].size`: absent is the view's size (version 1's only size); `{ "kind": "view", "scale": s }` is
  `max(1, round(width s))` by `max(1, round(height s))`; `{ "kind": "fixed", "width": w, "height": h }` is that.
- `frame.passes[i].color` may be absent: a pass with only a depth attachment (a depth pass).
- `renderPipelines[i].fragment` may be absent: a pipeline with no fragment stage (depth from the rasterised triangle).
- `frame.copies`: buffer copies run after the pose pass (below), each pass's camera into its own frame uniform.

Everything else a pass needs is objects version 1 already has: more textures, bind groups, pipelines and render
passes, listed in drawing order (each pass before the view, after the passes it reads). The group 3 binding layout
of every draw and post module in such a file is `passBindingsWgsl` in `src/artefact.ts`, a chunk the file carries.

## Mixed precision (version 4)

`precision` is `{"kind":"mixed","fallback":"f32-fallback","modules":{...},"buffers":{...}}`. Main descriptors hold
the f32 path. `modules` maps a module ID to a replacement list of chunk IDs. `buffers` maps a buffer ID to a
replacement `{size, init}` pair. Its blob may include binary16 and binary32 fields at their WGSL storage offsets.
Ids, bindings, pass recipes and frame writes stay the same. Both variants refer to the shared chunk and blob lists.

Before creating any objects, check whether the device grants `shader-f16`. If granted, apply both maps and report
`mixed`. Otherwise use the main descriptors and report `f32-fallback`. The TypeScript renderer also accepts an explicit f32 override, which uses the main descriptors and reports `f32`. Versions 1–3 have only the f32 path and no `precision` field.

## Load, call by call

1. **Read.** Parse `manifest.json`. Refuse unless `format == "loft-artefact"` and `version` is 1, 2, 3 or 4. Read each
   `wgsl/<chunks[i]>.wgsl` and each `bin/<blobs[i].name>.bin` (check its length against `blobs[i].bytes`).
2. **Refuse JavaScript.** If any `tables[i].fill.kind == "cpu"`, refuse to load and name the tables (`loft build`
   never writes such a file; this guards hand-made ones). (`fill.kind` is `map`, `scan`, `cpu` or, in version 2,
   `replay`: a table the frame's replays write.) Select precision before step 3.
3. **Buffers.** For each `buffers[i]`: `createBuffer({ label, size, usage })`. If it has `init`, and that blob is not
   empty: `queue.writeBuffer(buffer, 0, blob bytes)`.
4. **Samplers.** For each `samplers[i]`: `createSampler(descriptor)` (no mipmaps; `mipmapFilter` default).
5. **Bind-group layouts.** For each: `createBindGroupLayout({ label, entries })`, each entry as written with its
   `visibility` or-ed. A texture entry carries `sampleType`, `viewDimension` and `multisampled`.
6. **Pipeline layouts.** For each: `createPipelineLayout({ label, bindGroupLayouts: ids -> layouts })`.
7. **Shader modules.** For each `modules[i]`: its code is the texts of `chunks` concatenated in order with nothing
   between them; `createShaderModule({ label, code })`. (A module's first chunk is normally the prelude: the format
   carries it, a loader has no copy of its own.) Report a compile error with the module's label.
8. **Compute pipelines.** For each: `createComputePipeline({ label, layout, compute: { module, entryPoint } })`.
9. **Render pipelines.** For each: `createRenderPipeline` with `layout`, `vertex: { module, entryPoint, constants }`,
   `fragment: { module, entryPoint, constants, targets }`, `primitive`, `depthStencil` when present, `multisample`.
   No vertex buffers: vertex shaders derive everything from the vertex and instance index and the bound tables.
   `constants` are WGSL `override` values (wgpu: `PipelineCompilationOptions::constants`).
10. **Fixed bind groups.** For each `bindGroups[i]` whose entries name no texture: `createBindGroup({ layout, entries })`
    with `{ binding, buffer }` as the whole buffer and `{ binding, sampler }` as the sampler.
11. **Level-of-detail memory.** An empty map from grid slot to the level each instance drew last frame.

Pipelines are created at load, eagerly, so no frame stalls on compilation.

**Per view size** (at the first frame and whenever the width or height changes): destroy the previous set, then for
each `textures[i]`: `createTexture({ label, size, format, sampleCount, usage })` with `size` from its `size` field
(absent: the view's; version 2 above) and its default view; then for each bind group that names a texture,
`createBindGroup` with `{ binding, texture }` as that texture's default view.

A render pipeline with no `fragment` is created with none (it draws depth only).

## A frame, call by call

Given a view: camera, width, height, time, input and param values, `construction` (show construction lines),
`cellPixels` (default 7; 0 draws every grid at its finest).

1. **Frame writes**, for each `frame.writes[i]`, `queue.writeBuffer(buffer, 0, data)` (skip an empty `data`):
   - `frame`: 48 f32 (below).
   - `inputs`: `inputFloats` f32. Each input's `def` at its `slot`; the view's value over it (a scalar, or as many
     floats as the input's type: f32 1, vec2 2, vec3 3, vec4 4); then, if an input is named `time`, the view's time
     at its slot; then, for each `frame.replay[i]` (version 2), its step n at its `slot` (Replays, below). A view
     naming an unknown input or param is an error, not ignored, and so is a view setting a replay's slot.
   - `params`: `paramFloats` f32. Each param's `def` at its `slot`, the view's value over it. `min` and `max` are
     for editors; a loader does not clamp.
2. **Pose.** One command encoder. If `frame.pose` or `frame.replay` is not empty: one compute pass; first each
   replay's dispatches (Replays, below), then for each dispatch of `frame.pose`: `setPipeline`,
   `setBindGroup(k, bindGroups[k])` for each listed group, `dispatchWorkgroups(workgroups)`. Dispatches run in order
   and each sees what the earlier ones wrote.
3. **Copies** (version 2). For each `frame.copies[i]`: `copyBufferToBuffer(buffers[from], fromOffset, buffers[to], 0,
   bytes)` in the same encoder, after the pass, in order.
   **Bounds.** If `frame.bounds.kind == "read"`: for each grid, `copyBufferToBuffer(buffers[from], fromOffset,
   buffers[bounds.buffer], at * 16, instances * 16)` in the same encoder, after the pass. Submit the encoder.
4. **Read the bounds** (when "read"): map `bounds.buffer` for reading. A headless frame waits for this frame's map. An
   interactive frame does not wait: it plans with the last bounds it read (one or two frames old), or with none
   before the first read completes, and starts a new read only when none is in flight.
5. **Plan** (below): for each `plan[k]`, a list of runs `(vertices, instances, first)`, and the `loft_detail` slots.
   `queue.writeBuffer(buffers[frame.detail.buffer], 0, first frame.detail.floats floats of the slots)`.
6. **Passes.** One command encoder. For each `frame.passes[i]`: `beginRenderPass` with one colour attachment when
   `color` is present (its `view` and `resolveTarget`, each a texture's default view or the output; `loadOp`, `storeOp`,
   `clearValue` as r, g, b, a), none when it is absent (version 2), and, when `depth` is present, a depth attachment (`view` is a texture id;
   `depthLoadOp`, `depthStoreOp`, `depthClearValue`; no stencil). `setBindGroup(k, bindGroups[k])` for each listed
   group, in order. Then each command:
   - `fixed`: `setPipeline`, `draw(vertices, instances)`.
   - `planned`: if the plan for `plan[draw]` has no run, nothing (not even `setPipeline`); else `setPipeline`, then
     for each run `draw(run.vertices, run.instances, 0, instanceBase + run.first)`, `instanceBase` 0 when absent
     (version 3: a shared draw shader reads its draw's numbers from the row the high bits of the instance pick).
   End the pass. Submit.

A pass that draws a picture off screen (version 2) is executed exactly like the others: its textures, its bind
groups (its own group 0, whose `frame` uniform the copies filled; group 3, the pictures drawn before it) and its draws
(planned draws use the view's plan: the same runs and level-of-detail slots) are all in the lists.

A pass may draw into only part of its view-sized target (the clearance passes after the depth resolve draw one
fragment per 4 x 4 or 8 x 8 pixels into the top left: a full-screen triangle whose fragments past the grid discard);
the loader does nothing different for it.

All writes of a frame are issued before the submit that reads them. (A `writeBuffer` is ordered with submits, not with
passes inside one encoder: a write between two passes of one encoder lands before both.)

### Replays (version 2)

A scene may hold state stepped by its own function (`recur`), baked on a GPU by `loft build`. Its baked states are
ordinary static blobs (a buffer's `init`): a loader never bakes. When every state is kept that is all there is, and
the file is version 1. When the history is too large, every `every`-th state is kept (a checkpoint), and the frame
rebuilds the two states it reads from the nearest checkpoint: that is `frame.replay`, and the file is version 2.

Each `frame.replay[i]` has `slot`, `rate`, `start`, `steps`, `every` and three dispatches, `first`, `ping`, `pong`
(each like a pose dispatch: `pipeline`, `bindGroups`, `workgroups`). For a frame at time t (the view's time, the one
written into `time`):

- `n = min(max(floor((t - start) * rate), 0), max(steps - 1, 0))`, in f64, and `k = n mod every`.
- Write n as an f32 at float `slot` of the inputs (step 1 above). The shaders read it there, so every host replays to
  exactly the n the CPU computed.
- In the pose pass, before `frame.pose`: dispatch `first`, then for j = 1 .. k, `ping` when j is odd and `pong` when j
  is even: k + 1 dispatches.

A loader may skip dispatches when it has kept the step n_last its tables hold from its last frame (an optimisation, not
a format change: the pixels are the same bytes): same n, none; 0 < n - n_last <= k and `every` even, then n - n_last
steps, step j reading state n_last + j (`ping` when that index is odd, `pong` when even; state j lives in table j mod 2
when `every` is even). Anything else replays as above.

(What they do, for a reader: `first` reads checkpoint floor(n / every) from the static buffer, copies it into one
per-frame table and writes the next state into another; `ping` and `pong` each step one table into the other. The
shaders read states n and n + 1 from the two tables by the parity of k. On the GPU that baked, the replay repeats the
bake's own arithmetic, so it is bitwise the baked states; on another, within k steps of rounding.)

### The frame uniform (`frame` write)

48 f32, as WGSL `struct Frame` in the prelude chunk: `view_proj` (16, column-major), `inv_view_proj` (16), `eye`
(xyz, w = 1 perspective or 0 orthographic), `forward` (xyz unit, w = 0), `viewport` (width, height, 1/width,
1/height), `time` (x = seconds, rest 0). Computed in f64 and rounded once to f32. From `camera` (`cameras[name]`, or the
client's own) and aspect = width / height:

- `distance = |target - eye|` (must be positive); `near = distance / 100`, `far = distance * 100`.
- `forward = normalize(target - eye)`; `ref = (0, 0, -sign(forward.y))` if `|forward.y| > 0.999999`, else `(0, 1, 0)`;
  `right = normalize(cross(forward, ref))`; `up = cross(right, forward)`. The view matrix's rows are `right`, `up`,
  `-forward`, each with translation `-dot(row, eye)`; last row (0, 0, 0, 1).
- perspective (`fovY`): `f = 1 / tan(fovY / 2)`; `m[0] = f / aspect`, `m[5] = f`, `m[10] = far / (near - far)`,
  `m[11] = -1`, `m[14] = near far / (near - far)`, others 0.
- orthographic (`height`): `m[0] = 2 / (height aspect)`, `m[5] = 2 / height`, `m[10] = 1 / (near - far)`,
  `m[14] = near / (near - far)`, `m[15] = 1`, others 0.
- `view_proj = projection * view`; `inv_view_proj` its general inverse.

(Indices are column-major: element (row r, column c) at `c * 4 + r`.)

### The level-of-detail plan

The plan is the one per-frame procedure a loader runs on the CPU; phase 4 moves it into a compute stage. Per
`plan[k]` (draws with `layer == "construction"` get no runs unless the view shows construction):

- `fixed`: one run `(vertices, instances, 0)`.
- `grid` (`slot`, `radius`, `levels[]` each `cells: [a, b]`, `vertices`, `cell`): draws sharing a slot share one
  choice (the first to reach the slot chooses). With no bounds read yet, every instance takes level 0: slot
  `slot + i` gets `(cells[0], cells[1], 0, 0)` and the run is `(levels[0].vertices, instances, 0)`. With bounds, the
  grid's vec4 for instance i is `(x, y, z, s)` at `bounds.at + i` (anchor point, scale):
  - `w` = clip w of (x, y, z) (row 3 of `view_proj` dotted with (x, y, z, 1)); `p` = length of row 3's xyz (1 or 0);
    `r = |s| radius`; `fy` = length of row 1's xyz; pixels per unit at depth d: `height fy / (2 d)`.
  - behind if `w + r p <= near`: not drawn. Otherwise `across = 2 r ppu(max(w, near))`; under 1 pixel: not drawn.
  - `near_ppu = ppu(max(w - r p, near))`; level k's cell on screen: `cellPx(k) = |s| levels[k].cell near_ppu`.
  - fresh level: the largest k with `cellPx(k) <= target` (k only increases while it holds), from 0;
    `target = cellPixels`. With last frame's level `prev` for this instance (none at first): keep `prev` unless
    `cellPx(prev) > target 1.25` or `cellPx(prev + 1) <= target / 1.25` (when prev + 1 exists); then take fresh.
  - slot `slot + i` gets `(levels[L].cells[0], levels[L].cells[1], 0, 0)` for a drawn instance; remember L (or none).
  - runs: consecutive instances at one level form one run `(levels[L].vertices, count, first)`.
- `projected` (`slot`, `cell`, `plane`, `reach`, `lift`): the image of the plane y = `plane` out to view depth
  `reach`. `cellPx = cell (cellPixels > 0 ? cellPixels : 7)`. Unproject NDC (0, -1) at depths 0 and 1 to a ray, meet
  the plane (ahead only), lift that point by `lift`, project: `m = 0.02 + min(|ndc_y + 1| 1.25, 1)` (or `0.12` if the
  ray misses). Rows: NDC y in `[-1 - m, 1 + m]` whose ray (at NDC x = 0) meets the plane ahead within view depth
  `reach` (depth along `forward`); an interval with an end at one bound, the other end found by 40 bisections; none
  -> no run. `nx = max(1, ceil((1 + m) width / cellPx))`, `ny = max(1, ceil((y1 - y0) / 2 height / cellPx))`; slot
  `slot` gets `(nx, ny, y0, y1)`, slot `slot + 1` gets `(m, 0, 0, 0)`; one run `(6 nx ny, 1, 0)`.

`src/host/detail.ts` is the reference, line for line.

## Other loader calls

- **Curves.** `curves.buffer` holds `curves.floats` f32 (rows of 64 vec4 samples). A client may replace the whole
  contents with `writeBuffer` (an editor moving a curve); the length must not change.
- **Reading tables back** (tools): `dynamic` lists the per-frame table buffers (their labels are their names);
  `tables` says where each table lives (byte offset `offset * 16`) and how it is filled (`map` or `scan` at a level,
  or `replay` naming its recurrence).

## What is not in the file, and why

- JavaScript of any kind. A table filled by JavaScript (`cpu`) is refused by `loft build` by name, and counted by
  `loft build` and `loft render --artefact` ("n cpu (not data)").
- The CPU reading of the pose (`Artefact.pose`): for checks, baking and tests only.
- A bake. A scene's recurrences are baked once, on the GPU of the machine that runs `loft build`, and their states are
  static blobs; the step's bake module is not in the file. A file that keeps checkpoints carries the step only as the
  replay's three modules.
- A scene graph, meshes, materials, animation channels: there are none. Geometry is made in vertex shaders from
  tables; motion is compute stages; paint is fragment functions.

## Versions

`version` changes whenever a loader written for the previous one would draw a file wrongly or fail on it: a new field a
loader must act on, a changed meaning, a new `FrameWrite` or command kind. A field a loader may ignore does not
change it. The preludes are chunks, so changing them does not change the format.

**The rule: a mixed build is version 4. Otherwise, a file is version 3 when a draw command carries an `instanceBase`, else version 2 when it carries a replay or a pass, and version 1 otherwise, byte for byte as before.**
`src/host/lower.ts` selects the version. A loader for n reads every version before it; the TypeScript loader and
the Rust loader (rust/loft-loader) read all four.

- 1: the format above without `frame.replay` and without passes.
- 2 (2026-11-14): `frame.replay` and the table fill `replay` (Replays); textures of a scaled or fixed size, render
  passes with no colour attachment, pipelines with no fragment stage, `frame.copies` (Pictures drawn off screen). Each
  is optional; a file holding either kind uses version 2.
- 3 (2026-11-16): `instanceBase` on a `planned` command (shared draw shaders, src/host/share.ts; their rows are an
  ordinary static buffer at group 0 binding 5). A loader for 2 would ignore it and draw a shared draw with another
  draw's numbers. Written only when not 0; a file holding one uses version 3.

- 4 (2026-11-18): mixed shader and storage alternatives, plus the named f32 fallback.
