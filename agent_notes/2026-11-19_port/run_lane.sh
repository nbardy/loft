#!/bin/bash
# Launch one port lane: its worktree from main (tools/lane.sh), then a Codex implementer on its brief, unsandboxed so
# the harness's TS side can bake on the GPU. The owner launches; nothing here runs on its own.
#   agent_notes/2026-11-19_port/run_lane.sh <LANE> [model]      e.g. run_lane.sh G1
# Log, final message and exit status land in out/port_lanes/ of the primary checkout.
set -euo pipefail
lane="$1"; model="${2:-gpt-6.2-sol}"
repo="$(cd "$(dirname "$0")/../.." && pwd -P)"
lc="$(echo "$lane" | tr '[:upper:]' '[:lower:]')"
brief="$repo/agent_notes/2026-11-19_port/lanes/$lane.md"
[[ -f "$brief" ]] || { echo "run_lane: no brief $brief" >&2; exit 2; }
wt="$HOME/git/_wt/loft-port-$lc"
[[ -d "$wt" ]] || "$repo/tools/lane.sh" new "loft-port-$lc" "port/$lc" main
logs="$repo/out/port_lanes"; mkdir -p "$logs"
command codex exec -m "$model" --dangerously-bypass-approvals-and-sandbox -C "$wt" -c model_reasoning_effort=high \
  -o "$logs/$lane.final.md" "$(cat "$brief")" < /dev/null > "$logs/$lane.log" 2>&1
echo "exit=$?" > "$logs/$lane.done"
