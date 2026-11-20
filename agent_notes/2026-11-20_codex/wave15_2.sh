#!/usr/bin/env bash
# Wave v1.5-2 conductor: calm-gated & file-only (no ps/pgrep/lsof).
# Waits for final.finished (cam2/fs2/wrap1) + wave15.finished + ix4.done,
# runs solo R2 v15_exec first (folding held v15a/b/c), then 6 tight Phase-2 streams
# sequentially through run.sh (default LOFT_MAX_STEPS=1, E-core polite), then iv15_2 (Astra).
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
DIR="$ROOT/agent_notes/2026-11-20_codex"
RUN="$DIR/run.sh"
export LOFT_MAX_STEPS="${LOFT_MAX_STEPS:-1}"

calm() {
  [ "$(sysctl -n vm.loadavg | tr -d '{}' | awk '{print int($1)}')" -lt 20 ] \
    && [ ! -e "$HOME/Library/Logs/FreezeSampler/STALL" ]
}

step() {
  local name="$1" model="$2" effort="$3"
  [ -s "$DIR/${name}.done" ] && return 0
  until calm; do sleep 120; done
  "$RUN" "$name" "$model" "$effort" "$ROOT"
}

# Wait until the active anti-freeze wrap-up (final.sh: cam2/fs2/wrap1) and prior waves finish
until [ -s "$DIR/final.finished" ] && [ -s "$DIR/wave15.finished" ] && [ -s "$DIR/ix4.done" ]; do
  sleep 60
done

# Phase 1: R2 solo core refactor + fold held v15a/b/c
step v15_exec gpt-6.2-sol high

# Phase 2: 6 tight conflict-free streams (calm-gated, polite)
step v15_stdlib_finish gpt-6.2-sol high
step v15_perf          gpt-6.2-sol high
step v15_native        gpt-6.2-sol high
step cap1c             gpt-6.2-sol high
step v15_bloat         gpt-6.2-sol medium
step cap1b             gpt-6.2-sol high

# Phase 3: Wave v1.5-2 integrator (Astra)
step iv15_2 gpt-6.2-astra high

echo done > "$DIR/wave15_2.finished"
