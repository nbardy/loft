#!/bin/bash
# Run one Codex implementer in a worktree, unsandboxed so it can use the GPU. Usage: run_lane.sh <lane> <model> <worktree>
set -u
dir="$(cd "$(dirname "$0")" && pwd)"; lane="$1"; model="$2"; wt="$3"
command codex exec -m "$model" --dangerously-bypass-approvals-and-sandbox -C "$wt" -c model_reasoning_effort=high \
  -o "$dir/$lane.final.md" "$(cat "$dir/COMMON.md" "$dir/$lane.md")" < /dev/null > "$dir/$lane.log" 2>&1
echo "exit=$?" > "$dir/$lane.done"
