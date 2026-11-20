# Step v15_perf — R1 #2–3 (Sky Cube Binding + Shrine Cluster Culling) & R7 (Motion-Only FMA Barrier)

Read AGENTS.md and docs/V1_5_GOALS.md (R1 items 2 & 3, R7, G7.1). Do NOT touch BSDF ALU or march step counts (R1 rule). Inspect `origin/loft/x2c` and `origin/loft/x2b` for groundwork.

This stream merges the two small GPU/codegen performance fixes (`v15_sky` + `v15_fma`) under one owner so `loft-passes` / `loft-runner` changes land conflict-free.

## Task

Work in `~/git/_wt/loft-v15-perf` on branch `loft/v15-perf` from `origin/main` (after `v15_exec` has merged).

1. **R1 #2 — Finish the `x2c` Sky Bake Cube Binding:**
   - Wire the baked sky cubemap + sampler into the `Shade` pipeline bind group layout and WGSL shade code (fixing the missing cube binding on `loft/x2c`).
   - Re-bake only when sky/sun parameters change (`bake_dirty`); sample the cubemap in shading instead of evaluating the atmospheric integral per pixel.
   - **Target:** Hermit-20 uncontended GPU time `3.75 ms → ~2.5 ms` (paired A/B 95% CI via `tools/quiet_gpu.sh`), two-tier picture gate green.
2. **R1 #3 — Shrine `CoralBranch` Cluster Culling:**
   - Group Shrine `CoralBranch` instances into spatial clusters with coarse bounding spheres/AABBs checked before per-branch evaluation/draw so off-screen or coarse-missed clusters exit early.
   - **Target:** Shrine uncontended GPU time improves by `0.25–0.45 ms` (paired A/B via `tools/quiet_gpu.sh`), picture gate green.
3. **R7 — Motion-Only FMA Contraction Barrier (Replacing `x2b`'s 900-LOC Soft-Float):**
   - Isolate FMA-sensitive arithmetic ONLY in deterministic motion/kinematic compute kernels with a minimal contraction barrier (< 150 LOC; delete any `x2b` soft-float library if present). Keep all shading pipelines in `MTLMathModeFast`.
   - Add an automated MSL/WGSL leak-guard test proving motion kernels carry the barrier and shading pipelines do NOT.
   - **Target:** Motion twin **481 / 481** exact (`0` differing ticks) and 1,000-crab motion dispatch **≤ 0.06 ms** uncontended.

## Rules
- Use `git cambium add -b loft/v15-perf -name v15-perf -ephemeral ~/git/_wt/loft-v15-perf origin/main`.
- Do NOT touch `crates/loft-cli/src/gates.rs`, `tools/quiet_gpu.sh`, `corpus/`, or `stdlib/` language parser/HIR.
- Write `agent_notes/2026-11-20_v1/lanes/v15_perf.md` (≤ 30 lines) with paired uncontended timings (Hermit-20, Shrine, 1,000-crab), twin counts, and leak-guard result. Push `origin loft/v15-perf`.

Final message ≤150 words.
