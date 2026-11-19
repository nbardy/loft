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
| Deprecated trees | `DEPRECATED.md` |
