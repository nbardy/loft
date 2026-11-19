#!/bin/bash
# Run Phases 1.5 -> 5 in order on codex; each phase merges to main itself when its gates pass. Resumable.
D=$(cd "$(dirname "$0")" && pwd); cd ~/git/loft
run() { "$D/run.sh" "$1" "$2" "$3" ~/git/loft; [ "$(cat "$D/$1.done")" = "0" ] || { echo "stop at $1" > "$D/chain.stopped"; exit 1; }; }
run p15 gpt-6.2-astra xhigh
run p2  gpt-6.2-astra xhigh
run p3  gpt-6.2-astra xhigh
run p4  gpt-6.2-astra xhigh
run p5  gpt-6.2-astra xhigh
echo done > "$D/chain.finished"
