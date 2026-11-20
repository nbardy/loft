Effort: high

# s1_escapes · Real `bake_tex` Sampling, General `kernel ... allow(...)`, Whole-Pass WGSL `pass`, and `unproved(step_scale)`

You are a `gpt-6.2-sol` engineer. Repo `~/git/loft`. Read `CORE.md`, `docs/PHILOSOPHY.md`, `docs/spec/README.md` (`07` > `PATCH_V1_2` > `06` > `00-05`), `docs/proposals/2026-11-20_backends_and_escape_hatches.md` (§2 Raw WGSL Passes), and `agent_notes/2026-11-20_v1/design/scientist_review.md`.

## Setup & Rules
- From `~/git/loft`, run `git fetch origin main` and create a Cambium worktree:
  `git cambium add -b loft/s1-escapes -name loft-s1-escapes -ephemeral ~/git/_wt/loft-s1-escapes origin/main`.
  Record the base SHA (must include `i1` and `ipf` if merged). Work only inside that worktree.
- **Owned files:** `crates/loft-passes/src/{check.rs,bake_tex.rs,ad.rs,eval.rs,lib.rs,paths.rs,emit.rs}` (scoped strictly to `bake_tex` sampling, `kernel` lowering, whole-pass WGSL `pass` lowering, and `unproved(step_scale)` — do NOT touch `pf1b`'s traced march grouping or `s1_lip`'s Lipschitz rotation math), `scenes/spectral_caustic_study.loft`, new end-to-end tests in `crates/loft-passes/tests/` and `crates/loft-cli/tests/`, and `agent_notes/2026-11-20_v1/lanes/s1_escapes.md`.
- Write your prediction first. Iterate `CARGO_BUILD_JOBS=2 tools/dev.sh check loft-passes -p loft-cli --all-targets` until clean. Never escalate on a compile error. Allow at most 2 full test/GPU reruns under `tools/gpu_lock.sh`.
- Never alter frozen golden references for shipped scenes (`capsules`, `blobs`, `hermit`, `coral_shrine`, `crab`, `boss`, `heron`).
- Commit by path with trailer `Co-Authored-By: GPT (codex) <noreply@openai.com>`, push `loft/s1-escapes`, preserve evidence in `~/git/loft/out/meta/s1_escapes/`, and remove the worktree with `git cambium remove --safe loft-s1-escapes`.

## Task
Close the four major v1.2 / §07 front-end stubs identified in the Scientist audit so Loft's meta-engine seams work end-to-end instead of failing with `E405`:

1. **Real `bake_tex` texture sampling in `.loft` shaders:**
   - Allow `sample(TexName, uv)` / `sample_lod(TexName, uv, lod)` (matching `docs/spec/07` / `loft.lark`) on declared `bake_tex` targets inside `material` / `trace` / `kernel` code.
   - Bind baked textures cleanly in `@group(3)` via pass-derived `PassDecl.bindings` (both native CLI and `Bundle` lowering).
   - Update `scenes/spectral_caustic_study.loft:51` so `StudyPaper.material` actually samples the baked `StudyInk` texture instead of re-evaluating the 64-segment procedural `study_ink(p.xz)` per hit! Verify visual fidelity and speedup.
2. **General `kernel name(...) allow(...)` lowering (remove `"deposit_caustic"` hardcode):**
   - Remove the hardcoded `k.name.name != "deposit_caustic"` bypass at `crates/loft-passes/src/lib.rs:717` and the unconditional `pending_lowering` stamp at `check.rs:395`.
   - Lower arbitrary checked `kernel` declarations (with `allow(atomics, workgroup, subgroups, dynamic_loops)`) to real `PassDecl::Compute` WGSL shaders preserving typed table bindings, space/unit checking, and `Det` vs `NonDet` tagging (integer atomics = `Det`, float atomics = `NonDet`).
3. **Whole-pass custom WGSL `pass name reads [...] writes [...] { ... }`:**
   - Remove the unconditional `E405` refusal at `crates/loft-passes/src/lib.rs:739-745`.
   - Compile user-declared raw WGSL passes with custom `table` bindings into `PassDecl::Compute`, validating the resulting WGSL via Naga and generating the declared `@group(1)`/`@group(2)`/`@group(3)` bindings matching `PassDecl.bindings` (with honest `Truth::Unknown` unless a CPU twin is provided).
4. **Real `unproved(step_scale)` lowering inside `trace`/`surface`:**
   - Remove the `E405` / `unreachable!` blockers in `check.rs:886`, `eval.rs:509`, `ad.rs:97`, and `emit.rs:510`.
   - Lower `unproved(step_scale) { ... }` using the explicit `step_scale` for sphere-trace stepping, 4-tap tetrahedron finite-difference normals (no AD failure), and honestly stamping the kind's truth record `Unproved(AuthorOptOut)` instead of `Held` or `E211`/`E307`.

## Score
- **End-to-end integration test (source -> `loft-passes` -> `.loftb` / GPU execution under `tools/gpu_lock.sh`)** covering ALL FOUR features in one or more real `.loft` scenes:
  1. A custom `kernel` NOT named `"deposit_caustic"` using fixed-point `i32` atomics (`Det`) + a float-atomic variant tagged `NonDet`, verified by GPU readback against CPU expected values.
  2. `bake_tex` sampled in a shader (`scenes/spectral_caustic_study.loft` sampling `StudyInk` + a unit test checking `textureSample` in emitted WGSL).
  3. A custom whole-pass WGSL `pass` reading one `table` and writing another, compiled through `loft-passes`, validated by Naga, and executed on GPU with verified readback.
  4. An `unproved(0.5) { ... }` sharp non-Lipschitz SDF compiling cleanly, rendering non-blank normals/beauty, and reporting `Unproved(AuthorOptOut)` in `loft truth --json`.
- All existing grammar tests (280/280), `loft-passes` tests, 6 mutants, and baseline 18/18 shipped scene digests remain unchanged.
- Save all evidence under `~/git/loft/out/meta/s1_escapes/` and summarize in `agent_notes/2026-11-20_v1/lanes/s1_escapes.md`.

Final message <= 120 words.
