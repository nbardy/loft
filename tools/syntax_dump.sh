#!/bin/bash
# Build the Loft v1.0 syntax handoff: the ask, the bakeoff cards, tasks, verdicts and the Sonnet outputs, with headers.
set -euo pipefail
cd "$(dirname "$0")/.."
H=docs/handoff/2026-11-20_syntax; B=agent_notes/2026-11-20_syntax_bakeoff
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
  printf '\n\n================================================================\n# BACKGROUND (read in the repo, not inlined)\n================================================================\n\n- docs/PHILOSOPHY.md, CORE.md, docs/proposals/2026-11-20_meta_engine_plan.md\n- agent_notes/dump_manual_nov_19th_alpha_v1.md (the language draft)\n- docs/handoff/2026-11-20_spec/astra_spec_v1.md (Astra'"'"'s spec v1)\n- agent_notes/2026-11-20_meta/phase1.md (what is built, and where it differs from Astra)\n- engines/*.loft, lib/*.loft (the built Phase 1 scenes in syntax A)\n'
} > "$H/DUMP.md"
pbcopy < "$H/DUMP.md"
echo "copied $(wc -w < "$H/DUMP.md") words from $H/DUMP.md"
