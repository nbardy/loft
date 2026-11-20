# Step v15_native — R6 Rungs 2–3: Cooperative Matrix, bk1b MSL Slots & MetalFX HostCallback

Read AGENTS.md, docs/V1_5_GOALS.md (R6, G5.4, G8.3, G8.4, G8.5), `docs/proposals/2026-11-20_capability_gaps.md` (`cap1` M4 proofs), and `agent_notes/2026-11-20_v1/lanes/bk1b.md`.

This stream merges `cap1d` + `v15_metalfx` into ONE owner for R6's native wgpu bridge rungs 2 & 3 (~400–700 LOC total; no full `objc2-metal` rewrite) while preserving a clean `HostCaps::WEB_BASELINE` compilation boundary on `wasm32`.

## Task

Work in `~/git/_wt/loft-v15-native` on branch `loft/v15-native` from `origin/main` (after `v15_exec` has merged).

1. **Rung 2a — `EXPERIMENTAL_COOPERATIVE_MATRIX` Neural / Field Inference (`cap1d`):**
   - Expose native `EXPERIMENTAL_COOPERATIVE_MATRIX` (`8×8×8` `f32`/`f16`, already verified on M4 in `wgpu 30.0.1` by `cap1`) behind `HostCaps` with clean scalar/ALU fallback on `HostCaps::WEB_BASELINE`.
   - Land a real Loft neural SDF / radiance-field inference witness scene + benchmark (`tools/quiet_gpu.sh`) proving cooperative-matrix dispatch beats the 512² ALU baseline (`0.238 ms p50`) or matches it with validated numeric output vs. CPU reference.
2. **Rung 2b — `bk1b` MSL Passthrough Auto-Slot Generator (`G5.4`):**
   - Finish automatic `[[buffer(N)]]` / `[[texture(N)]]` slot binding generation for `PASSTHROUGH_SHADERS` MSL kernels (`bfloat` / `simdgroup_matrix` / unchecked loops) and add a witness test compiled & executed on Metal.
3. **Rung 3 — `MetalFX` Upscaler via `PassRun::HostCallback` + `wgpu_hal::metal`:**
   - Expose a narrow `PassRun::HostCallback` hook in the runner schedule executor so macOS native passes can invoke a host Metal encoder callback via `wgpu_hal::metal` without bypassing the rest of the wgpu render graph.
   - Wire Apple `MetalFX` upscaling behind `cfg(target_os = "macos")` + `HostCaps::METAL_FX`, with an explicit fallback on `HostCaps::WEB_BASELINE`.
   - Measure an end-to-end upscaled scene on M4 via `tools/quiet_gpu.sh`: prove real MetalFX output (non-stub, visual/PSNR check, and frame-time win vs. native full-res).
4. **Web boundary check:**
   - `cargo check --target wasm32-unknown-unknown` MUST pass cleanly with zero Metal/ObjC leakage into `HostCaps::WEB_BASELINE`.

## Rules
- Use `git cambium add -b loft/v15-native -name v15-native -ephemeral ~/git/_wt/loft-v15-native origin/main`.
- Do NOT touch `stdlib/`, `corpus/`, or `crates/loft-cli/src/gates.rs`.
- Write `agent_notes/2026-11-20_v1/lanes/v15_native.md` (≤ 30 lines) with cooperative-matrix timing, MetalFX before/after timing, `wasm32` check result, and LOC added. Push `origin loft/v15-native`.

Final message ≤150 words.
