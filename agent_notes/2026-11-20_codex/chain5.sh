#!/bin/bash
# Fix wave: per phase, Sol fixes the integrator's findings, then Astra re-scores and merges (MERGED marker = real merge).
D=$(cd "$(dirname "$0")" && pwd); L=~/git/loft/out/meta
caffeinate -i -w "$$" &
run() { "$D/run.sh" "$1" "$2" "$3" ~/git/loft; }
merged() { [ -s "$L/$1-r2/MERGED" ]; }
for p in 23 4 5; do
  run fx$p gpt-6.2-sol high
  run ri$p gpt-6.2-astra xhigh
  merged i$p || echo "i$p not merged: $(cat $L/i$p-r2/BLOCKED 2>/dev/null | head -3)" >> "$D/chain5.blocked"
done
echo done > "$D/chain5.finished"
