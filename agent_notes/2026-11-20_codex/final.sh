#!/bin/bash
# Final sequence, one step at a time, each only when the machine is calm (load < 20, no recorded stall).
# File-only checks: no ps, pgrep or lsof.
D=$(cd "$(dirname "$0")" && pwd)
calm() { [ "$(sysctl -n vm.loadavg | tr -d '{}' | awk '{print int($1)}')" -lt 20 ] && [ ! -e "$HOME/Library/Logs/FreezeSampler/STALL" ]; }
step() { until calm; do sleep 120; done; "$D/run.sh" "$@"; }
B=/var/tmp/spindump-stuck-stackshot.2026-11-20-20:18:32.27983.buf
until calm; do sleep 120; done
[ -e "$B" ] && [ ! -s ~/git/loft/out/meta/stall/stackshot.txt ] && { mkdir -p ~/git/loft/out/meta/stall; /usr/sbin/spindump -i "$B" -noFile -noBinary -o ~/git/loft/out/meta/stall/stackshot.txt 2>/dev/null; }
step cl1 gpt-6.2-sol medium ~/git/cleanup_tools
step ix4 gpt-6.2-astra high ~/git/loft
step fs1 gpt-6.2-sol medium ~/git/freeze_sampler
step cam2 gpt-6.2-sol high ~/git/_wt/cambium-v010
step fs2 gpt-6.2-sol high ~/git/freeze_sampler
step wrap1 gpt-6.2-astra high ~/git/loft
echo done > "$D/final.finished"
