# Lean Rust: packed tables, cameras, the scene resolve, loft-draw on loft-gpu (19 Nov 2026)

Plan: `docs/proposals/2026-11-19_lean_rust_loft.md` §1.5, §1.9, §2.2, lane 3.1/3.2/3.3. Follows
`agent_notes/2026-11-19_lean_rust_wire2.md` ("Left" 2 and 3). Branch `feat/lr-draw` from `7f48109`. M4, loaded by other
lanes (load1 5–15), so no timing here is a measurement.

## Brief (before the edits)

- **Blocker.** 232 of 295 corpus entries build through the driver, 26 load: emit bound one storage buffer per table
  read and 205 bundles passed WebGPU's default of 8 per stage. 0 draw: no camera layout, no host bindings for draw
  templates, loft-draw on loft-loader.
- **Owner.** loft-core emit's binding and `draw/*`, loft-gpu, loft-draw (and their TS callers: serve's rust route,
  `loft profile` under rust, `tools/parity/run.ts`). Not mine: the recorder, `wire.rs`, Check/Solve (wire3, solve2).
- **Expected.** Packed tables: ~231 of 232 load. Drawn and passing: bounded by what the TS library records (nothing).
- **Stop rule.** Two failed fixes in one subsystem. **Claim boundary:** counts on this machine.

## What

1. **Packed tables** (`80b11cf`). `layout` puts every rows resource in one of at most 5 storage buffers ("packs"):
   read-only tables apart from kernel-written ones (WebGPU refuses one buffer bound read-only and read-write in one
   dispatch), a stepped resource alone (the host swaps the buffer per step). Bundle V1 gains
   `packs: [Pack { access: Read | Write, vec4s, holds: [(resource, vec4 offset)] }]`; `Module.reads` is gone.
   Every module of a stage declares every pack at `@group(1) @binding(p)` (`loft_pack{p}`), so loft-gpu builds one
   group-1 layout for render pipelines (all read-only) and one for compute (written packs read-write). A `Fetch` reads
   `loft_pack{p}[OFFSET + min(row, ROWS - 1) * LANES + lane]`, constants in the text. Fill kernels write their pack in
   place; group 2 still binds a kernel's writes (rows: the whole pack; a step: its next state). loft-gpu `validate`
   refuses a pack overlap, a resource in two packs or none, a stepped resource sharing its pack, a kernel writing a
   read-only pack, and packs plus writes past the device's storage-buffer limit. Draw templates read rows through
   generated `loft_rows{res}(k)` readers, and the opaque scene's depth and colour through the prelude (new group 0
   binding 4, `loft_scene_colour`); draw modules have no `@group(2)` any more.
2. **Cameras** (`9531665`). Bundle `cameras: [Camera { name, lens: (module, entry) }]`. The driver writes one compute
   entry `loft_lens{k}` per camera into the root module, writing the lens record (`loft_ir::frame::LENS`: kind and
   fovy, then eye/target/up, or a matrix's four columns) from the camera's graph roots; a camera part that reads a free
   variable is `Scene { Unsupported }` naming it. loft-gpu has the one builder, `camera.wgsl` (TS `camera.ts`'s
   conventions: near/far at distance / 100 and × 100, the pole flip), run every frame and copied into the frame
   header before the frame kernels. `Loft::use_camera(name)` (bundle), `set_look` (host look camera, same path),
   `set_camera` (the host's own matrices, Keel). Instances get the loft's camera copied into each frame.
3. **Scene resolve.** Per view pass, before the first item that writes no depth after one that did, the host ends
   the render pass, copies the depth attachment into the r32float scene depth (a full-screen draw) and the colour
   into the scene colour, and continues loading both. A caller view whose attachments cannot be read is a typed
   `Usage` naming the fix. `loft_scene_clearance` stays 0 (its tile reduction is not ported).
4. **Screen covers never cull.** The full-screen triangle is clockwise; an opaque sky asking for culling drew
   nothing (found by drawing `src/record/sample.ts`'s job). Comment at the site in `draw/mod.rs`.
5. **loft-draw on loft-gpu.** `still`, `profile` and `serve` load `scene.loft` (a file or its directory). `--view`
   names a bundle camera. `profile` lost `--wait` and `--pose-time` (loft-gpu has no LOD bounds and no separate pose
   stages; its kernel pass is timed as "gpu kernel pass"). Serve wire 2: `{bundle, camera: named | look, ...}`.
   - `src/cli/serve.ts` rust route: the compile child builds with the engine that draws (`scene.loft` under rust);
     the supervisor checks params, inputs and cameras against the bundle's JSON head (`LoftHeader`, also what
     `--check` hooks receive under rust); fallback cameras front/side/three become host look cameras, orthographic
     ones are refused by name. The identity's build is the file's sha256; `FrameCamera` (identity.ts) records a
     bundle camera by name.
   - `loft profile` under rust builds through the driver and times with `loft-draw profile`.
   - `tests/serve.rust.test.ts` runs on `tests/fixtures/ball_bundle.ts` (no corpus scene records draws).
   - `tools/parity/run.ts rust` now draws every bundle with draws at R0's frames and compares each with
     `parity-diff --rule picture` (Dawn references `--rule cross`). `reference` and `repeat` refuse: R0 is recorded
     data, and they need loft-draw on TS directories (check out `7f48109` to draw a reference set again).
     `tools/parity/linux.sh` calls `reference`, so it refuses too.

## Known answer (first light of the path)

A unit sphere as a Ray cover's hit function, painted by its normal, seen by a bundle look camera from (0, 0, 5) with
fovy 0.8 at 256² (`tests/fixtures/ball_bundle.ts`). Predicted image radius `tan(asin 1/5) / tan 0.4 × 128` = 61.8 px,
centred at (128, 128), +x right and +y up. Measured: 61.9 px (area) / 62.0 (extent), centred, orientation as
predicted. The bundle camera and the same camera given as a host look camera give byte-identical frames (serve test).

## Counts

Of 295 corpus entries, on `e8638c7` plus this note (one full `bun tools/parity/run.ts rust`, `out/parity/rust/rust.json`):

| | before (7f48109) | now |
|---|---:|---:|
| built (driver, `--naga --precision mixed`) | 232 | **232** (283 end as R0 did) |
| loaded and one frame drawn by loft-gpu (`corpus_bundles_load`, f32, default limits) | 26 | **232** (predicted ~231) |
| drawn at R0's frames (bundles with draws) | 0 | **0** |
| passing §2.2a | 0 | **0** |

Every built bundle now binds 1 to 5 packs; the one bundle that also passed the binding-index limit loads too, since
packs replaced per-table bindings. Loads were run as 8 shards of the test binary (4 at a time, niced): 8 × 29 ok.

## Parity failure classes

Nothing is compared, so each class is why an entry has no frame:

| Class | Entries | Fix (owner) |
|---|---:|---|
| Built, no Draw/Pass/Camera decls: the TS library records none, so the bundle has no camera named as R0's frames are and nothing to draw | 232 | the TS library's hit/place/paint roles + `buildRust` passing the `Scene` (lane 3.1 TS half; "Left" 1) |
| Refused `load` as R0 was (library files without a default export) | 52 | none: same as R0 |
| Refused `scene`, R0 built: a raw WGSL global only a TS draw/pass/compute defines (`loft_passtex_*` 3, `loft_cast` 2, `soft_box` 1, `loft_compute_*` 2, `t_clay_clusters` 1) | 9 | Pass/Draw decls, texture resource decl, raw compute kernels on the wire |
| Refused `scene` as R0 was | 1 | none |
| Refused `internal` as R0 was | 1 | none |

(Three R0 CPU-table refusals, open, storm and stormfront, build here; they count in the 232.)

## What still uses loft-loader

- Nothing that draws: loft-draw, serve, profile and the parity runner are on loft-gpu.
- The crate itself (workspace member, its `loft-view` binary), and `tools/rust_compare.ts` (loft-view against Dawn on
  TS directories), which nothing in the gates runs. Mentions only: `tools/check.sh` (layers list), `gate_flags.sh`,
  `rust/Cargo.toml`, `loft-gpu/src/cache.rs`, `tools/parity/diff.rs`. The crate can be deleted at the cutover with
  `rust_compare.ts`; its LOD plan (`plan.rs`) is the source for porting `loft_detail`.

## Left

1. **The TS library's draws (lane 3.1's TS half)** are the whole of the parity gap: `src/draw/*.ts` and `src/soft/*`
   as `role` hit/place/paint functions recorded into Draw/Pass/Camera decls, and `buildRust` passing that `Scene` to
   `rec.job`. Until then no corpus bundle carries a camera or a draw. Shading parity then needs the styles' raw WGSL
   (`shade`, `ink`, `sky`, `sun`: `Needs::Style`) to reach the templates.
2. **LOD** (`loft_detail` stays zero): needs per-draw detail slots and bounds in the bundle; port from loft-loader
   `plan.rs` when grid draws exist. **Clearance** tile reduction. **Offscreen/shadow passes**: no texture resource
   decl, and textures are not readable at group 1 yet.
3. **Load time**: a bundle with hundreds of fill modules (Titan, crab lineup 73 MB) takes minutes to load here (Naga
   in `validate`, Naga again in wgpu, a Metal compile per pipeline); one module per fill kernel is the cost.

## Reproduce

`bun tools/parity/run.ts rust` (needs `out/parity/R0` copied, `tools/dev.sh build loft-node --fast`,
`tools/dev.sh build loft-draw --fast`, `cargo build --release --manifest-path tools/parity/Cargo.toml`); loads:
`LOFT_BUNDLES=$PWD/out/parity/rust/build LOFT_BUNDLES_SHARD=k/8 <loft-gpu it binary> --exact corpus_bundles_load
--ignored --nocapture` (run directly: under `dev.sh test` the light compile lock is held for the whole run).
