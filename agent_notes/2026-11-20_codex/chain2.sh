#!/bin/bash
# Loft v1.0 build: per phase, implement (Sol; Astra for 1.5, already running) -> review (Astra) -> fix + merge (Sol). Resumable.
D=$(cd "$(dirname "$0")" && pwd)
wait_done() { until [ -s "$D/$1.done" ]; do sleep 60; done; }
run() { "$D/run.sh" "$1" "$2" "$3" ~/git/loft; }
ok()  { [ "$(cat "$D/$1.done")" = "0" ] || { echo "stop at $1" > "$D/chain.stopped"; exit 1; }; }
# Phase 1.5 implementer (Astra) was started by the first chain; reuse its run if it is still going.
pgrep -f "p15.brief.md" >/dev/null && wait_done p15 || run p15 gpt-6.2-astra xhigh; ok p15
run rp15 gpt-6.2-astra xhigh; ok rp15; run fp15 gpt-6.2-sol high; ok fp15
for p in p2 p3 p4 p5; do
  run $p gpt-6.2-sol high; ok $p
  run r$p gpt-6.2-astra xhigh; ok r$p
  run f$p gpt-6.2-sol high; ok f$p
done
echo done > "$D/chain.finished"
