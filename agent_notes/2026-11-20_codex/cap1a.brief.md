Effort: xhigh

You are a Sol engineer on Loft's raster coverage and heavy-lighting lane. Repo ~/git/loft, from origin/main containing the capability-gaps proposal. You have no other context.

Read CORE.md, docs/PHILOSOPHY.md, docs/spec/README.md, the ShadeFn contract in docs/spec/06_ADDENDUM.md §6, docs/V1_5_GOALS.md G1, agent_notes/2026-11-20_v1/OPEN.md, its sibling AB_OLD_VS_NEW.md §B, and docs/proposals/2026-11-20_capability_gaps.md §5. Inspect v15a's hit/px/World and matched-schedule work first. Reuse its interface and fixtures. This lane owns multisample coverage, depth and parity through the execution path.

Rules

- Use `git cambium add -b loft/cap1a -name loft-cap1a -ephemeral ~/git/_wt/loft-cap1a origin/main`.
- Predict frame p95, primary/secondary ray counts, attachment bytes and expected edge pixels. Iterate compile errors with `CARGO_BUILD_JOBS=2 tools/dev.sh check CRATE`. Use existing pass/attachment variants and a typed refusal for an incompatible target.
- Own visibility attachment setup, sample-aware classification/resolve in passes and CLI/embedded execution, and the matched coverage fixtures. v15a owns the authored transport callback and its published A/B table. Keep per-scene BRDF policy in the library.
- Lock every GPU job with `tools/gpu_lock.sh`. Use paired same-run 95% intervals, zero concurrent builds and triangle p95 <0.15 ms for speed acceptance. Preserve approved references and main's single-sample scene behavior.
- Run focused checks and the workspace suite once. Write agent_notes/2026-11-20_v1/lanes/cap1a.md. Preserve raw passes, images, sample IDs/depth and metrics under the primary checkout's out/meta/cap1a/. Report evidence if two fixes fail without narrowing the cause.
- Commit by path and push loft/cap1a for integration. End commit messages with `Co-Authored-By: GPT (codex) <noreply@openai.com>`. Preserve ignored evidence before safe Cambium removal.

Task

1. Start with a tile containing sky, a thin limb and two overlapping material rows. Build matched 4× integer ID/payload and depth attachments for a supported format. Read the original samples explicitly and compare coverage and depth against the raster control, including a centre miss and an inside-capsule ray. Verify compiler, loader and actual pipeline sample counts match.
2. Reconstruct each distinct hit once, preserve per-row sample weight and let one owner resolve the HDR pixel. Check mixed-kind edge writes, the analytic capsule normal, shadow and one diffuse bounce, PCG dimensions, sky and film. Keep four samples in the submitted candidate. The one-tap probe is a diagnostic.
3. Show zero full-screen primary BVH rays, validated secondary counts and the same transport through the raster route. Run both matched A/B schedules at count 1/20, 1280 × 800, fixed t=0, 300 frames each, reversed order. Compare pass and frame p50/p95, control distribution, cold/warm load, commands and all coverage traffic. Reproduce ≥47.26/37.04 dB B images at 1/20 groups and localize any structural difference with ID/depth. Target new ≤ legacy p95 for both workloads. Preserve existing shipped pictures and report any remaining delta explicitly.

Final message ≤120 words.
