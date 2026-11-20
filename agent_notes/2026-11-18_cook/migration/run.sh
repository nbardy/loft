#!/bin/bash
# Run one Codex worker on a brief file. Usage: run.sh <name> <model> <worktree> <brief>
set -u
dir="$(cd "$(dirname "$0")" && pwd)"; name="$1"; model="$2"; wt="$3"; brief="$4"
command codex exec -m "$model" --dangerously-bypass-approvals-and-sandbox -C "$wt" -c model_reasoning_effort=high \
  -o "$dir/$name.final.md" "$(cat "$brief")" < /dev/null > "$dir/$name.log" 2>&1
echo "exit=$?" > "$dir/$name.done"
