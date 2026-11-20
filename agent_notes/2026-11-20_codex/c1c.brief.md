# c1c · Verify the body of motion helpers marked wgsl det

You are a gpt-6.2-sol engineer. Repo ~/git/loft. Implement the c1c decision in agent_notes/2026-11-20_v1/design/c1.md. Read AGENTS.md, AGENT_CARD.md, CORE.md, docs/PHILOSOPHY.md, docs/spec/README.md (07 > PATCH_V1_2 > 06 > 00–05), and agent_notes/2026-11-20_v1/OPEN.md. The probes and archived raw evidence are in the primary checkout under out/meta/c1/.

From ~/git/loft, run `git fetch origin main` and `git cambium add -b loft/c1c -name loft-c1c -ephemeral ~/git/_wt/loft-c1c origin/main`. Work in that workspace. Record the base SHA. Prior lane heads are evidence only.

**Owned files** — `crates/loft-passes/src/check.rs`, `crates/loft-passes/src/solver.rs`, `crates/loft-passes/src/motion.rs`, `crates/loft-passes/src/lib.rs` (module/verification hook), `crates/loft-passes/src/det_check.rs` if needed, and focused fixtures/tests under `crates/loft-passes/tests/motion_source*`. Also own `agent_notes/2026-11-20_v1/lanes/c1c.md`.

**Task.** Close the raw-body legality part of 06 §10. Current law and solver reachability stop at FnDecl::refuses_unverified_raw. That checks the marker bit. Naga accepts WGSL that Loft prohibits in deterministic motion. Use one verifier from both motion paths to inspect reachable typed raw functions/helpers, including those used by scheduled checks. Restrict the inspection to authored code and its call graph so generated advance loops, outbox reservations and scaffolding retain their documented semantics. Prove fixed loop bounds and permitted effects. Emit E504 with the original author location for an unbounded loop or unsupported effect/math; keep the existing wrapped motion functions for permitted transcendental calls. Preserve strict arithmetic as a separate lowering concern. Use typed Naga IR and existing provenance. The D2a branch is a source of experiments; its raw-loop blanket rejection and unrelated numerical rewrite exceed this lane.

**Score.** Reproduce the twelve CPU-only `loft show` observations in det.log (sources and script in out/meta/c1). An ordinary Loft identity, marked raw identity and a literal three-trip for-loop must compile on each route. Unmarked raw identity, marked data-dependent while, and marked pow(x,x) must each exit 1 with E504 on both routes. Today the while and pow cases all exit 0. Include a transitive helper, a valid render-only raw helper, a declared fixed bound, and a raw write outside its permitted state as controls. Valid finite loops must execute the predicted result. Preserve the positive shipped scenes, diagnostic spans and all prior negative cases. Record which guarantees are verified. Full CPU/Metal rounding parity and authored BODY/POSE execution remain tracked under D2.

**Execution and handoff**

- Write the prediction, code and focused checks first. Use two jobs. Repair compiler diagnostics with `CARGO_BUILD_JOBS=2 tools/dev.sh check loft-passes --all-targets` until clean. Then build once with `CARGO_BUILD_JOBS=2 tools/dev.sh build loft-passes --fast --tests --bins --examples --message-format=json-render-diagnostics`. Capture Cargo’s artifact JSON and run the emitted executables from their crate directories. Allow at most two test/GPU reruns. After a second failed fix, ESCALATE in the lane note with the SHA, command, input, prediction, actual output and smallest discriminating observation.
- Classify inputs at the boundary, dispatch variants exhaustively, and preserve typed failures. Keep refs, manifests, tolerances and historical inputs frozen. Never re-approve a reference or re-pin any fixture or dependency. Keep examples/ read-only.
- Serialize every GPU run under `tools/gpu_lock.sh`. Run focused checks here. The integrator owns the combined full suite and gate. Save commands, exits, adapter, overrides, counts, timing controls, hashes and lossless comparison evidence under ~/git/loft/out/meta/c1c/. Put measured before/after, remaining misses and evidence paths in the lane note. Mark unsupported or unmeasured cases explicitly.
- Commit only owned paths, push `loft/c1c`, and leave main to the integrator. Once evidence and branch are preserved, remove your lane checkout from the primary repo with `git cambium remove --safe loft-c1c`. End each commit message with the following trailer.

Co-Authored-By: GPT (codex) <noreply@openai.com>

Final message ≤120 words.
