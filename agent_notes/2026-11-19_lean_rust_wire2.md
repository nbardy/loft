# Lean Rust: wire 2, the decls, the fills and the prelude (19 Nov 2026)

Plan: `docs/proposals/2026-11-19_lean_rust_loft.md` §3.2 (pinned there as built), §1.4–§1.7, §1.9. Follows
`agent_notes/2026-11-19_lean_rust_driver.md` ("What blocks the cutover" 1–4). Branch `feat/lr-wire2` from `2726371`,
with main `b0a1f2b` (feat/lr-speed) merged in. Numbers below: M4, loaded (other lanes' gates), on the merged branch.

## Brief (written before the edits)

- **Blocker.** 0 of 295 corpus bundles could be drawn and 214 were refused by Naga. (a) The wire had no layout for
  Draw, Pass, Medium, Camera, Check or Solve decls. (b) A `Kernel` decl carried no fill body, so every table was
  `Init::Zero`. (c) Library raw WGSL read globals only the TS host declares (`loft_detail` 204, `view_dir` 2,
  `loft_cast` 2, `loft_passtex_*` 3, `loft_compute_*` 2, `soft_box` 1).
- **Owner:** the recorder (`src/record/*`), loft-ir `wire.rs`, decode and print, and emit's prelude. Not mine:
  emit/Naga call sites, f16 gating and loft-draw (feat/lr-speed, merged mid-lane); `solve.rs` (feat/lr-solve).
- **Expected.** Built rises from 27 toward R0's 238; drawn stays 0 until the library records Draw decls; parity is
  reported, not expected. **Stop rule:** two failed fixes in one subsystem. **Claim boundary:** counts on this machine.

## What

- **(b) Fills.** A table's `Kernel` decl is one of `Fill::{Map, Scan, Data, Recurrence}` (loft-ir `graph.rs`, TS
  `Fill` in `wire.ts`). Map runs and the scan carry their bodies as the decl's roots over its own binders (`K3r0i`,
  `K3s0`, `K3i`); supplied numbers go in `rows`; a recurrence's tables are a named variant (its stepping is not on the
  wire yet). The recorder builds the bodies while writing the job (`Recorder.useVariables`: expr.ts hands over
  `variable`, because recorder.ts cannot import a value from the module that imports it). The driver plans them
  (`fill_decls`): Const bodies run on the tape into `Init::Bytes`, the rest become one compute module each
  (`loft-core/src/fill.rs`, `@workgroup_size(64)` map, one-invocation scan), in schedule order. Lifted frame values
  are dropped (emit does not read them yet). A frame-filled table starts `Init::Zero` (the host runs `By` kernels at
  load only). WIRE = 2, addon stamp `abi 4 wire 2`.
- **(a) Decls.** Draw (tag 3), Pass (4), Medium (5), Check (6), Solve (7) and Camera (10) each decode into their
  own `Graph` array; Fn, kernel, medium and camera references and every role's signature are checked after all decls
  are read. TS shapes and the authoring entry `role(name, params, body)` are `src/record/decls.ts`; the writer is
  `Recorder.job(program, roots, scene)`. The driver lowers draws, media and passes through lane 3.1's `draw::lower`
  and exports each camera's parts as `loft_camera{k}_{part}`. Passes target the view only: an offscreen or shadow
  target needs a texture resource decl, which does not exist yet.
- **Print.** After `roots`: one `kernel` line per fetched table (fill walked after the roots, binders `fill TY SLOT`),
  then one line per scene-level decl (graph functions as `#body` over `arg TY K`, a solve's jets as `jet`). Both
  printers, grammar in `src/record/print.ts`. `src/record/sample.ts` writes one job with every scene-level layout.
- **(c) Prelude** (`loft-core/src/emit/prelude.rs`, bytes in `loft-ir/src/frame.rs`). One text at the top of every
  module: `frame` (camera matrices, eye, forward, viewport, time, sun, then params and inputs: the TS field names,
  because library raw WGSL reads them), `loft_detail` and `LOFT_DETAIL_SLOT`, `view_dir`, `pixel_world`,
  `sea_footprint`, `copy_id`, `quad_corner`, `scene_depth`/`scene_position`/`scene_clearance` over two group-0
  textures. Draw templates read `frame.*` (the separate `View` binding is gone) and their depth texture is `loft_depth`.
  `prelude::globals` runs every job (also under `Naga::Load`): a raw function with no text, or one reading a name
  the prelude does not declare, is `Scene { Global { name, needs } }` with `Needs::{Draw, Pass, Kernel, Table,
  Style, Prelude}`, each naming its fix. Time in generated code is `frame.time.x`.
- **loft-gpu** (outside my files, needed for the prelude): group 0 is the frame (header `loft_ir::frame::HEADER` =
  208 bytes), `loft_detail` (64 KiB, zero until the LOD plan is ported), and the two scene textures (1 × 1, far and
  clear, until the depth resolve is ported); `set_camera`. Its tests' hand-written prelude moved with it. The ignored
  test `corpus_bundles_load` loads every built bundle and draws one frame (`LOFT_BUNDLES`, `LOFT_BUNDLES_SHARD=k/n`).

## Pass / fail

| Check | Result |
|---|---|
| Rust print = TS print | **125/125**: the 124 example jobs (fills included) + `sample.ts`'s job with every scene-level layout. TS `printGraph` = `printJob` on all 124 |
| `tools/dev.sh test` loft-ir 10/10, loft-core 20/20 (incl. lane 2.1's ignored fill check on main's typed Tol), loft-gpu 6/6; lint and rustfmt clean; `pnpm typecheck` clean | pass |
| Corpus through the driver (`bun tools/parity/run.ts rust`) | **232 built** of 295 (was 27); 283 end as R0 did (was 81). 229 of R0's 238 builds build. The 9 R0 builds refused here, each by name: `loft_passtex_*` 3 (Pass), `loft_cast` 2 and `soft_box` 1 (Draw), `loft_compute_*` 2 (Kernel), `t_clay_clusters` 1 (Table) |
| loft-gpu loads the built bundles (`corpus_bundles_load`, 16 shards, f32, one frame, default device limits) | **26 of 232** load and run a frame (kernels dispatched; there is nothing to draw). 205 are refused at bind group layout creation: emit binds one storage buffer per table read and the default limit is 8 per stage. 1 exceeds the binding index limit. A frame-sized steps buffer fixed 4 others (the 208-byte header outgrew the old 256-byte minimum) |
| Drawn (loft-draw against R0, §2.2a) | **0 of 295.** No bundle has a draw: the TS library records no Draw decls yet (lane 3.1's TS half: hit, place and paint functions), and loft-draw still reads TS artefact directories |
| Parity passing | **0 of 295** (nothing drawn) |

## Left

1. **Draws:** the TS library as `role` graph functions recorded into Draw decls (lane 3.1's TS half). The wire,
   decode, print and driver path exist; `sample.ts` is the only job using them.
2. **Storage bindings:** emit binds one storage buffer per table read (`r{res}`), not layout's ≤5 packed buffers; most
   corpus modules exceed WebGPU's default 8 per stage, so loft-gpu refuses them at load (row above).
3. **Host:** the draw templates' `@group(2)` rows and textures have no host layout; the LOD plan, the depth resolve
   and cameras (bundle `cameras` names the exported parts) are not ported.
4. **Recurrences** (`Fill::Recurrence`) and author compute (`loft_compute_*`) as Steps/Raw kernels on the wire;
   offscreen Pass targets need a texture resource decl.
5. **Check/Solve programs:** decls decode; `run` still answers `Unbuilt` for them (analyses, feat/lr-solve).
6. **Cost:** a kernel module per GPU fill: Titan builds 824 modules (rust ≈ 4 s with Naga at emit); owl 62 kernels,
   rust 26.7 ms with Naga at load (the default since feat/lr-speed).

## Reproduce

`bun out/wire/dump_all.sh`-style dump (score.ts `--dump/--print` per example into a folder) plus
`bun src/record/sample.ts DIR`, then `LOFT_WIRE_CORPUS=DIR tools/dev.sh test loft-ir --run-ignored all`;
`bun tools/parity/run.ts rust` (needs `out/parity/R0` as a copy, not a symlink: seeds import `loft` by package name);
`LOFT_BUNDLES=$PWD/out/parity/rust/build LOFT_BUNDLES_SHARD=k/16 tools/dev.sh test loft-gpu --retries 0 --run-ignored only -E 'test(corpus_bundles_load)'`.
Gates on the merged branch: `check.sh quick` and `check.sh full` exit 0 (loft-draw built first: serve.rust needs it).
