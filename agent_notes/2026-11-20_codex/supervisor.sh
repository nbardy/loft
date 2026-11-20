#!/usr/bin/env bash
# Lightweight supervisor: logs wave/step/memory status every 15 minutes until all waves finish.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
DIR="$ROOT/agent_notes/2026-11-20_codex"
LOG="$DIR/supervisor.log"

log_status() {
  local ts active free_pct
  ts="$(date '+%Y-%m-%d %H:%M:%S')"
  active="$(pgrep -f "codex exec.*-C ($HOME/git/loft|$HOME/git/_wt/)" 2>/dev/null | while read -r p; do ps -o pid=,etime=,comm= -p "$p" 2>/dev/null; done || true)"
  free_pct="$(memory_pressure 2>/dev/null | awk '/System-wide memory free percentage:/ {gsub(/%/,"",$5); print $5}' || echo "?")"
  {
    echo "=== [$ts] free_mem=${free_pct}% ==="
    for w in wave15 wave4 wave15_2; do
      [ -f "$DIR/${w}.finished" ] && echo "  $w: FINISHED" || echo "  $w: running/waiting"
    done
    local done_list=""
    for s in build1 cam1 v15a v15b v15c iv15 std1b std1c std1d std1e port1 ix4 v15_exec v15_perf v15_native cap1c v15_bloat cap1b iv15_2; do
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
  if [ -f "$DIR/wave15.finished" ] && [ -f "$DIR/wave4.finished" ] && [ -f "$DIR/wave15_2.finished" ] \
     && [ -f "$DIR/build1.done" ] && [ -f "$DIR/cam1.done" ]; then
    echo "=== [$(date '+%Y-%m-%d %H:%M:%S')] ALL WAVES COMPLETE — supervisor exiting ===" >> "$LOG"
    exit 0
  fi
  sleep 900
done
