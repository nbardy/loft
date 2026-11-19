#!/bin/bash
# One cook cycle N: Astra plans (pN) -> Sol lanes in parallel -> Astra integrates (iN). Resumable (run.sh skips done steps).
N=$1; D=$(cd "$(dirname "$0")" && pwd)
caffeinate -i -w "$$" &
run() { "$D/run.sh" "$1" "$2" "$3" ~/git/loft; }
[ -s "$D/p$N.brief.md" ] || sed "s/__N__/$N/g" "$D/plan_template.md" > "$D/p$N.brief.md"
run p$N gpt-6.2-astra xhigh
[ -e "$D/cook.DONE" ] && { echo done > "$D/cycle$N.finished"; exit 0; }
[ -s "$D/c$N.lanes" ] && [ -s "$D/i$N.brief.md" ] || { echo "p$N wrote no lanes or integrator brief" > "$D/cycle$N.stopped"; exit 1; }
pids=(); for l in $(cat "$D/c$N.lanes"); do run $l gpt-6.2-sol high & pids+=($!); sleep 20; done; wait "${pids[@]}"
run i$N gpt-6.2-astra xhigh
echo done > "$D/cycle$N.finished"
