#!/bin/bash
# Run fs2 after fs1, only when calm (load < 20, no recorded stall). File-only checks.
D=$(cd "$(dirname "$0")" && pwd)
calm() { [ "$(sysctl -n vm.loadavg | tr -d '{}' | awk '{print int($1)}')" -lt 20 ] && [ ! -e "$HOME/Library/Logs/FreezeSampler/STALL" ]; }
until [ -s "$D/fs1.done" ] && calm; do sleep 120; done
"$D/run.sh" fs2 gpt-6.2-sol high ~/git/freeze_sampler
