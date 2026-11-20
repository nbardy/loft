Effort: high

You are a Loft engineer on a Sol implementation lane. Repo ~/git/loft. The audit measured main at `1194779`. You have no other context.

Read `docs/spec/README.md` (precedence 07 > PATCH_V1_2 > 06 > 00–05), `docs/PHILOSOPHY.md`, `CORE.md`, `agent_notes/2026-11-20_v1/OPEN.md` and `agent_notes/2026-11-20_v1/design/lean1.md`.

## Rules

- Start after x2b integration or its recorded hold on main. Coordinate the queued x3 ownership of check/ad/eval with its lead. Pin the integrated control SHA, then create `git cambium add -b loft/lean1a -name loft-lean1a -ephemeral ~/git/_wt/loft-lean1a origin/main`.
- Own `crates/loft-parse/` and exactly `ad.rs, check.rs, eval.rs, infer.rs, kernel_units.rs, lip.rs, refine.rs, motion_source.rs, plan_source.rs` in `crates/loft-passes/src/`. Write your measurements to `agent_notes/2026-11-20_v1/lanes/lean1a.md`. Keep the Phase 5 archive read-only.
- Measure before and after with `scc --by-file --format csv crates/loft-*/src crates/shared` (3.7.0, Code field). Attribute x2's deltas separately. Count embedded tests, WGSL, generated inputs and included source. Preserve independent test assertions and readable formatting.
- Predict the count and parity before implementation. Write the code, then iterate `CARGO_BUILD_JOBS=2 tools/dev.sh check CRATE` until it compiles. Run focused tests through `tools/dev.sh` and GPU work under `tools/gpu_lock.sh`. After two failed full-test/GPU fixes without a narrower cause, escalate with the evidence.
- Dispatch once into a handler that receives its variant. Preserve current diagnostics, exits, bytes, unknowns and caller contracts. Keep all frozen references and thresholds. Require 18/18 actual frame hashes and nine bead hashes identical to the integrated control, with identical Pictures, Twin, Speed, Load and Truth scores and named verdict outcomes. The integrator runs `tools/check.sh full` once per combined revision. Record any inherited failure by name.
- Write before/after counts, deleted producers and consumers, measured scores, commit IDs and evidence paths to your lane note. Preserve ignored evidence under the primary checkout's `out/meta/lean1a/`. Push `loft/lean1a` for the integrator. Leave landing on main to the integrator. Commit messages end with "Co-Authored-By: GPT (codex) <noreply@openai.com>".

## Task

Give each checked call a resolved operation, typed operands and source span indexed by the existing expression ID. Keep the lossless CST and pre-check syntax used by AD. Reuse `WorldQuery` and `Eval<S>` where they already express the contract. Feed checker, concrete evaluator, interval evaluator and derivative rules from the same operation classification, preserving their distinct arithmetic and refusal rules. Expose the decisions that B needs in an owned module. Keep the old caller signatures working during adoption.

Start with two concrete traces. Follow a scoped helper calling a builtin through `Parser::kind`, `Ck::call` and `Eval::expr_inner`. Then follow `motion_source::solver` into `scheduled` and `plan_source::check_source` into `lower_source`. Remove the repeated solver config and objective decoding. In the parser, replace the repeated kind-body scan and duplicated row/list/unit parsing with one cursor over CST spans and a scoped resolution pass. Preserve virtual newline insertion, forward kind helpers, diagnostics and source order. Leave the final rounding sequence and x2b verification intact.

## Acceptance

Reach parser **≤1,650** (from 2,581) and the nine passes files **≤2,780** (from 5,087), total **≤4,430**. Run `tools/check.sh static`, `tools/dev.sh test loft-parse` and focused `loft-passes` diagnostics, phase4, motion and Tier 0 tests. Compare both grammar recognizers, lossless source replay, E-codes, spans, notes and fixes to the control. Include a shadowed builtin, kind helper declared after use, unit power/reciprocal, opted-out derivative, verified raw solver and its E504 negative control. Handoff to B names the exact operation API and the removed reclassifiers.

Final message ≤120 words.
