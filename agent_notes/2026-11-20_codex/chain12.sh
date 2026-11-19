#!/bin/bash
D=$(cd "$(dirname "$0")" && pwd)
caffeinate -i -w "$$" &
run() { "$D/run.sh" "$1" "$2" "$3" ~/git/loft; }
run d2 gpt-6.2-astra xhigh
[ -s "$D/d2.lanes" ] || { echo "d2 wrote no lanes" > "$D/chain12.stopped"; exit 1; }
pids=(); for l in $(cat "$D/d2.lanes"); do run $l gpt-6.2-sol high & pids+=($!); sleep 20; done; wait "${pids[@]}"
run i8 gpt-6.2-astra xhigh
echo done > "$D/chain12.finished"
