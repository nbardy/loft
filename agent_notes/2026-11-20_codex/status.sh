#!/bin/bash
# Loft v1.0 pipeline status: which steps finished, which is running, latest commits.
D=$(cd "$(dirname "$0")" && pwd)
steps="p15 rp15 fp15 d0 s0 z23 l23a l23b l23c l23d i23 z4 l4a l4b l4c i4 z5 l5a l5b l5g l5c l5d l5e l5f i5"
pgrep -f chain3.sh >/dev/null && echo "pipeline: RUNNING" || echo "pipeline: not running"
[ -e "$D/chain.stopped" ] && echo "STOPPED: $(cat "$D/chain.stopped")"; [ -e "$D/chain.finished" ] && echo "FINISHED"
for s in $steps; do
  if [ -s "$D/$s.done" ]; then st="done (exit $(cat "$D/$s.done"))"
  elif pgrep -f "run.sh $s " >/dev/null; then st="RUNNING, log $(( $(wc -c < "$D/$s.log" 2>/dev/null || echo 0) / 1024 )) KB"
  else st="-"; fi
  printf "  %-5s %s\n" "$s" "$st"
done
echo "latest commits on loft branches:"
git -C ~/git/loft for-each-ref --sort=-committerdate --count=5 --format='  %(committerdate:relative)  %(refname:short)  %(subject)' refs/heads/loft
