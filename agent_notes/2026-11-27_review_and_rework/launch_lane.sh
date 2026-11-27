#!/bin/bash
# Launch one rework lane in its own git worktree with gpt-6.2-sol (medium effort).
# Usage: launch_lane.sh <lane_name> [base_ref]
set -euo pipefail
lane=$1
base=${2:-HEAD}
D=$(cd "$(dirname "$0")" && pwd)
REPO=$(cd "$D/../.." && pwd)
WT="$HOME/git/_wt/r27-$lane"
BRANCH="rework/2026-11-27-$lane"
BRIEF="$D/briefs/r27_${lane//-/_}.md"
LOG="$D/logs/r27_${lane//-/_}.log"
FINAL="$D/logs/r27_${lane//-/_}.final.md"
DONE="$D/logs/r27_${lane//-/_}.done"

mkdir -p "$D/logs" "$HOME/git/_wt"
[ -f "$BRIEF" ] || { echo "missing brief: $BRIEF" >&2; exit 1; }

if [ ! -d "$WT" ]; then
  if git -C "$REPO" show-ref --verify --quiet "refs/heads/$BRANCH"; then
    git -C "$REPO" worktree add "$WT" "$BRANCH"
  else
    git -C "$REPO" worktree add -b "$BRANCH" "$WT" "$base"
  fi
fi

export PATH="$HOME/.local/bin:${CARGO_HOME:-$HOME/.cargo}/bin:$PATH"
export RUSTC_WRAPPER=sccache CARGO_INCREMENTAL=0 CARGO_BUILD_JOBS=2

COMPILE_RULE="## MANDATORY COMPILE DISCIPLINE (STRICT)
Compilation shares a lock on this 24 GB Mac.
1. Read ALL owned files and relevant interfaces upfront before editing.
2. Write ALL code across all files in one coherent pass BEFORE your first compile.
3. Target 2-3 compiles total: one \`CARGO_BUILD_JOBS=2 tools/dev.sh check <crate>\`, then focused \`tools/dev.sh test <crate>\`.
4. Commit cleanly on branch \`$BRANCH\` by explicit path. Do NOT push or merge to main.
---

"

PROMPT="${COMPILE_RULE}$(cat "$BRIEF")"

taskpolicy -b nice -n 15 perl -e 'alarm shift; exec @ARGV' 5400 \
  codex exec -m gpt-6.2-sol \
  --dangerously-bypass-approvals-and-sandbox \
  -C "$WT" \
  -c model_reasoning_effort=medium \
  -o "$FINAL" \
  "$PROMPT" < /dev/null > "$LOG" 2>&1
rc=$?
echo "$rc" > "$DONE"
exit "$rc"
