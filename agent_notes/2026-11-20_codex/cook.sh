#!/bin/bash
# Supervisor: every 10 min, keep the Codex pipeline alive. Finishes chain12, then runs cook cycles 1..MAX
# until Astra writes cook.DONE. Relaunches a dead chain/cycle (resumable). No Claude involvement.
D=$(cd "$(dirname "$0")" && pwd); MAX=${MAX:-8}; L="$D/cook.log"
log() { echo "$(date '+%m-%d %H:%M') $*" >> "$L"; }
alive() { pgrep -f "$D/$1" >/dev/null || pgrep -f "agent_notes/2026-11-20_codex/$1" >/dev/null; }
disk_ok() { [ "$(df -k /System/Volumes/Data | tail -1 | awk '{print int($4/1048576)}')" -ge 15 ]; }
caffeinate -i -w "$$" &
while true; do
  if [ -e "$D/cook.DONE" ]; then log "cook.DONE: stopping"; exit 0; fi
  if ! disk_ok; then log "disk under 15 GB: waiting"; sleep 600; continue; fi
  if [ ! -e "$D/chain12.finished" ] && [ ! -e "$D/chain12.stopped" ]; then
    alive chain12.sh || { log "relaunch chain12"; nohup "$D/chain12.sh" >> "$D/chain12.log" 2>&1 & }
  else
    N=1; while [ -e "$D/cycle$N.finished" ] || [ -e "$D/cycle$N.stopped" ]; do N=$((N+1)); done
    if [ $N -gt $MAX ]; then log "reached $MAX cycles: stopping"; exit 0; fi
    if [ -e "$D/cycle$((N-1)).stopped" ] && [ -e "$D/cycle$((N-2)).stopped" ]; then log "two cycles stopped in a row: stopping"; exit 1; fi
    alive "cycle.sh $N" || { log "launch cycle $N"; nohup "$D/cycle.sh" $N >> "$D/cycle$N.log" 2>&1 & }
  fi
  sleep 600
done
