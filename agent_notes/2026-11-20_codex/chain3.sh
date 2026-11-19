#!/bin/bash
# Loft v1.0 fan-out build. Freeze (Astra) -> parallel lanes (Sol, CPU only, <=3 Rust-heavy) -> audit + integrate + one GPU gate (Astra).
D=$(cd "$(dirname "$0")" && pwd)
wait_done() { until [ -s "$D/$1.done" ]; do sleep 60; done; }
run() { "$D/run.sh" "$1" "$2" "$3" ~/git/loft; }
ok()  { [ "$(cat "$D/$1.done")" = "0" ] || { echo "stop at $1" > "$D/chain.stopped"; exit 1; }; }
lanes() { for l in "$@"; do run $l gpt-6.2-sol high & sleep 20; done; wait; }   # lane failures are judged by the integrator
# Phase 1.5 (Astra, already running) -> review -> fix
if pgrep -f "run.sh p15 " >/dev/null; then wait_done p15; else run p15 gpt-6.2-astra xhigh; fi; ok p15
run rp15 gpt-6.2-astra xhigh; ok rp15
run fp15 gpt-6.2-sol high; ok fp15
# Step 0 artifacts
run s0 gpt-6.2-astra xhigh; ok s0
# Phases 2+3
run z23 gpt-6.2-astra xhigh; ok z23
lanes l23a l23b l23d; lanes l23c          # three Rust-heavy lanes, then the .loft lane
run i23 gpt-6.2-astra xhigh; ok i23
# Phase 4
run z4 gpt-6.2-astra xhigh; ok z4
lanes l4a l4b l4c
run i4 gpt-6.2-astra xhigh; ok i4
# Phase 5
run z5 gpt-6.2-astra xhigh; ok z5
lanes l5a l5b l5c; lanes l5d l5e l5f
run i5 gpt-6.2-astra xhigh; ok i5
echo done > "$D/chain.finished"
