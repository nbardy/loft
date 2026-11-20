Effort: high

You are a Loft engineer on a Sol implementation lane. Repo ~/git/loft. The audit measured main at `1194779`. You have no other context.

Read `docs/spec/README.md` (precedence 07 > PATCH_V1_2 > 06 > 00–05), `docs/PHILOSOPHY.md`, `CORE.md`, `agent_notes/2026-11-20_v1/OPEN.md` and `agent_notes/2026-11-20_v1/design/lean1.md`.

## Rules

- Start on combined main after lean1a, lean1b, lean1c and their x2 prerequisites. Coordinate overlapping CLI tests with the queued x3 lead. The gate lands with the passing combined tree. Pin the integrated control SHA, then create `git cambium add -b loft/lean1d -name loft-lean1d -ephemeral ~/git/_wt/loft-lean1d origin/main`.
- Own `crates/loft-abi/`, `crates/loft-ir/`, `crates/loft-format/`, `crates/loft-cli/` except `src/native/`, `tools/check.sh`, an optional `tools/check_lines.py` helper and root `Cargo.lock` for dependencies of your crates. Write your measurements to `agent_notes/2026-11-20_v1/lanes/lean1d.md`. Keep the Phase 5 archive read-only.
- Measure before and after with `scc --by-file --format csv crates/loft-*/src crates/shared` (3.7.0, Code field). Attribute x2's deltas separately. Count embedded tests, WGSL, generated inputs and included source. Preserve independent test assertions and readable formatting.
- Predict the count and parity before implementation. Write the code, then iterate `CARGO_BUILD_JOBS=2 tools/dev.sh check CRATE` until it compiles. Run focused tests through `tools/dev.sh` and GPU work under `tools/gpu_lock.sh`. After two failed full-test/GPU fixes without a narrower cause, escalate with the evidence.
- Dispatch once into a handler that receives its variant. Preserve current diagnostics, exits, bytes, unknowns and caller contracts. Keep all frozen references and thresholds. Require 18/18 actual frame hashes and nine bead hashes identical to the integrated control, with identical Pictures, Twin, Speed, Load and Truth scores and named verdict outcomes. The integrator runs `tools/check.sh full` once per combined revision. Record any inherited failure by name.
- Write before/after counts, deleted producers and consumers, measured scores, commit IDs and evidence paths to your lane note. Preserve ignored evidence under the primary checkout's `out/meta/lean1d/`. Push `loft/lean1d` for the integrator. Leave landing on main to the integrator. Commit messages end with "Co-Authored-By: GPT (codex) <noreply@openai.com>".

## Task

Parse commands and replay modes at the CLI boundary. Replace the remote JSON copies of `LoftLog/LoggedInput/Input` with serde on the format-owned fields. Retain JSON shape, unknown-field refusal, wire tags and both 3/4-section bundle readers. Share the validated replay tick/update/first-failure driver for historical expression-twin and native signatures. Preserve both backend identities, mismatched-device refusals, intermediate failing ticks, binary and JSON logs and existing behavior for unsupported inputs. Use one verdict mapping per ingress (including missing/nonfinite/skipped), then `loft_ir::Verdict`. Keep `Failure::exit` as the sole command exit mapping with the existing messages and statuses. Share scene loading and option parsing across build, render, still, clip and profile without changing defaults or observable order.

Implement `tools/check.sh lines` and call its function in `static_checks`. Follow the exact scc 3.7.0 inventory, file inclusions, charges and error conditions in lean1.md. Report every crate and total even on failure. Make over-budget, absent source, absent scc or unmapped member fail. Charge native CLI to CLI and shared visibility to std. Verify with the audited tree and a temporary per-crate overage under the aggregate cap.

## Acceptance

Reach non-native CLI **≤1,700** (from 2,987), format **≤700** (740), IR **≤480** (456), ABI **≤450** (446), combined **≤3,330**. The combined gate ceilings are passes 8,000, CLI 2,200, std 2,250, parse 1,650, runner 1,450, plan 800, format 700, IR 480, ABI 450 and **17,980 total**. At `1194779` the counter must report 31,484 and seven offending crates. Run ABI goldens, format corruption/depth tests, CLI exit-status/clip/replay tests and `tools/check.sh lines` on the combined tree. Check frozen archive bytes, replay JSON field order/values, state and bundle identities, one recovered intermediate miss and both historical/native signatures. Save the control and candidate GPU evidence as described in the audit. The integrator's unchanged full score, hashes and functional execution ledger are required for landing.

Final message ≤120 words.
