#!/bin/bash
# Build the chief-scientist handoff: every doc with a header saying what it is, then the key instructions; copy it.
#   tools/handoff_dump.sh            writes docs/handoff/2026-11-20_spec/DUMP.md and pbcopies it
set -euo pipefail
cd "$(dirname "$0")/.."
H=docs/handoff/2026-11-20_spec
bun src/cli/main.ts context > "$H/_loft_context.md" 2>/dev/null || true
sec() { printf '\n\n================================================================\n# %s\n# file: %s\n# what it is: %s\n================================================================\n\n' "$1" "$2" "$3"; cat "$2"; }
{
  sec "PROMPT" "$H/00_PROMPT.md" "the ask: what we need you to write"
  sec "STATE AND LESSONS" "$H/01_STATE_AND_LESSONS.md" "where Loft stands, every number that decides a design, what went well, where we turned poorly"
  sec "PATTERNS TODAY" "$H/02_PATTERNS_TODAY.md" "what an author could do in old Loft: geometry, folding, PBR, stylised looks, escape hatches, motion"
  sec "PHILOSOPHY" "docs/PHILOSOPHY.md" "the stance: meta engine; hard code is cheap, lock-in is expensive"
  sec "CORE" "CORE.md" "priorities and definition of done"
  sec "META ENGINE PLAN" "docs/proposals/2026-11-20_meta_engine_plan.md" "our plan from first principles, including our decisions on your draft (section 3)"
  sec "LANGUAGE DRAFT (yours)" "agent_notes/dump_manual_nov_19th_alpha_v1.md" "your previous answer: grammar, typing, staging, tiers, the hermit crab, lowering"
  sec "RENDER ARCHITECTURE" "docs/proposals/2026-11-19_render_architecture.md" "our consolidated render design (evidence, not instructions): kinds, six queries, schedules"
  sec "MOTION ARCHITECTURE" "docs/proposals/2026-11-19_motion_architecture.md" "our consolidated motion design (evidence, not instructions): body/state/pose, ends, pins, laws, fused ticks"
  sec "MOTION HANDOFF" "docs/handoff/2026-11-19_motion_chief_scientist.md" "the motion goal, measured look rounds, and the nine questions we most want answered"
  sec "FOLDING AND TRACING" "docs/folding_and_tracing.md" "old design note on domain maps, folds and tracers"
  sec "REFERENCES" "$H/03_REFERENCES.md" "papers, systems and APIs worth reading, grouped by design question (cited from memory)"
  sec "APPENDIX: LOFT CONTEXT" "$H/_loft_context.md" "the old model-facing API summary (what models were taught)"
  sec "APPENDIX: SIGNATURES" "$H/_signatures.md" "every exported author-facing signature of the old library (coverage record)"
  sec "!!IMPORTANT!! KEY INSTRUCTIONS" "$H/KEY_INSTRUCTIONS.md" "how to treat our prior conversations and exactly what to send back"
} > "$H/DUMP.md"
pbcopy < "$H/DUMP.md"
echo "copied $(wc -w < "$H/DUMP.md") words, $(wc -c < "$H/DUMP.md") bytes from $H/DUMP.md"
