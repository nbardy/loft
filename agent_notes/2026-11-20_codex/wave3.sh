#!/bin/bash
# Wave x3 (the scientist's workers): runs after wave x2 has finished. s1_escapes + s1_controls_fix in parallel, then ix3.
D=$(cd "$(dirname "$0")" && pwd)
caffeinate -i -w "$$" &
until [ -e "$D/wave2.finished" ]; do sleep 120; done
run() { "$D/run.sh" "$1" "$2" "$3" ~/git/loft; }
run s1_escapes gpt-6.2-sol high & a=$!; sleep 20; run s1_controls_fix gpt-6.2-sol medium & b=$!; wait $a $b
run ix3 gpt-6.2-astra high
echo done > "$D/wave3.finished"
