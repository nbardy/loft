#!/bin/bash
# Launch one CODE-ONLY (LOFT_NO_COMPILE=1) refactor lane in its own git worktree with gpt-6.2-sol.
# Usage: launch_nocargo.sh <brief_stem> <branch_suffix> [base_ref] [effort]
set -euo pipefail
brief_stem=$1
suffix=$2
base=${3:-HEAD}
effort=${4:-high}
D=$(cd "$(dirname "$0")" && pwd)
REPO=$(cd "$D/../.." && pwd)
WT="$HOME/git/_wt/r27-$suffix"
BRANCH="rework/2026-11-27-$suffix"
BRIEF="$D/briefs/${brief_stem}.md"
LOG="$D/logs/${brief_stem}.log"
FINAL="$D/logs/${brief_stem}.final.md"
DONE="$D/logs/${brief_stem}.done"

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

NO_COMPILE_RULE="## MANDATORY ZERO-COMPILE CODE-ONLY REFACTOR MODE (STRICT!)
**DO NOT RUN \`cargo\`, \`rustc\`, \`tools/dev.sh\`, or \`tools/check.sh\` AT ALL IN THIS LANE.**
We are doing a large, fast, parallel code-only refactor; integration compiles ONCE at the end.
1. Read ALL relevant files, structs, traits, imports, visibility, and \`Cargo.toml\` files upfront using \`rg\` and file reads.
2. Write 100% complete, carefully type-checked Rust / .loft code in one decisive pass so it compiles cleanly on the first try at integration.
3. You MAY run instant read-only checks (\`git diff --check\`, \`python3 tools/check_loft_grammar.py <file>\`), NEVER \`cargo\`/\`rustc\`/\`tools/dev.sh\`.
4. Commit cleanly on branch \`$BRANCH\` by explicit path. Do NOT push or merge.
---

"

PROMPT="${NO_COMPILE_RULE}$(cat "$BRIEF")"

taskpolicy -b nice -n 15 perl -e 'alarm shift; exec @ARGV' 3600 \
  codex exec -m gpt-6.2-sol \
  --dangerously-bypass-approvals-and-sandbox \
  -C "$WT" \
  -c model_reasoning_effort="$effort" \
  -o "$FINAL" \
  "$PROMPT" < /dev/null > "$LOG" 2>&1
rc=$?
echo "$rc" > "$DONE"
exit "$rc"
