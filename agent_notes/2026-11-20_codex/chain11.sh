#!/bin/bash
D=$(cd "$(dirname "$0")" && pwd)
caffeinate -i -w "$$" &
run() { "$D/run.sh" "$1" "$2" "$3" ~/git/loft; }
pids=(); for l in $(cat "$D/v.lanes"); do run $l gpt-6.2-sol high & pids+=($!); sleep 20; done; wait "${pids[@]}"
run iv gpt-6.2-astra xhigh
echo done > "$D/chain11.finished"
