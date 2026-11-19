#!/bin/bash
D=$(cd "$(dirname "$0")" && pwd)
caffeinate -i -w "$$" &
run() { "$D/run.sh" "$1" "$2" "$3" ~/git/loft; }
run fxcut gpt-6.2-sol high
run rr1 gpt-6.2-sol high
echo done > "$D/chain9.finished"
