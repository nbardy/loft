#!/bin/bash
# Final wrap-up: once ix4, fs1, cam2 and fs2 are done, and the machine is calm, run the wrap1 integrator. File-only checks.
D=$(cd "$(dirname "$0")" && pwd)
calm() { [ "$(sysctl -n vm.loadavg | tr -d '{}' | awk '{print int($1)}')" -lt 20 ] && [ ! -e "$HOME/Library/Logs/FreezeSampler/STALL" ]; }
until [ -s "$D/ix4.done" ] && [ -s "$D/fs1.done" ] && [ -s "$D/cam2.done" ] && [ -s "$D/fs2.done" ] && calm; do sleep 120; done
"$D/run.sh" wrap1 gpt-6.2-astra high ~/git/loft
echo done > "$D/wrapup.finished"
