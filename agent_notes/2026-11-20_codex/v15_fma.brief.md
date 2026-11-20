# Step v15_fma — R7 Motion-Only FMA Contraction Barrier (Replacing x2b Soft-Float)

Read AGENTS.md, docs/V1_5_GOALS.md (R7, G7.1), and inspect `origin/loft/x2b` (`git log origin/main..origin/loft/x2b --stat`).

## Task

Replace `x2b`'s heavy 900-LOC `u32` software-float workaround with a surgical, motion-only FMA contraction barrier while keeping shading at full fast-math speed:

1. **Scoped motion-only contraction barrier:**
   - Isolate FMA-sensitive arithmetic ONLY in deterministic motion/kinematic kernels (where CPU twin vs. Metal GPU diverged on `ffma` contraction), using a minimal contraction barrier (e.g. MSL `[[precise]]` / split mul+add barrier or WGSL lowering rule applied strictly to `Motion` / `Exact` compute shaders, NOT fragment/shading pipelines).
   - Keep all shading pipelines in `MTLMathModeFast` — zero shading throughput regression.
2. **Automated MSL shading-FMA leak guard:**
   - Add a unit/CI test inspecting emitted MSL/WGSL for shading pipelines vs. motion kernels to prove:
     - Motion kernels carry the contraction barrier (no unguarded `fma` drift).
     - Shading shaders do NOT emit the motion barrier or disable fast-math.
3. **Prove both accuracy and speed (uncontended via `tools/quiet_gpu.sh`):**
   - Twin gate: **0 / 481** differing ticks across the full motion twin suite (481/481 exact).
   - 1,000-crab motion dispatch: **≤ 0.06 ms** uncontended (vs. `x2b`'s over-budget soft-float).
   - Net code size: delete the ~900-LOC soft-float library if present; implementation should be compact (< 150 LOC).

## Rules
- Use `git cambium add -b loft/v15-fma -name v15-fma -ephemeral ~/git/_wt/loft-v15-fma`.
- Stay strictly in motion-kernel codegen/runtime + its tests; do not touch shading pipelines or `stdlib/`.
- Write `agent_notes/2026-11-20_v1/lanes/v15_fma.md` with twin counts, 1,000-crab timing, and leak-guard test output. Push `origin loft/v15-fma`.

Final message ≤150 words.
