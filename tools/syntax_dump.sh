#!/bin/bash
# Build the Loft v1.0 syntax handoff: the ask, the bakeoff cards, tasks, verdicts and the Sonnet outputs, with headers.
set -euo pipefail
cd "$(dirname "$0")/.."
H=docs/handoff/2026-11-20_syntax; B=agent_notes/2026-11-20_syntax_bakeoff
sed -n '/^## 1\. Decisions/,/^## 3\. The core/p' docs/handoff/2026-11-20_spec/astra_spec_v1.md | sed '$d' > "$H/_astra_spec_s1_s2.md"
sec() { [ -f "$2" ] || return 0; printf '\n\n================================================================\n# %s\n# file: %s\n# what it is: %s\n================================================================\n\n' "$1" "$2" "$3"; cat "$2"; }
{
  sec "THE ASK" "$H/00_SYNTAX_ASK.md" "the question, what we measured, our reflection, the ten questions, what to send back"
  sec "TASKS" "$B/TASKS.md" "the three bakeoff tasks, in plain words"
  for s in A B C D E F G; do sec "CARD $s" "$B/card_$s.md" "the one-page language card for candidate $s"; done
  sec "VERDICT ROUND 1 (A-D)" "$B/VERDICT.md" "the blind judge's scores for A to D"
  sec "VERDICT ALL (A-E)" "$B/VERDICT_ALL.md" "the judge's scores including the terse candidate E (if finished)"
  sec "VERDICT SEVEN (A-G)" "$B/VERDICT_SEVEN.md" "the judge's scores for all seven (if finished)"
  for s in A B C D E F G; do sec "SONNET OUTPUT $s" "$B/out_${s}_sonnet.md" "the three tasks written by Sonnet in syntax $s"; done
  for s in A D E F G; do sec "HAIKU OUTPUT $s" "$B/out_${s}_haiku.md" "the three tasks written by Haiku in syntax $s"; done
  # Background inlined: the reader may have no access to the repo.
  sec "BACKGROUND: PHILOSOPHY" "docs/PHILOSOPHY.md" "the stance: meta engine; hard code is cheap, lock-in is expensive"
  sec "BACKGROUND: META ENGINE PLAN" "docs/proposals/2026-11-20_meta_engine_plan.md" "the plan, invariants and gates"
  sec "BACKGROUND: LANGUAGE DRAFT" "agent_notes/dump_manual_nov_19th_alpha_v1.md" "the chief scientist's draft (declaration style, syntax A's origin)"
  sec "BACKGROUND: ASTRA SPEC v1, sections 1-2" "$H/_astra_spec_s1_s2.md" "Astra's decisions and language section (builder style, syntax B's origin)"
  sec "BACKGROUND: PHASE 1 NOTE" "agent_notes/2026-11-20_meta/phase1.md" "what is built in syntax A, measured numbers, and where it differs from Astra"
  for f in engines/*.loft; do sec "BACKGROUND: BUILT SCENE $(basename "$f")" "$f" "a Phase 1 scene as built, in syntax A"; done
} > "$H/DUMP.md"
pbcopy < "$H/DUMP.md"
echo "copied $(wc -w < "$H/DUMP.md") words from $H/DUMP.md"
