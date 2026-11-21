#!/bin/bash
# Run one codex step with a wall-clock limit and one retry: run.sh <name> <model> <effort> <dir>. Resumable.
n=$1; m=$2; e=$3; w=$4; D=$(cd "$(dirname "$0")" && pwd)
[ -s "$D/$n.done" ] && exit 0
# A brief may set its own effort with a line "Effort: low|medium|high|xhigh" (overrides the caller).
o=$(grep -m1 -oE '^Effort: (low|medium|high|xhigh)' "$D/$n.brief.md" | awk '{print $2}'); [ -n "$o" ] && e=$o
case $m in *astra*) lim=${LIM:-10800};; *) lim=${LIM:-5400};; esac   # 3 h Astra, 90 min Sol
export RUSTC_WRAPPER=sccache CARGO_INCREMENTAL=0
export PATH="${CARGO_HOME:-$HOME/.cargo}/bin:$PATH"
# Polite resource guard (20 Nov): keep desktop & games responsive on 24 GB MacBook Air.
freepct() { memory_pressure 2>/dev/null | awk -F": " '/free percentage/{gsub("%","",$2); print int($2)}'; }
game_running() { pgrep -qf "TheBazaar|The Bazaar" 2>/dev/null; }
# One pool (R9, 20 Nov): default LOFT_MAX_STEPS=1 while interactive; override to 2 overnight/AFK.
# Slots are lock files, so no process-table scans (the 20:17 freeze was a kernel process-lookup lock convoy that pgrep/ps/lsof feed).
# Atomic create with noclobber; a slot whose owner PID is gone (kill -0 on one PID) is freed.
S="$D/.slots"; mkdir -p "$S"; slot=""
while [ -z "$slot" ]; do
  for i in $(seq 1 "${LOFT_MAX_STEPS:-1}"); do
    if ( set -C; echo $$ > "$S/$i" ) 2>/dev/null; then slot="$S/$i"; break; fi
    o=$(cat "$S/$i" 2>/dev/null); if [ -n "$o" ] && ! kill -0 "$o" 2>/dev/null; then rm -f "$S/$i"; fi
  done
  [ -z "$slot" ] && sleep 60
done
trap 'rm -f "$slot"' EXIT
while [ "$(freepct)" -lt 25 ] 2>/dev/null; do sleep 60; done
f=$(freepct)
if game_running || [ "${f:-0}" -lt 35 ]; then
  export CARGO_BUILD_JOBS=1
else
  export CARGO_BUILD_JOBS=2
fi
for try in 1 2; do
  "$D/../../tools/quiet_gpu.sh" --wait || exit $?
  # Background QoS (E-cores + throttled I/O) + nice +15 so foreground apps/games never starve.
  taskpolicy -b nice -n 15 perl -e 'alarm shift; exec @ARGV' $lim codex exec -m "$m" --dangerously-bypass-approvals-and-sandbox -C "$w" \
    -c model_reasoning_effort="$e" -o "$D/$n.final.md" "$(cat "$D/$n.brief.md")" < /dev/null >> "$D/$n.log" 2>&1
  rc=$?; [ $rc -eq 0 ] && break
  echo "run.sh: $n try $try exited $rc; retrying (the brief continues from the workspace)" >> "$D/$n.log"
done
echo $rc > "$D/$n.done"
