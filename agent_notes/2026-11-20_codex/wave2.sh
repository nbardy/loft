#!/bin/bash
# Lead's parallel wave x2: five Sol lanes + Astra lean audit in parallel, then one integrator.
D=$(cd "$(dirname "$0")" && pwd)
caffeinate -i -w "$$" &
run() { "$D/run.sh" "$1" "$2" "$3" ~/git/loft; }
run x2f gpt-6.2-astra xhigh & A=$!
pids=(); for l in x2a x2b x2c x2d x2e; do run $l gpt-6.2-sol high & pids+=($!); sleep 20; done; wait "${pids[@]}"
run ix2 gpt-6.2-astra xhigh
wait $A
[ -s "$D/lean1.lanes" ] && { pids=(); for l in $(cat "$D/lean1.lanes"); do run $l gpt-6.2-sol high & pids+=($!); sleep 20; done; wait "${pids[@]}"
  sed -e 's/w1–w4/the lean1 lanes/; s#loft/w1..w4#loft/lean1a..d#; s#lanes/w\*.md#lanes/lean1*.md#; s#w\*.brief.md#lean1*.brief.md#; s#loft/intw#loft/intlean#; s#out/meta/iw#out/meta/ilean#g' "$D/iw.brief.md" > "$D/ilean.brief.md"
  run ilean gpt-6.2-astra xhigh; }
echo done > "$D/wave2.finished"
