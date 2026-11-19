#!/bin/bash
D=$(cd "$(dirname "$0")" && pwd)
caffeinate -i -w "$$" &
run() { "$D/run.sh" "$1" "$2" "$3" ~/git/loft; }
run rr gpt-6.2-sol high & RR=$!
run rel gpt-6.2-astra xhigh
[ -s ~/git/loft/out/meta/rel/MERGED ] && run cut gpt-6.2-astra xhigh
wait $RR
echo done > "$D/chain8.finished"
