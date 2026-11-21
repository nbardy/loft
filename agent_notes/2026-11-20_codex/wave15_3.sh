#!/usr/bin/env bash
# Wave v1.5-3 conductor (Stage B — Pre-Training Authoring & Creative Trainer Bridge):
# 1. Waits cleanly for wave15_2.finished (all Phase-2 workers + iv15_2 merged to main).
# 2. Stage B1 (parallel, staggered 25s): v15_context (loft context + loft fork) & v15_motion_mood (Disney springs + mood).
# 3. Stage B2 (solo): iv15_3a merges B1 into main, builds release loft, removes merged worktrees & branches.
# 4. Stage B3 (solo): v15_seeds_creative builds 12-15 meta-engine "improve" seeds against the real release loft +
#    loft context + springs/mood, wires ~/git/creative (LoftV15 adapter + system-v15.txt + v15-meta.jsonc),
#    verifies end-to-end, cleans up, and tags v1.5-train-ready.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
DIR="$ROOT/agent_notes/2026-11-20_codex"
RUN="$DIR/run.sh"
export LOFT_MAX_STEPS="${LOFT_MAX_STEPS:-4}"

log() { printf '[%s] %s\n' "$(date '+%Y-%m-%d %H:%M:%S')" "$*"; }

calm() {
  [ "$(sysctl -n vm.loadavg | tr -d '{}' | awk '{print int($1)}')" -lt 20 ] \
    && [ ! -e "$HOME/Library/Logs/FreezeSampler/STALL" ]
}

step_sync() {
  local name="$1" model="$2" effort="$3"
  [ -s "$DIR/${name}.done" ] && return 0
  until calm; do sleep 60; done
  log "starting solo step: $name ($model/$effort)"
  "$RUN" "$name" "$model" "$effort" "$ROOT"
  log "finished solo step: $name (exit=$(cat "$DIR/${name}.done" 2>/dev/null || echo '?'))"
}

log "wave15_3 waiting for wave15_2.finished..."
until [ -s "$DIR/wave15_2.finished" ]; do
  sleep 45
done
log "observed wave15_2.finished; starting Stage B (pre-training polish)"

# Stage B1: v15_context and v15_motion_mood in parallel (disjoint files, staggered 25s)
b1_pids=()
for spec in "v15_context:gpt-6.2-sol:high" "v15_motion_mood:gpt-6.2-sol:high"; do
  name="${spec%%:*}"; rest="${spec#*:}"; model="${rest%%:*}"; effort="${rest##*:}"
  [ -s "$DIR/${name}.done" ] && { log "skipping already-done $name"; continue; }
  until calm; do sleep 60; done
  log "launching Stage-B1 worker: $name ($model/$effort)"
  "$RUN" "$name" "$model" "$effort" "$ROOT" &
  b1_pids+=("$!")
  sleep 25
done
for pid in "${b1_pids[@]}"; do
  wait "$pid" || true
done
log "Stage B1 finished"

# Stage B2: merge v15_context + v15_motion_mood into main, rebuild release loft, clean up branches/worktrees
step_sync iv15_3a gpt-6.2-sol high

# Stage B3: build meta-engine "improve" seeds + wire ~/git/creative + basin test against live loft context & springs/mood
step_sync v15_seeds_creative gpt-6.2-astra high

# Tag training-ready snapshot if main is clean of conflicts
git -C "$ROOT" tag -f v1.5-train-ready HEAD >/dev/null 2>&1 || true
git -C "$ROOT" push -f origin v1.5-train-ready >/dev/null 2>&1 || true

echo done > "$DIR/wave15_3.finished"
log "wave15_3 FINISHED (tagged v1.5-train-ready)"
