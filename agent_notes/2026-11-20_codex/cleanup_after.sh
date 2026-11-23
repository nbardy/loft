#!/bin/bash
# After div2 and imp1 finish: prune old Cambium caches, then remove Loft worktrees that are merged and clean.
# Real (APFS) space is measured with df, not du. Cambium's guards refuse dirty or unmerged workspaces.
D=$(cd "$(dirname "$0")" && pwd); L="$D/cleanup_after.log"
until [ -s "$D/div2.done" ] && [ -s "$D/imp1.done" ]; do sleep 300; done
exec >>"$L" 2>&1; echo "== $(date)"
avail() { df -k / | awk 'NR==2{print $4}'; }
a=$(avail)
for r in loft creative; do (cd ~/git/$r && ~/.local/bin/agent-build cambium prune --older-than 24h --json | tail -c 600; echo); done
b=$(avail); echo "prune freed $(( (b - a) / 1024 )) MiB (df)"
cd ~/git/loft && git fetch -q origin
for n in perf-frame-legacy motion-nfeet div2-styles perf-frame perf-load perf-instrument perf-precision; do
  if cambium remove --require-merged origin/main "$n"; then echo "removed $n"; else echo "kept $n (dirty, unmerged or in use)"; fi
done
c=$(avail); echo "worktrees freed $(( (c - b) / 1024 )) MiB (df); total $(( (c - a) / 1024 )) MiB"
touch "$D/cleanup_after.finished"
