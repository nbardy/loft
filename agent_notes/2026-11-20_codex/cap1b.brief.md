Effort: xhigh

You are a Sol engineer on Loft's particle physics lane. Repo ~/git/loft, from origin/main containing the capability-gaps proposal. You have no other context.

Read CORE.md, docs/PHILOSOPHY.md, docs/spec/README.md (including the 20 November owner ruling), docs/spec/PATCH_V1_2.md, docs/V1_5_GOALS.md G2, agent_notes/2026-11-20_v1/OPEN.md and docs/proposals/2026-11-20_capability_gaps.md §1. v15b owns the generic kernel and effect checker. Reuse its landed contract or exercise the same pass ABI from a bundle while it is pending. Record any source/runner dependency separately.

Rules

- Work in a Cambium checkout with `git cambium add -b loft/cap1b -name loft-cap1b -ephemeral ~/git/_wt/loft-cap1b origin/main`.
- Predict the numerical result, cost and readback before implementation. Iterate compile errors with `CARGO_BUILD_JOBS=2 tools/dev.sh check CRATE`. Use exhaustive variants and explicit unsupported/overflow outcomes.
- Run one GPU job at a time under `tools/gpu_lock.sh`. For speed acceptance use paired runs, no concurrent build, triangle p95 below 0.15 ms and a 95% interval. Keep correctness observations from noisy runs. Preserve the frozen references.
- Own the particle transfer library/scene, its grid/twin fixtures and pass-local capability selection for scatter. Coordinate any shared link change with v15b. Keep numerical policy out of the generic runner.
- Run focused checks and the workspace suite once. Preserve inputs, shader hashes, outputs and raw measurements under the primary checkout's out/meta/cap1b/. Write agent_notes/2026-11-20_v1/lanes/cap1b.md. If two attempts fail without narrowing the cause, report the failing fixture and evidence.
- Commit by path, push loft/cap1b and leave integration to main's integrator. End commit messages with `Co-Authored-By: GPT (codex) <noreply@openai.com>`. Preserve ignored evidence before safe Cambium removal.

Task

1. Deliver an authored P2G → grid update → G2P transfer in a small FLIP splash or MPM scene. Bind the trilinear 65,536-particle control through the same source/bundle path. Add an actual 27-tap transfer and report stencil and solver costs separately. Target ≤16 ms frame p95 at 1280 × 800 for the showcase.
2. Choose fixed i32/u32 scales from range and precision, prove or count K, and test capacity+1, signed cancellation, cell boundaries and permutation of particle IDs. Compare every word to a wide integer CPU sum at the transfer boundary. An overflow must leave the previously published grid intact.
3. Enable native f32 atomicAdd only on a granted device. Propagate NonDet to an opted-in consumer and show E504 with the producer span. The same scatter feeding an ordinary scene runs. For 32 seeds in spread/clumped layouts require finite results, normalized global mass/momentum errors ≤10⁻⁵ and worst cell error ≤10⁻⁴ on the proposal's one-step fixture. Track velocity/energy drift over 120 steps separately.
4. Force sorted gather on browser WebGPU and score the same grid under its declared arithmetic contract. Log chosen route, scratch bytes, pipeline count, CPU/GPU costs, particles/s and node contributions/s. Compare with the proposal's 0.46–0.48 ms native medians. Report clear, sort and full step separately.

Final message ≤120 words.
