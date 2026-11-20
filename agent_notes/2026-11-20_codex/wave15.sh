#!/bin/bash
# v1.5 wave: starts once the stdlib restructure (std1a) has landed, runs alongside wave x4's ports. v15a/b/c, then iv15.
D=$(cd "$(dirname "$0")" && pwd)
caffeinate -i -w "$$" &
until [ -s "$D/std1a.done" ]; do sleep 120; done
run() { "$D/run.sh" "$1" "$2" "$3" ~/git/loft; }
pids=(); for l in v15a v15b v15c; do run $l gpt-6.2-sol high & pids+=($!); sleep 20; done; wait "${pids[@]}"
run iv15 gpt-6.2-astra high
echo done > "$D/wave15.finished"
