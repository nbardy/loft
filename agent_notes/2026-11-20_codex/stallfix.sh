#!/bin/bash
# Run the anti-freeze fixes one at a time, only when the machine is calm (load < 20). Never scans the process table.
D=$(cd "$(dirname "$0")" && pwd)
calm() { [ "$(sysctl -n vm.loadavg | tr -d '{}' | awk '{print int($1)}')" -lt 20 ] && [ ! -e "$HOME/Library/Logs/FreezeSampler/STALL" ]; }
until calm; do sleep 120; done
B=/var/tmp/spindump-stuck-stackshot.2026-11-20-20:18:32.27983.buf
[ -e "$B" ] && [ ! -s ~/git/loft/out/meta/stall/stackshot.txt ] && { mkdir -p ~/git/loft/out/meta/stall; /usr/sbin/spindump -i "$B" -noFile -noBinary -o ~/git/loft/out/meta/stall/stackshot.txt 2>/dev/null; }
until calm; do sleep 120; done; "$D/run.sh" fs1 gpt-6.2-sol medium ~/git/freeze_sampler
until calm; do sleep 120; done; "$D/run.sh" cam2 gpt-6.2-sol high ~/git/_wt/cambium-v010
echo done > "$D/stallfix.finished"
