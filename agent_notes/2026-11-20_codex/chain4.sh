#!/bin/bash
# Loft v1.0, wider fan-out: Phase 2+3 and Phase 4 lanes run together (7 Sol lanes), then their integrators in turn;
# then Phase 5's 7 lanes together. Builds queue on the shared compile lock; lanes compile once at the end.
D=$(cd "$(dirname "$0")" && pwd)
caffeinate -i -w "$$" &
disk() { f=$(df -k /System/Volumes/Data | tail -1 | awk '{print int($4/1048576)}'); [ "$f" -ge 15 ] || { echo "stop: only ${f} GB free" > "$D/chain.stopped"; exit 1; }; }
wait_done() { until [ -s "$D/$1.done" ]; do sleep 60; done; }
run() { "$D/run.sh" "$1" "$2" "$3" ~/git/loft; }
ok()  { [ "$(cat "$D/$1.done")" = "0" ] || { echo "stop at $1" > "$D/chain.stopped"; exit 1; }; }
step() { if pgrep -f "run.sh $1 " >/dev/null; then wait_done $1; else run $1 $2 $3; fi; ok $1; }
lanes() { for l in "$@"; do run $l gpt-6.2-sol high & sleep 20; done; wait; }   # the integrators judge lane failures
step rp15 gpt-6.2-astra xhigh
step fp15 gpt-6.2-sol high
step d0 gpt-6.2-sol high
disk; step s0 gpt-6.2-astra xhigh
step z23 gpt-6.2-astra xhigh
step z4 gpt-6.2-astra xhigh
disk; lanes l23a l23b l23c l23d l4a l4b l4c
step i23 gpt-6.2-astra xhigh
step i4 gpt-6.2-astra xhigh
disk; step z5 gpt-6.2-astra xhigh
lanes l5a l5b l5g l5c l5d l5e l5f
step i5 gpt-6.2-astra xhigh
echo done > "$D/chain.finished"
