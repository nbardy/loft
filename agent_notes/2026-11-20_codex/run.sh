#!/bin/bash
# Run one codex step with a wall-clock limit and one retry: run.sh <name> <model> <effort> <dir>. Resumable.
n=$1; m=$2; e=$3; w=$4; D=$(cd "$(dirname "$0")" && pwd)
[ -s "$D/$n.done" ] && exit 0
# A brief may set its own effort with a line "Effort: low|medium|high|xhigh" (overrides the caller).
o=$(grep -m1 -oE '^Effort: (low|medium|high|xhigh)' "$D/$n.brief.md" | awk '{print $2}'); [ -n "$o" ] && e=$o
case $m in *astra*) lim=${LIM:-10800};; *) lim=${LIM:-5400};; esac   # 3 h Astra, 90 min Sol (90 min cut Astra design passes mid-probe)
export RUSTC_WRAPPER=sccache CARGO_INCREMENTAL=0
# Resource guard (lead, 20 Nov): scale cargo jobs with free memory, and hold new steps while the machine is short.
freepct() { memory_pressure 2>/dev/null | awk -F": " '/free percentage/{gsub("%","",$2); print int($2)}'; }
# One pool (R9, 20 Nov): at most LOFT_MAX_STEPS codex steps run at once across every conductor.
while [ "$(ps -eo args | grep -c "^codex exec.*-C $HOME/git/loft")" -ge "${LOFT_MAX_STEPS:-6}" ]; do sleep 60; done
while [ "$(freepct)" -lt 15 ] 2>/dev/null; do sleep 60; done
f=$(freepct); if [ "${f:-0}" -ge 40 ]; then export CARGO_BUILD_JOBS=4; elif [ "${f:-0}" -ge 25 ]; then export CARGO_BUILD_JOBS=3; else export CARGO_BUILD_JOBS=2; fi
for try in 1 2; do
  perl -e 'alarm shift; exec @ARGV' $lim codex exec -m "$m" --dangerously-bypass-approvals-and-sandbox -C "$w" \
    -c model_reasoning_effort="$e" -o "$D/$n.final.md" "$(cat "$D/$n.brief.md")" < /dev/null >> "$D/$n.log" 2>&1
  rc=$?; [ $rc -eq 0 ] && break
  echo "run.sh: $n try $try exited $rc; retrying (the brief continues from the workspace)" >> "$D/$n.log"
done
echo $rc > "$D/$n.done"
