#!/bin/bash
# Merge Phase 2+3, then a design pass on the failure classes (two-strike rule), its Sol lanes in parallel, then one integration.
D=$(cd "$(dirname "$0")" && pwd)
caffeinate -i -w "$$" &
run() { "$D/run.sh" "$1" "$2" "$3" ~/git/loft; }
run m23 gpt-6.2-astra xhigh
run d1 gpt-6.2-astra xhigh
[ -s "$D/d1.lanes" ] || { echo "d1 wrote no lanes" > "$D/chain6.stopped"; exit 1; }
pids=(); for l in $(cat "$D/d1.lanes"); do run $l gpt-6.2-sol high & pids+=($!); sleep 20; done; wait "${pids[@]}"
run i6 gpt-6.2-astra xhigh
echo done > "$D/chain6.finished"
