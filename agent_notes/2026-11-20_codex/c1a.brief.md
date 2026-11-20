# c1a · Remove the Hermit surface-bake pipeline

You are a gpt-6.2-sol engineer. Repo ~/git/loft. Implement the c1a decision in agent_notes/2026-11-20_v1/design/c1.md. Read AGENTS.md, AGENT_CARD.md, CORE.md, docs/PHILOSOPHY.md, docs/spec/README.md (07 > PATCH_V1_2 > 06 > 00–05), and agent_notes/2026-11-20_v1/OPEN.md. The probes and archived raw evidence are in the primary checkout under out/meta/c1/.

From ~/git/loft, run `git fetch origin main` and `git cambium add -b loft/c1a -name loft-c1a -ephemeral ~/git/_wt/loft-c1a origin/main`. Work in that workspace. Record the base SHA. Prior lane heads are evidence only.

**Owned files** — `crates/loft-passes/src/emit/surface.rs`, `crates/loft-passes/src/emit.rs` (indexed surface emission), `crates/loft-cli/src/native/mod.rs` (grid resources and draws), and `crates/loft-cli/examples/phase4_gate.rs` (Hermit comparison). Also own `agent_notes/2026-11-20_v1/lanes/c1a.md`.

**Task.** The gate counts 14 real pipelines for both Hermit cases. The extra pipeline fills 6,369 object-space points once. C1's matched indexed-vertex experiment read no points and reproduced six main frames. Implement index-only evaluation on the current grid-eligible surface path. Generate UV from the integer vertex index with the original divisions, evaluate the authored surface, and use the actual row for the pose. Preserve the eligibility proof, both seam endpoints, the 36,864 index order and the camera and shadow indirect draws. Remove dead layouts, binds, compute creation and submission. Keep actual create telemetry. Carry older emitted modules through a compatible entry with its declared resources. Retain the existing mesh, row-dependent surface, procedural and AOV behavior. No scene-name switches.

**Score.** Use the ordinary `loft load`, `profile` and still paths on engines/hermit.loft with `--set count=1` and `20`. The prediction is 14→13 actual creates, zero grid compute dispatches at load, 8 modules, 17 commands, cold ≤1.5 s and warm ≤0.060 s at both counts. Use distinct cold nonces. At 1280×800, 300 timed orbit frames after warmup, caps are 5.0/5.6 ms. Record triangle control and per-pass p50/p95 against matched main. Compare lossless beauty, depth and ID at 0, 4 and 9 s for both counts, zero differing pixels. Include a non-indexed surface control, an archived Hermit program and the existing overdraw path. Report net production lines and removed GPU allocations. The six beauty observations in grid.log are evidence for the design. The AOVs and removal of the create itself are delivery checks.

**Execution and handoff**

- Write the prediction, code and focused checks first. Use two jobs. Repair compiler diagnostics with `CARGO_BUILD_JOBS=2 tools/dev.sh check loft-cli -p loft-passes --all-targets` until clean. Then build once with `CARGO_BUILD_JOBS=2 tools/dev.sh build loft-cli -p loft-passes --fast --tests --bins --examples --message-format=json-render-diagnostics`. Capture Cargo’s artifact JSON and run the emitted executables from their crate directories. Allow at most two test/GPU reruns. After a second failed fix, ESCALATE in the lane note with the SHA, command, input, prediction, actual output and smallest discriminating observation.
- Classify inputs at the boundary, dispatch variants exhaustively, and preserve typed failures. Keep refs, manifests, tolerances and historical inputs frozen. Never re-approve a reference or re-pin any fixture or dependency. Keep examples/ read-only.
- Serialize every GPU run under `tools/gpu_lock.sh`. Run focused checks here. The integrator owns the combined full suite and gate. Save commands, exits, adapter, overrides, counts, timing controls, hashes and lossless comparison evidence under ~/git/loft/out/meta/c1a/. Put measured before/after, remaining misses and evidence paths in the lane note. Mark unsupported or unmeasured cases explicitly.
- Commit only owned paths, push `loft/c1a`, and leave main to the integrator. Once evidence and branch are preserved, remove your lane checkout from the primary repo with `git cambium remove --safe loft-c1a`. End each commit message with the following trailer.

Co-Authored-By: GPT (codex) <noreply@openai.com>

Final message ≤120 words.
