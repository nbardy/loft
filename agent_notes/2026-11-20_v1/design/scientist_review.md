# Scientist Architecture & Performance Review — Actionable Design Directives (2026-11-20)

This design note records the Scientist's local code, shader, and M4 Metal measurement audit of `main` (`5527cba` / `f9602b5`) and directs the `s1_*` execution wave orchestrated by `agent_notes/2026-11-20_codex/sci_chain.sh`.

> **Coordination notice for `cook.sh` / `p2` Astra planners:** `sci_chain.sh` actively owns the six lanes below (`s1_lip`, `s1_controls`, `s1_web`, `s1_exec`, `s1_motion_fma`, `s1_escapes`) and their integrator `is1`. Do **not** duplicate these lanes or touch their owned files in `c2*` while `sci_chain.sh` is running (`! -e agent_notes/2026-11-20_codex/sci_chain.finished`).

---

## 1. Architectural Decisions & Cuts

1. **One Execution Path & Honest L0/L1 Boundary (`s1_exec`):**
   - `loft-std` currently depends on `wgpu = 30.0.1` + `wgpu-naga-bridge` and holds ~2,429 lines of device/encoder/layout code moved out of `loft-runner`, while `loft-cli/src/native/` maintains a duplicate 1,938-line renderer bypassing `Runner`.
   - **Rule:** `loft-std` must become **100% `wgpu`-free** (pure CPU twin, replay/stepping, layout validation). Move all GPU execution into `loft-runner` and delete the thin pass-through wrappers and duplicate CLI renderer.
   - **Honest ceilings (counting ALL `wgpu`-touching code, non-test):** `loft-ir <= 700`, `loft-format <= 950`, `loft-runner <= 2,200`, `loft-std <= 1,200` (zero `wgpu`), `loft-cli/src/native <= 400`.
   - **Bug fixes required in `loft-runner`:**
     - Derive per-table `BufferUsages` from pass bindings; never unconditionally stamp `BufferUsages::VERTEX` on LUT/storage buffers (`loft-runner/src/lib.rs:47`, which caused `wgpu-core` 1,122->1,123 word padding breaking `arrayLength(&luts) - 1122`).
     - Replace per-frame `device.create_buffer_init` allocations in `encode.rs:228-235` and `outbox.rs:66-70` with a reusable staging ring / `queue.write_buffer`.
     - Parallelize pipeline compilation inside `ComputeCache::new` (v3 regressed load 0.85s -> 3.14s by serializing).
     - Bump `COMPILER_ABI` to `2` (keep ABI-1 reader) adding `TextureBakeDecl` and `PathWorldDecl` so `.loftb` exports and executes bakes and path schedules (`crates/loft-passes/src/bundle.rs:325`).

2. **Motion CPU/GPU Twin Parity Without Shading FMA Leakage (`s1_motion_fma`):**
   - `d2` proved first state divergence is implicit Metal FMA contraction (`crab t=2 x`, `heron t=19 hip_y`, `boss t=8 windup`).
   - `pf1`/`bk1b` proved shading currently compiles with `MTLMathModeFast` (FMA ON) and turning contraction off in shading regresses Hermit Shade by `+0.184 ms` p50 / `+0.862 ms` p95 (global Safe mode `+4.680 ms`).
   - **Decision:** Do **not** emit a giant `u32` software IEEE-754 emulator in portable WGSL unless a non-Metal backend actually exhibits non-FMA rounding drift. Enforce exact non-contracting binary32 **strictly on `motion`/`solver` compute modules** (explicit `fma` paired with `f32::mul_add` in `det.wgsl`, plus contraction-barrier / non-contracting emission on motion ops only). Never touch shading modules; add an automated MSL check asserting shading keeps `MTLMathModeFast` and zero `contract(off)`.

3. **Lipschitz Orthogonal-Rotation Inflation (`s1_lip`):**
   - `engines/coral_shrine.loft:111-123` (`lantern_fold`) applies `abs(p)*1.45 - off` + an orthogonal Y-rotation 4 times, then divides by `4.420506 = 1.45^4`. True mathematical `L = 1.0`.
   - `lip.rs` + `eval.rs:1107-1130` sum scalar component bounds (`|cos|+|sin| = 1.3752`) and take Euclidean norm on every `vec3` constructor, inflating 4 folds to **`L = 22.871492`** (22.9x too large).
   - `pf1` AOVs showed `FoldedLantern` averages **129.7 steps** (p95 182, max 192 cap) with **100% of hits** in `Secant/L <= 0.3`, burning **1.066 ms** on 13,131 pixels, and truncating interior rays at step 192.
   - **Decision:** Add orthogonal-rotation / norm-preserving Lipschitz tracking in `lip.rs` and `eval.rs` (`rot_x`/`rot_y`/`rot_z` and exact 2D Givens/orthogonal matrix patterns) so rotated IFS folds derive `L <= 2.0`. Keep frozen Coral Shrine reference untouched; record candidate Shrine images + timing in `out/meta/s1_lip/` for owner look approval alongside a new non-golden unit/CLI test proving `L <= 2.0` and step count drop.

4. **Real Meta-Engine Escapes & `bake_tex` Sampling (`s1_escapes`):**
   - Wire real `bake_tex` sampling from `.loft` shaders (update `scenes/spectral_caustic_study.loft` so `StudyPaper.material` samples `StudyInk` instead of re-evaluating `study_ink(p.xz)` per hit).
   - Remove the hardcoded `"deposit_caustic"` exception in `crates/loft-passes/src/lib.rs:717` and lower general `kernel ... allow(atomics, workgroup, subgroups, dynamic_loops)` to `PassDecl::Compute`.
   - Lower whole-pass `pass name reads [...] writes [...] { ... }` (currently hard-refused E405 at `lib.rs:739-745`).
   - Lower `unproved(step_scale)` to 4-tap finite-difference normals + honest `Unproved(AuthorOptOut)` truth stamp (currently E405/unreachable in `eval.rs:509`, `ad.rs:97`, `emit.rs:510`).

5. **Control-Pass Gate (`s1_controls`):**
   - Start with **2** independently scheduled whole-pass hand-written WGSL controls (Hermit Shade + Shrine Vis) using ABBA ring-timestamp block bootstrap (`upper_95_ci(Loft / hand) <= 1.05`) + a synthetic 2x slowdown mutant that fails the gate. Defer the other 6 controls from `bk1` until these 2 prove low-noise stability.

6. **Real Browser `WebGpu` Executor (`s1_web`):**
   - Complete `web.rs` from its 93-line buffer-allocator stub into a real pipeline + bind-group + `encode_into` executor over a JS-owned `GPUDevice`, passing `wasm32-unknown-unknown` build with zero compiler crates and a headless browser proof.

7. **Explicitly Cut / Deferred (Do Not Schedule):**
   - Native `MetalBackend` (predicted saving only `0.09-0.19 ms` vs `2,300-3,850` LOC; below `0.30 ms` bar).
   - HDR+Present fusion, per-kind tile lists, `f16` shading, ground fBM bake, FMA-in-shading changes (`pf1` proved `0 ms` available on main).
   - Consumer Wasm compiler passes (`behaviour {}`) — defer until `check.rs` is refactored and built-in `kernel`/whole-pass lowering is live.
   - Phase 5.5 `env_stride` batching and 295-scene corpus grind until owner clears pending picture approvals.
