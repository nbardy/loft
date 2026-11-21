#!/usr/bin/env bash
# Lightweight supervisor: logs wave/step/memory status every 15 minutes until all waves (through wave15_3) finish.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
DIR="$ROOT/agent_notes/2026-11-20_codex"
LOG="$DIR/supervisor.log"

log_status() {
  local ts active free_pct
  ts="$(date '+%Y-%m-%d %H:%M:%S')"
  # File-only status: no pgrep/ps (the 20 Nov freezes were a kernel process-lookup lock convoy those tools feed).
  active="$("$DIR/swarm_status.sh" --stuck 30 2>/dev/null | sed -n '/^-- slots/,/^-- finished/p' | sed '$d' || true)"
  free_pct="$(memory_pressure 2>/dev/null | awk '/System-wide memory free percentage:/ {gsub(/%/,"",$5); print $5}' || echo "?")"
  {
    echo "=== [$ts] free_mem=${free_pct}% ==="
    for w in wave15 wave4 final wave15_2 wave15_3; do
      [ -f "$DIR/${w}.finished" ] && echo "  $w: FINISHED" || echo "  $w: running/waiting"
    done
    local done_list=""
    for s in v15_exec v15_stdlib_finish v15_perf v15_native cap1c v15_bloat cap1b iv15_2 creative_v15_runtime v15_context v15_motion_mood iv15_3a v15_crab_seeds v15_meta_seeds iv15_3b; do
      if [ -f "$DIR/${s}.done" ]; then done_list="${done_list} ${s}"; fi
    done
    echo "  done:${done_list:- (none yet)}"
    if [ -n "$active" ]; then
      echo "  active codex:"
      echo "$active" | sed 's/^/    /'
    else
      echo "  active codex: (none)"
    fi
  } >> "$LOG"
}

while true; do
  log_status
  if [ -f "$DIR/wave15_2.finished" ] && [ -f "$DIR/wave15_3.finished" ]; then
    echo "=== [$(date '+%Y-%m-%d %H:%M:%S')] ALL WAVES COMPLETE (v1.5-train-ready) — supervisor exiting ===" >> "$LOG"
    exit 0
  fi
  sleep 900
done
