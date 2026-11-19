#!/bin/bash
# Run one codex lane: run.sh <name> <model> <effort> <worktree>; brief at <name>.brief.md; resumable (skips if done).
n=$1; m=$2; e=$3; w=$4; D=$(cd "$(dirname "$0")" && pwd)
[ -s "$D/$n.done" ] && exit 0
codex exec -m "$m" --dangerously-bypass-approvals-and-sandbox -C "$w" -c model_reasoning_effort="$e" \
  -o "$D/$n.final.md" "$(cat "$D/$n.brief.md")" < /dev/null > "$D/$n.log" 2>&1
echo $? > "$D/$n.done"
