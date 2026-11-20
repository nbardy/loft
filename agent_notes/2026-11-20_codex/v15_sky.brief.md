# Step v15_sky — R1 #2 (x2c Sky Cube Binding) & R1 #3 (Shrine CoralBranch Cluster Culling)

Read AGENTS.md and docs/V1_5_GOALS.md (R1 items 2 & 3, pending `loft/x2c`). Do NOT touch BSDF ALU or march step counts (R1 explicit rule).

## Task

Work in `~/git/_wt/loft-v15-sky` on branch `loft/v15-sky` starting from `origin/main` (inspect `git show origin/loft/x2c` for the sky-bake groundwork already written, and pick/adapt cleanly onto `origin/main` after `v15_exec`).

1. **R1 #2 — Finish the `x2c` Sky Bake Cube Binding:**
   - Wire the baked sky cubemap + sampler into the `Shade` pipeline bind group layout and WGSL shade code (fixing the missing cube binding noted on `loft/x2c`).
   - Re-bake only when sky/sun parameters change (`bake_dirty`), sampling the cubemap in shading instead of evaluating the atmospheric integral per pixel.
   - **Target:** Hermit-20 uncontended GPU time `3.75 ms → ~2.5 ms` (verify via `tools/quiet_gpu.sh` with paired A/B 95% CI, same resolution and scene), picture gate passes under the `v15c` two-tier gate.
2. **R1 #3 — Shrine `CoralBranch` Cluster Culling:**
   - Group Shrine `CoralBranch` instances into spatial clusters with coarse bounding spheres/AABBs checked before per-branch evaluation/draw so off-screen or coarse-missed clusters exit early.
   - **Target:** Shrine uncontended GPU time improves by `0.25–0.45 ms` (paired A/B via `tools/quiet_gpu.sh`), picture gate passes with zero visual regressions.

## Rules
- Use `git cambium add -b loft/v15-sky -name v15-sky -ephemeral ~/git/_wt/loft-v15-sky`.
- Do NOT touch `crates/loft-cli/src/gates.rs`, `tools/quiet_gpu.sh`, or `loft-std` core language parser/HIR.
- Write `agent_notes/2026-11-20_v1/lanes/v15_sky.md` with paired before/after uncontended GPU timings (Hermit-20, Shrine, Table A/B) and push `origin loft/v15-sky`.

Final message ≤150 words.
