#!/bin/bash
# Swarm status from files only. It never scans the process table (no ps, pgrep or lsof): the 20 Nov freezes were a
# kernel process-lookup lock convoy that those tools feed.
#   swarm_status.sh            one report
#   swarm_status.sh --stuck N  also flag running steps whose log has not grown in N minutes (default 30)
D=$(cd "$(dirname "$0")" && pwd); STUCK=${2:-30}; now=$(date +%s)
mins() { echo $(( (now - $(stat -f %m "$1")) / 60 )); }
load=$(sysctl -n vm.loadavg | tr -d '{}' | awk '{print $1}')
free=$(memory_pressure 2>/dev/null | awk -F': ' '/free percentage/{print $2}')
echo "== $(date '+%m-%d %H:%M')  load ${load}  free mem ${free:-?}  pool ${LOFT_MAX_STEPS:-1}"
[ -e "$HOME/Library/Logs/FreezeSampler/STALL" ] && echo "!! FREEZE RECORDER REPORTS A PROCESS-LOOKUP STALL: do not run ps, pgrep, lsof or spindump"
echo "-- slots (step holders)"
for f in "$D"/.slots/*; do [ -e "$f" ] || continue; pid=$(cat "$f"); printf "  slot %s  pid %s  since %sm\n" "$(basename "$f")" "$pid" "$(mins "$f")"; done
echo "-- running (log present, no .done)"
for l in "$D"/*.log; do
  s=$(basename "$l" .log); [ -s "$D/$s.done" ] && continue; [ -s "$D/$s.brief.md" ] || continue
  m=$(mins "$l"); flag=""; [ "$m" -ge "$STUCK" ] && flag="  <-- no log growth ${m}m (stuck or interrupted)"
  printf "  %-14s log %5s KB, last write %sm ago%s\n" "$s" "$(( $(stat -f %z "$l") / 1024 ))" "$m" "$flag"
done
echo "-- finished in the last 3 h"
find "$D" -name '*.done' -mmin -180 -exec stat -f '%Sm %N' -t '%H:%M' {} + | sort | sed -E "s#$D/##; s/\.done//" | while read -r t s; do printf "  %s %-14s exit %s\n" "$t" "$s" "$(cat "$D/$s.done")"; done
echo "-- conductors (markers)"
for c in "$D"/wave*.sh; do w=$(basename "$c" .sh); [ -e "$D/$w.finished" ] && st=finished || { [ -s "$D/$w.log" ] && st="started, last log $(mins "$D/$w.log")m ago" || st="not started"; }; printf "  %-10s %s\n" "$w" "$st"; done
gl=/tmp/loft-gpu.lock; [ -e "$gl" ] && echo "-- GPU lock: $(cat "$gl" 2>/dev/null | head -1)  ($(mins "$gl")m)" || true
