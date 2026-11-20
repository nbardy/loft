#!/bin/bash
# Performance diagnosis (Astra) -> its lanes (Sol, parallel) -> one integration (Astra).
D=$(cd "$(dirname "$0")" && pwd)
caffeinate -i -w "$$" &
run() { "$D/run.sh" "$1" "$2" "$3" ~/git/loft; }
run pf1 gpt-6.2-astra xhigh
[ -s "$D/pf1.lanes" ] || { echo "pf1 wrote no lanes" > "$D/pfchain.stopped"; exit 1; }
pids=(); for l in $(cat "$D/pf1.lanes"); do run $l gpt-6.2-sol high & pids+=($!); sleep 20; done; wait "${pids[@]}"
sed -e 's/w1–w4/the pf1 lanes/; s#loft/w1..w4#loft/pf1a..pf1c#; s#lanes/w\*.md#lanes/pf1*.md#; s#w\*.brief.md#pf1*.brief.md#; s#loft/intw#loft/intpf#; s#out/meta/iw#out/meta/ipf#g' "$D/iw.brief.md" > "$D/ipf.brief.md"
run ipf gpt-6.2-astra xhigh
echo done > "$D/pfchain.finished"
