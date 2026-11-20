Effort: high

You are a Loft engineer on a Sol implementation lane. Repo ~/git/loft. The audit measured main at `1194779`. You have no other context.

Read `docs/spec/README.md` (precedence 07 > PATCH_V1_2 > 06 > 00–05), `docs/PHILOSOPHY.md`, `CORE.md`, `agent_notes/2026-11-20_v1/OPEN.md` and `agent_notes/2026-11-20_v1/design/lean1.md`.

## Rules

- Start after lean1b and the x2a/x2b integration decisions on main. Take the accepted native and motion behavior as the control. Pin the integrated control SHA, then create `git cambium add -b loft/lean1c -name loft-lean1c -ephemeral ~/git/_wt/loft-lean1c origin/main`.
- Own `crates/loft-runner/`, `crates/loft-std/`, `crates/shared/` and `crates/loft-cli/src/native/`. Write your measurements to `agent_notes/2026-11-20_v1/lanes/lean1c.md`. Keep the Phase 5 archive read-only.
- Measure before and after with `scc --by-file --format csv crates/loft-*/src crates/shared` (3.7.0, Code field). Attribute x2's deltas separately. Count embedded tests, WGSL, generated inputs and included source. Preserve independent test assertions and readable formatting.
- Predict the count and parity before implementation. Write the code, then iterate `CARGO_BUILD_JOBS=2 tools/dev.sh check CRATE` until it compiles. Run focused tests through `tools/dev.sh` and GPU work under `tools/gpu_lock.sh`. After two failed full-test/GPU fixes without a narrower cause, escalate with the evidence.
- Dispatch once into a handler that receives its variant. Preserve current diagnostics, exits, bytes, unknowns and caller contracts. Keep all frozen references and thresholds. Require 18/18 actual frame hashes and nine bead hashes identical to the integrated control, with identical Pictures, Twin, Speed, Load and Truth scores and named verdict outcomes. The integrator runs `tools/check.sh full` once per combined revision. Record any inherited failure by name.
- Write before/after counts, deleted producers and consumers, measured scores, commit IDs and evidence paths to your lane note. Preserve ignored evidence under the primary checkout's `out/meta/lean1c/`. Push `loft/lean1c` for the integrator. Leave landing on main to the integrator. Commit messages end with "Co-Authored-By: GPT (codex) <noreply@openai.com>".

## Task

Prepare B's ordered declarations once for both host and native. Audit what x2a already removed before deleting anything. Share pipeline/link/record logic for draws, bakes, mips, ticks and dirty edits. Leave native device acquisition, submit/readback, cache hook, triangle/timestamp probes and reporting intact. Distinguish external-unbound from missing resources. Have `Linker::ticks` pass the selected fused payload to reflection and have `Attachments::canvas` resolve host/internal sources for `views`. Keep live device checks, invalidation, clear-on-first-write, shadow reuse, both depth conventions and a host-owned command encoder.

For CPU stepping, unify binding preparation and checked ranges around `compute_twin::dispatch` in `step_bundle_cpu` and `History::step_live`, keeping their bytes, cursors and checkpoint lifetimes. Keep arbitrary validated compute entry interpretation, failure paths, async promotion and browser host capabilities. Preserve logical buffer ranges and x2a's numeric slot resolution, R16 behavior and cold pipeline-create concurrency. A moved executor is still charged to execution.

## Acceptance

Reach runner **≤1,450** (from 1,553), std including shared **≤2,250** (from 4,107) and native CLI **≤500** (from 2,208), combined **≤4,200**. Report the functional execution count across all destinations in x2a's own metric as well. Run focused runner and std tests under the GPU lock, then `tools/dev.sh check loft-runner --target wasm32-unknown-unknown --features web-sys`. Compare source-native, embedded and decoded bundle RGBA/depth/ID, a renamed slot, the 4,488-byte LUT, supported/refused bake formats, bind/rebind, resize/swap, shadow invalidation, checkpoint seek and stale promotion. Preserve post-x2 command counts, module/pipeline counts and all timing-gate decisions. Report timestamp controls and samples. Keep the existing CLI facade usable by D.

Final message ≤120 words.
