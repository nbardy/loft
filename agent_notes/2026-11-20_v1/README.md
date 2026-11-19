# Loft v1.0 build (started 20 Nov 2026)

The working folder for building Loft v1.0. The spec is `docs/spec/` (the source of truth; `06_ADDENDUM.md`
overrides; `README.md` holds the lead's rulings). The scientist's two raw dumps are here verbatim.

## Who does what
- **gpt-6.2-sol (engineer):** implements each phase and applies review fixes, merging to main when gates pass.
- **gpt-6.2-astra (reviewer):** after each phase, reviews correctness, direction and code quality; writes
  `reviews/<phase>.md` (MUST-FIX / SHOULD-FIX / NOTE). Astra also implements Phase 1.5 (the parser), the hardest slice.
- **Claude:** set-up and summaries only.

## Pipeline (`agent_notes/2026-11-20_codex/chain2.sh`, resumable)
Phase 1.5 (Astra) → review → fix → Phase 2 → review → fix → Phase 3 → … → Phase 5. A phase that cannot pass its gates
stops the chain (`chain.stopped`). Restart after a reboot: `nohup agent_notes/2026-11-20_codex/chain2.sh &`.

## Where things are
| What | Where |
|---|---|
| Spec | `docs/spec/00..06`, `docs/spec/README.md` |
| Scientist dumps (raw) | `dump_nov_19th_scientist_details.md`, `dump_nov_19th_scientist_more_finishing_details.md` (here) |
| Briefs, logs, final messages | `agent_notes/2026-11-20_codex/` (`p*` implement, `r*` review, `f*` fix) |
| Phase notes (predictions, scores) | `agent_notes/2026-11-20_meta/<phase>.md` |
| Reviews | `reviews/` (here) |
| Retired engine and Phase 5 inputs | [DEPRECATED.md](../../DEPRECATED.md) |

## Update: fan-out pipeline (`chain3.sh`, replaces chain2)
Per the scientist's fan-out handoff (`fanout_handoff.md` here): Phase 1.5 → review → fix → **legacy engine cleanup** → **Step 0 artifacts** (golden
vectors crate `loft-abi`, `docs/spec/QUIRKS.md`, contention canary, gate mutants, `AGENT_CARD.md`, CARGO_BUILD_JOBS=2,
GPU lock) → for Phases 2+3, 4 and 5: **freeze** shared types (Astra) → **parallel Sol lanes** (CPU twin + naga only,
≤3 Rust-heavy at once, each pushes its own branch, ESCALATE after two failed attempts) → **Astra audit + integrate**
(fixes MUST-FIX, holds `/tmp/loft-gpu.lock`, runs ONE full GPU gate, writes `out/meta/<id>/gate.json`, merges).
Lane notes: `lanes/`. Restart: `nohup agent_notes/2026-11-20_codex/chain3.sh &`.
