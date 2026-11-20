#!/bin/bash
# Build the chief-scientist review packet: every file with a header saying what it is; copies it to the clipboard.
#   tools/review_dump.sh     writes docs/handoff/2026-11-20_review/DUMP.md and pbcopies it
set -euo pipefail
cd "$(dirname "$0")/.."
H=docs/handoff/2026-11-20_review
sec() { [ -f "$2" ] || return 0; printf '\n\n================================================================\n# %s\n# file: %s\n# what it is: %s\n================================================================\n\n' "$1" "$2" "$3"; cat "$2"; }
{
  sec "PROMPT" "$H/00_PROMPT.md" "the review ask: where we stand and what we need back"
  sec "OPEN TARGETS" "agent_notes/2026-11-20_v1/OPEN.md" "every open target with measured value, target and owner"
  sec "DESIGN D1" "agent_notes/2026-11-20_v1/design/d1.md" "Astra's first failure-class design (runner size, frame time, Tier 0, migration)"
  sec "DESIGN D2" "agent_notes/2026-11-20_v1/design/d2.md" "Astra's proved causes: FMA twin divergence, Tier 0 tape cost, execution path"
  sec "CONSUMER PASSES (lead)" "docs/proposals/2026-11-20_consumer_passes.md" "the owner's question and the lead's first answer"
  sec "CONSUMER PASSES (Astra)" "docs/proposals/2026-11-20_consumer_passes_astra.md" "Astra's design: Wasm build-time passes, behaviour {} example"
  sec "ROOM RUNNERS STUDY" "agent_notes/2026-11-20_v1/room_runner/STUDY.md" "devex study and integration plan for the first embedding host"
  sec "PHILOSOPHY" "docs/PHILOSOPHY.md" "the stance: meta engine; hard code is cheap, lock-in is expensive"
  sec "CORE" "CORE.md" "priorities and definition of done (rewritten for the new workspace)"
  sec "AGENT CARD" "AGENT_CARD.md" "the one-page card models write scenes from"
  sec "SPEC README" "docs/spec/README.md" "precedence (07 > PATCH_V1_2 > 06 > 00-05) and lead rulings"
  for f in docs/spec/0*.md docs/spec/PATCH_V1_2.md docs/spec/QUIRKS.md; do sec "SPEC $(basename "$f" .md)" "$f" "your spec as we filed it (normative)"; done
  sec "GRAMMAR" "docs/spec/loft.lark" "the normative grammar"
  sec "CODE SAMPLE: hermit engine" "engines/hermit.loft" "the hermit crab in E+ (3,021 tokens)"
  sec "CODE SAMPLE: crab motion" "scenes/crab.loft" "a motion scene"
  sec "CODE SAMPLE: motion library" "lib/motion.loft" "the motion std library"
  sec "CODE SAMPLE: reference integrator" "engines/reference.loft" "the reference path tracer as an example engine"
  printf '\n\n================================================================\n# REPO LAYOUT\n================================================================\n\n'
  git ls-files | cut -d/ -f1-2 | sort | uniq -c | sort -rn | head -40
  printf '\nRust lines per crate (src):\n'; for c in crates/*; do printf '%s %s\n' "$(basename "$c")" "$(git ls-files "$c/src" | grep '\.rs$' | xargs cat 2>/dev/null | wc -l)"; done
} > "$H/DUMP.md"
pbcopy < "$H/DUMP.md"
echo "copied $(wc -w < "$H/DUMP.md") words, $(wc -c < "$H/DUMP.md") bytes from $H/DUMP.md"
