# Step v15_metalfx — R6 Rungs 2–3: MetalFX HostCallback + bk1b MSL Passthrough Slot Generator

Read AGENTS.md, docs/V1_5_GOALS.md (R6, G5.4, G8.4, G8.5), and `agent_notes/2026-11-20_v1/lanes/bk1b.md`.

## Task

Complete rungs 2b and 3 of R6's 3-rung wgpu native bridge (~300–500 LOC total; no full `objc2-metal` rewrite) while preserving a clean `HostCaps::WEB_BASELINE` compilation boundary on `wasm32`:

1. **Rung 3 — `MetalFX` Upscaler via `PassRun::HostCallback` + `wgpu_hal::metal`:**
   - Expose a narrow `PassRun::HostCallback` hook in the runner schedule executor so native macOS passes can invoke a host Metal command encoder callback via `wgpu_hal::metal` without bypassing the rest of the wgpu render graph.
   - Wire Apple `MetalFX` spatial (and temporal if motion vectors + depth are bound cleanly) upscaling behind `cfg(target_os = "macos")` + `HostCaps::METAL_FX`, with an explicit, clean no-op/upsample fallback on `HostCaps::WEB_BASELINE`.
   - Measure an end-to-end 1080p→4K (or 540p→1080p matched) scene on M4 via `tools/quiet_gpu.sh`: prove real MetalFX execution (non-stub output, PSNR/visual sanity, and frame-time win vs. native full-res).
2. **Rung 2b — `bk1b` MSL Passthrough Slot Generator (`G5.4`):**
   - Finish the auto-slot generator for `PASSTHROUGH_SHADERS` MSL kernels (so users don't hand-number `[[buffer(N)]]` / `[[texture(N)]]` slots to match wgpu bind groups) and add a witness test covering `bfloat` / `simdgroup_matrix` or unchecked-loop MSL passthrough cleanly compiled & executed on Metal.
3. **Web boundary check:**
   - `cargo check --target wasm32-unknown-unknown` must pass cleanly with zero Metal/ObjC leakage into `HostCaps::WEB_BASELINE`.

## Rules
- Use `git cambium add -b loft/v15-metalfx -name v15-metalfx -ephemeral ~/git/_wt/loft-v15-metalfx`.
- Keep changes isolated to host-callback / MetalFX / MSL-slot modules; do not touch `stdlib/`, `corpus/`, or `crates/loft-cli/src/gates.rs`.
- Write `agent_notes/2026-11-20_v1/lanes/v15_metalfx.md` with MetalFX before/after timings, wasm32 check result, and LOC added. Push `origin loft/v15-metalfx`.

Final message ≤150 words.
