# Loft Rework Plan — 27 Nov 2026

## Strategic Decision: Rework Underneath, Shrink Core Rust 29.4k → ~10k, Grow Clean Composable `stdlib/`

**Keep the compiler — it IS the moat.** Delete the 60% of accumulated agent slop underneath it, and move every baked engine/renderer/motion convention out of Rust into a rich, well-organized, composable `stdlib/` (which is explicitly **not** penalized for size — size is judged strictly on the Rust core!).

### Target Architecture (~9.6k Rust Core LOC, down from 29,362)
- **`loft-lang` (~5.5k, down from 14.9k):** Lean parser (no `cst.rs`), clean type/`@Space`/`writes`/E504 checker, stage-0 `build` + sound interval/Lipschitz + clean AD, structured AST→WGSL emitter + generic schedule lowering. Zero hardcoded Vis64/shadow/shade_tiles/woodblock/spectral_path/world_medium string surgery.
- **`loft-runner` (~1.8k, down from 7.3k):** ONE generic `wgpu` pass-graph executor (native + WebGPU). Delete `loft-native`, `loft-wgpu/web.rs`, collapse `loft-std`+`loft-wgpu`+`loft-runner`, move `compute_twin.rs` & `loft-gltf` out of core.
- **`loft-cli` (~1.2k, down from 2.4k):** Thin driver (`check`, `show`, `build`, `still`, `clip`, `profile`, `context`, `fork`); move `examples/` & verification harnesses into `tools/loft-verify/`.
- **`loft-abi` + `loft-ir` + `loft-format` (~1.1k, down from 1.7k):** Generic wire records ONLY (strip `CreatureHeader`/`FootPin`/`ManipState`/`TrailNode` out of `loft-abi`, delete unused `Op::Intrinsic`/`IrModule`/`PassRegistry`).
- **`stdlib/` (uncapped, composable, well-organized):** All engines (`render/`, `engines/`), shapes (`shapes/Capsule`, `GlossBead`), SDFs (`ellipsoid`, `smax`), materials (`bsdf`, `chitin`, `fur`), lighting, and modular motion live here as ordinary, forkable `.loft` + `.wgsl`.

**Do NOT break the `.loft` surface API.** Existing rollouts (`out/spark_pairs/models/*`, `out/spark_pairs/loft3d/*`, `out/spark_pairs/chains/*`, `corpus/` 295 Phase 5 fixtures, `engines/gates.json` 19 pinned frames, `scenes/`, `tests/characters/`) all speak the same clean E+ `.loft` surface (`space`, `kind`, `build`/`emit`, `schedule`, `law`, `check`, `table`/`pass`/`kernel`).

The architectural defects live **underneath** that surface:
1. Stock schedule verbs (`render_bundle`, `find_shadow_map`, `shade_tiles`, `screen_pass`, `orbit`) are privileged Rust code instead of ordinary `.loft` library functions with the exact same signatures.
2. `AuthoredCompute` and raster/render schedules are artificially siloed in `loft-passes`.
3. `loft-wgpu` seals away standard GPU raster state (back-face culling, alpha blending, custom samplers, vertex buffers, custom render targets).
4. `loft-emit` stitches pipelines via brittle WGSL string `.replace(...)` surgery instead of structured lowering.
5. Four missing additive language features (`v3@Space` literals, small fixed arrays, struct returns, bitwise/shift ops) force grotesque workarounds in `stdlib/motion/`.

### Rollout & API Compatibility Contract (Non-Negotiable)
Every wave MUST preserve:
- **100% parse + compile + render compatibility** for all `out/spark_pairs/**/*.loft`, `corpus/**/*.loft`, `scenes/**/*.loft`, `stdlib/**/*.loft`, and `tests/characters/**/*.loft`.
- Specifically: `p3@World + [x, y, z]` (used on every `emit Capsule(...)` line in `out/spark_pairs/`) MUST stay valid; `stdlib/engines/{capsules,blobs,hermit,coral_shrine,realtime,woodblock,reference,spectral_caustic}.loft` paths and frozen `engines/gates.json` RGBA hashes MUST stay green.
- All new syntax/runtime capabilities are **strictly additive** (using backward-compatible `Execution` descriptor extensions in `loft-ir/src/execution.rs` rather than breaking `LOFTB001`).

---

## Part 1: Making Loft a Real Meta-Engine

Ordered so every step buys a concrete user capability while keeping all existing rollouts running unchanged.

### Milestone M1 — Unify the Pass Graph & Open Up `loft-wgpu` (Wave 1, in flight as `r27-meta-passgraph`)
- **Remove the compute/render silo** (`loft-passes/src/lib.rs`, `bundle.rs`, `authored_compute.rs`): allow a single `schedule` and `.loftb` bundle to freely mix `table(...)`, `pass ... { compute(...) / dispatch(...) }`, `bake_tex`, `run_laws`, and raster/render passes.
- **Stop forcing dummy raster passes** (`loft-passes/src/pipeline/schedule.rs:124-133`): `screen_pass` and standalone compute schedules no longer need fake `render_bundle`/`shade_tiles`/`sky` entries.
- **Open up `loft-wgpu` raster state** via backward-compatible `loft_ir::execution::Execution` v3 extensions:
  - Support `CullMode::None | Front | Back` (not just `None | Front`).
  - Support `BlendMode::Opaque | Additive | Alpha` (standard premultiplied/straight alpha).
  - Support configurable sampler filter/address modes (`Nearest | Linear`, `Clamp | Repeat`).
  - Allow optional vertex buffers (`draw.vertex.is_some()`) without rejecting them in `loft-wgpu/src/schedule.rs:235`.

### Milestone M2 — Real `target` Declarations & Arbitrary Multi-Pass Render Graphs (Wave 2)
- Implement `target Name: Format Extent [msaa N]` (already in `grammar/loft.lark:30`, currently `E405` in `parser.rs:625`).
- Generalize `FramePass` (`crates/loft-passes/src/frame_pass.rs`) so passes can read/write **any** declared `target` or `table` (not just the 9 hardcoded `FrameView` names), and `draw(...)` can target any declared `target` (not just `Swapchain`).
- Add `PassRun::Render` / `Raster` lowering for arbitrary user-defined color + depth target sets.
- Now a user can write a G-buffer, shadow cascade, bloom chain, TAA pass, or fluid-plus-render pipeline purely in `.loft` + WGSL without touching Rust.

### Milestone M3 — Demote Hardcoded Engines to Ordinary `stdlib/` Recipes (Wave 3)
Keep existing verb names and call signatures identical (so zero rollouts break), while moving implementation out of Rust into readable, forkable `stdlib/render/` modules:
- `orbit` / `use_camera` → `stdlib/render/camera.loft`
- `find_shadow_map` → `stdlib/render/shadow.loft` (using `on dirty(Casters)` and `for k in Casters`)
- `shade_tiles` + `outline_and_paper` → `stdlib/render/shade.loft` + `display.loft` (delete `defaults.rs`'s secret `woodblock(...)` rewrite and `emit.rs`'s hardcoded `"woodblock_pigment"` / `"sun_sky"` / `"spectral_dielectric"` branches)
- `screen_pass` / `spectral_path.wgsl` → `stdlib/render/path.loft` + `stdlib/engines/spectral_caustic.loft` (delete the hardcoded `abs(h.p.y) < 0.015` photon map from `loft-emit`!)
- Move `CreatureHeader` / `FootPin` / `ManipState` / `TrailNode` out of `crates/loft-abi/src/lib.rs` into `stdlib/motion/` once `loft-abi` only carries generic wire primitives (`BvhNode`, `LeafRef`, `VisRec`, `VerdictRecord`, `MeshRow`, `AdvanceArgs`, `det`).
- Unify browser execution (`loft-wgpu/src/web.rs`) onto the same `wgpu` WebGPU backend path as native instead of a separate half-implemented JS metadata dumper.

---

## Part 2: Codebase Cleanup on the Path There

### Cleanup Track C1 — Compiler Soundness & Correctness (Wave 1, in flight as `r27-soundness`)
- Fix provably unsound `map2` Lipschitz bound (`loft-eval/src/eval.rs:1387-1395`) and `sqrt(n)` broadcast factor.
- Fix `Stmt::For` mixed `i32`/`u32` bound swap (`loft-eval/src/eval.rs:504`).
- Fix `ad.rs:56` shallow-clone node ID bug and `BinOp::Rem` derivative (`ad.rs:167`).
- Fix `check.rs:1619` so `<`/`<=`/`>`/`>=` only accept numeric types.
- Enforce transitive E504 determinism check on `Det` `kernel`s (`check.rs:104-120`) and make `NonDet` sticky in `loft-passes/src/motion.rs:60`.
- Fix duplicate `add_media` inside `PointLamp` loop (`stage0.rs:149-159`).
- Fix `loft-format/src/bundle.rs:214-231` `validate_wgsl` capabilities.
- Add `det_log` to `crates/loft-abi/src/det.rs` and align `det_sin(-0.0)` with WGSL `sign()`.

### Cleanup Track C2 — Runtime Hot-Path & GPU Ray Perf (Wave 1, in flight as `r27-perf`)
- Precompute `inv_d = 1.0 / ray.d` in `bvh.wgsl` and `mesh.wgsl`; add near-child ordering in `mesh.wgsl`.
- Eliminate per-frame heap allocations in `loft-runner/src/encode.rs` and `embed.rs` (cached `Canvas` comparison, precomputed initializer `BTreeSet`, skip empty param vecs, no `wave.to_string()` per batch, dirty-tracked `Inputs::advance` in `loft-std/src/inputs.rs`).
- Later (Wave 2): SAH BVH build in `loft-eval/src/bvh.rs`, GPU-side animated bound update so `loft clip` stops recompiling every frame, and deduplicate the 4 ray-AABB / 3 BVH implementations.

### Cleanup Track C3 — Honest Gates, CLI Consistency & Grammar Sync (Wave 1, in flight as `r27-gates-and-grammar`)
- Fix `tools/loft-verify/src/suite.rs:752-761` so contended Speed gates do **not** silently exit 0 (report explicit contention failure / exit code 6, make quiet threshold adapter-calibratable or overridable).
- Update `tools/loft-verify/src/controls.rs` to use `load_control` (before + after) instead of the deprecated 1-triangle probe, and filter `0` timestamps from skipped shadow passes in `whole_frame_ms`.
- Fix `loft render --time T` (`crates/loft-cli/src/render.rs`) to sample motion snapshots like `loft still`/`loft clip`; unify quantile helpers across `loft-cli`.
- Sync `grammar/loft.lark` + `docs/spec/loft.lark` with `parser.rs` (additive: bitwise ops, `u`/`i` suffixes, fieldless `kind`, leading-operator continuation).

### Cleanup Track C4 — Additive Language Ergonomics & Stdlib De-Bloat (Wave 2, after `loft/integrate-1125` + Wave 1 merge)
- Add 4 strictly additive language features:
  1. Explicit `v3@Space` vector literals (e.g. `vec(x, y, z)@Space` or `v3(x, y, z)@Space`) while keeping `p3 + untyped [x, y, z]` valid for existing rollouts.
  2. Small fixed-size arrays on `kind` rows and locals (`[f; 8]`).
  3. User `struct` return values in functions and CPU twin.
  4. Bitwise/shift operators (`<<`, `>>`, `&`, `|`, `^`) in surface grammar + `loft-eval` + `loft-emit`.
- Then delete ~1,500 lines of duplicated/contorted stdlib boilerplate: merge `stdlib/motion/gait.loft` + `legs.loft` + `scenes/showcase/std1e/gait.loft`, replace `Mood = frame@World` with `struct Mood`, remove hardcoded crab claws from generic `Walker`, move demo scenes (`capsules`, `blobs`, `hermit`, `coral_shrine`) out of `stdlib/engines/` (or keep thin re-export shims if anything imports them), and delete dead styles/imports.
- Replace `loft-emit`'s string-surgery `.replace(...)` pipeline stitching (`shade_tiles.rs`, `paths.rs`, `bundle.rs:168`) with structured AST/module composition.
- Make physical units either checked across all `fn`/`kind`/`law` bodies (with opt-in gradual mode so rollouts stay green) or cleanly document where units are checked; remove the 24-file hardcoded stdlib whitelist from `scope.rs`.

---

## Wave 1 Parallel Execution (`gpt-6.2-sol`, `medium` effort)

Four strictly disjoint lanes launched immediately off `main` (`f858d61f`), zero overlapping files, zero rollout-breaking API changes:

| Lane | Branch | Owned Files | Rollout Impact |
|---|---|---|---|
| `r27-soundness` | `rework/2026-11-27-soundness` | `loft-eval/src/{eval,ad,lip,refine}.rs`, `loft-check/src/check.rs`, `loft-passes/src/{motion,pipeline/stage0}.rs`, `loft-format/src/bundle.rs`, `loft-abi/src/det.rs` | None (pure bug/soundness fixes) |
| `r27-perf` | `rework/2026-11-27-perf` | `loft-runner/src/{encode,embed}.rs`, `loft-std/src/inputs.rs`, `loft-emit/src/{bvh,mesh}.wgsl` | None (bit-identical pixels) |
| `r27-meta-passgraph` | `rework/2026-11-27-meta-passgraph` | `loft-ir/src/execution.rs`, `loft-wgpu/src/{command,storage,schedule}.rs`, `loft-passes/src/{lib,bundle,pipeline/schedule,authored_compute}.rs` | None (100% additive) |
| `r27-gates-and-grammar` | `rework/2026-11-27-gates-and-grammar` | `tools/loft-verify/src/{suite,controls}.rs`, `crates/loft-cli/src/{render,lib,profile,clip}.rs`, `grammar/loft.lark`, `docs/spec/loft.lark`, `tools/check_loft_grammar.py` | None (100% additive) |
