Effort: high

You are a Loft engineer on a Sol implementation lane. Repo ~/git/loft. The audit measured main at `1194779`. You have no other context.

Read `docs/spec/README.md` (precedence 07 > PATCH_V1_2 > 06 > 00–05), `docs/PHILOSOPHY.md`, `CORE.md`, `agent_notes/2026-11-20_v1/OPEN.md` and `agent_notes/2026-11-20_v1/design/lean1.md`.

## Rules

- Start after lean1a and the x2a/x2b integration decisions. Observe x2c and queued x3 ownership on overlapping compiler/shading files. Pin the integrated control SHA, then create `git cambium add -b loft/lean1b -name loft-lean1b -ephemeral ~/git/_wt/loft-lean1b origin/main`.
- Own `crates/loft-plan/` and `crates/loft-passes/` except A's `src/{ad,check,eval,infer,kernel_units,lip,refine,motion_source,plan_source}.rs`. Write your measurements to `agent_notes/2026-11-20_v1/lanes/lean1b.md`. Keep the Phase 5 archive read-only.
- Measure before and after with `scc --by-file --format csv crates/loft-*/src crates/shared` (3.7.0, Code field). Attribute x2's deltas separately. Count embedded tests, WGSL, generated inputs and included source. Preserve independent test assertions and readable formatting.
- Predict the count and parity before implementation. Write the code, then iterate `CARGO_BUILD_JOBS=2 tools/dev.sh check CRATE` until it compiles. Run focused tests through `tools/dev.sh` and GPU work under `tools/gpu_lock.sh`. After two failed full-test/GPU fixes without a narrower cause, escalate with the evidence.
- Dispatch once into a handler that receives its variant. Preserve current diagnostics, exits, bytes, unknowns and caller contracts. Keep all frozen references and thresholds. Require 18/18 actual frame hashes and nine bead hashes identical to the integrated control, with identical Pictures, Twin, Speed, Load and Truth scores and named verdict outcomes. The integrator runs `tools/check.sh full` once per combined revision. Record any inherited failure by name.
- Write before/after counts, deleted producers and consumers, measured scores, commit IDs and evidence paths to your lane note. Preserve ignored evidence under the primary checkout's `out/meta/lean1b/`. Push `loft/lean1b` for the integrator. Leave landing on main to the integrator. Commit messages end with "Co-Authored-By: GPT (codex) <noreply@openai.com>".

## Task

Make compiler lowering the single producer of ordered executable `PassDecl`s. Keep `Program`'s public/sexp and `Compiled/Report` inspection contracts, including field names, source locations, NaN payloads and module/row order. Build the executable declarations from checked inputs. Delete source-native vs archive schedule reconstruction wherever x2a has left two writers. Consume A's operation payloads in `Emitter::expression` and `tier0_author::Lower::expr`. Carry validated row slots into `shader_queries`. Preserve the 56-op tape, its residual specialization, 32 registers, branch behavior, mutation generations and shader entry points.

Give the draw builder one resolved coverage payload. Emit query definitions with their chosen namespace and row access so `paths.rs::{namespace,row_arena_source}` cease interpreting generated lines. Preserve the emitted math, loop termination, derivative rules and independent CPU oracle. Compile proof of invariant lets into typed bindings for all three readers in `emit_prepared`. Pack a `FieldMedium` row once, preserving the existing sequence of M×(1+P) rows and the order of errors. Keep the optical boundary stack separate. In `loft-plan` make ground/flight phase selection explicit. Preserve 24 knots, 96 collocation points, launch boundary conventions, arithmetic order, failed headers and packed prior bytes.

## Acceptance

Reach **≤5,220** across B's passes files (from 10,460) and **≤800** in plan (from 859), combined **≤6,020**. A+B passes must be **≤8,000**. Reconcile composition 1,150, draw/lighting/world 1,970, tape 1,540 and motion/solver/priors 560 against the ledger in the audit. Compare native/archived declarations, byte hashes and `ir --sexp` round trips, two medium rows plus two lamps (six packed rows), and prior samples exactly at launch/landing. Run `tools/dev.sh test loft-plan`, focused passes diagnostics, furnace, phase4/5, tier0 and motion tests, plus the integrated x2b 481-tick control in all three scenes. Record shader/pipeline identity and order where persisted artifacts depend on them.

Final message ≤120 words.
