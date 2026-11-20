# Step v15_exec — R2 One Execution Path, wgpu-Free loft-std & Lines Ratchet (SOLO)

Read AGENTS.md and docs/V1_5_GOALS.md (R2, G5.1, G5.2). You are the **sole** core-refactor owner (per R2: never run parallel lean lanes again) and you run alone before wave15_2 phase 2 starts.

## Task

1. **Merge ONE execution path, fold the held Wave-15 branches (`v15a`/`v15b`/`v15c`), and delete the duplicate CLI renderer** (building on what `loft/x2a` proved, starting cleanly from current `origin/main` after `wave4` merged):
   - Unify `loft-runner` (`crates/loft-runner`) and `loft-cli` (`crates/loft-cli`) onto a single execution path so `loft-cli/native` drops to **≤ 450 scc lines** and `loft-runner` drops to **≤ 2,200 scc lines**.
   - Fix the known `x2a` leftovers: eliminate the per-frame buffer allocation (`encode.rs:228`) and the `U::VERTEX` padding hazard (`lib.rs:47`).
   - **Fold the 3 held Wave-15 branches** (see `out/meta/iv15/BLOCKED` — all 3 already did 90% of the work and only stalled on cross-crate bindings you own):
     - **`origin/loft/v15a` (`f83ecd6`):** keep `(hit, px, world)` `shade_tiles` + raster vis-buffer `realtime_matched`, AND wire Metal `EXPERIMENTAL_RAY_QUERY` accel-structure bindings in `loft-runner` (`Limits::default().using_acceleration_structure_values(adapter.limits())` per `cap1`) so secondary shadow/GI rays use hardware RT (`Table B < 15 ms` uncontended).
     - **`origin/loft/v15b` (`bf1e54f`):** keep `kernel allow` + `NonDet` physics tier, AND fix StudyInk group 3 binding 3, portable baked-program export, and the 2-word raw storage `arrayLength` padding.
     - **`origin/loft/v15c` (`699216e`):** keep `tools/quiet_gpu.sh` + two-mode scorer, hoist `Deterministic(tol)` / `Stochastic` into `crates/loft-cli/src/gates.rs` (R8), and tighten the triangle preflight gate to **`p95 < 0.150 ms`**.
2. **Make `loft-std` 100% `wgpu`-free** (`cargo tree -p loft-std -i wgpu` returns nothing). Keep shader/stdlib sources and typed contracts in `loft-std`; keep all `wgpu` runtime handles in `loft-runner` / GPU crates.
3. **Hit the R2 Wave-1 core ceiling and add the ratchet:**
   - Total non-test Rust core scc code lines **≤ 18,000** (`scc` on `crates/` excluding `tests/`).
   - Add `./tools/check.sh lines` reporting exact per-crate scc counts and failing if core exceeds the ratchet (18,000 for wave 1, `loft-std` wgpu dep count > 0, `loft-runner` > 2,200, `loft-cli/native` > 450).
4. **Verify:**
   - Run `cargo test --workspace` and the full picture + twin gates (`tools/quiet_gpu.sh` if present) on a release build. All deterministic gates must pass; no regressions in Speed/Load.
   - Commit on branch `loft/v15-exec` in worktree `~/git/_wt/loft-v15-exec`, push `origin loft/v15-exec`, and **fast-forward/merge cleanly into `origin/main`** ONLY if all tests & gates are green (or leave on `origin/loft/v15-exec` with exact blockers if anything fails).

## Speed rules
- Use `git cambium add -b loft/v15-exec -name v15-exec -ephemeral ~/git/_wt/loft-v15-exec` (it inherits the cargo seed).
- Batch edits; iterate `cargo check` first, then run targeted tests and one final gate verification.
- Write a concise lane note to `agent_notes/2026-11-20_v1/lanes/v15_exec.md` with before/after scc line counts per crate and `cargo tree -p loft-std -i wgpu` output.

Final message ≤150 words.
