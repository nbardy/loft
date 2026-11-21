#!/usr/bin/env bash
# Wave v1.5-3 conductor (Stage B — Pre-Training Authoring, Crab-Overfit Migration & Creative Trainer Bridge):
# - creative_v15_runtime runs IMMEDIATELY in parallel (pure Python in ~/git/creative, zero Rust build contention).
# - Once wave15_2.finished drops (all Phase-2 Loft workers + iv15_2 merged to main):
#   - Stage B1 (parallel, staggered 25s): v15_context (AST loft context + loft fork) & v15_motion_mood (Disney springs + mood).
#   - Stage B2 (solo): iv15_3a merges B1 into main, rebuilds release target/release/loft, cleans up worktrees & branches.
#   - Stage B3 (parallel, waits for iv15_3a + creative_v15_runtime.done):
#       v15_crab_seeds (migrated + full-engine-breadth crab-overfit-v15 seed pack in ~/git/creative)
#     + v15_meta_seeds (non-crab general meta-engine seeds + basin_test.sh in ~/git/loft).
#   - Stage B4 (solo): iv15_3b (Astra) integrates everything, runs pytest + LoftV15 render smoke, cleans up all
#     ephemeral worktrees & merged branches, updates SPRINT_TO_TRAIN.md, and tags v1.5-train-ready.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
DIR="$ROOT/agent_notes/2026-11-20_codex"
RUN="$DIR/run.sh"
export LOFT_MAX_STEPS="${LOFT_MAX_STEPS:-6}"

log() { printf '[%s] %s\n' "$(date '+%Y-%m-%d %H:%M:%S')" "$*"; }

calm() {
  [ "$(sysctl -n vm.loadavg | tr -d '{}' | awk '{print int($1)}')" -lt 25 ] \
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

# Stage B0 (IMMEDIATE, ZERO WAIT): launch creative_v15_runtime in ~/git/creative (pure Python, no Rust build!)
creative_pid=""
if [ ! -s "$DIR/creative_v15_runtime.done" ]; then
  if [ -s "$DIR/creative_v15_runtime.log" ]; then
    log "creative_v15_runtime already in progress"
  else
    log "launching IMMEDIATE Stage-B0 worker: creative_v15_runtime (gpt-6.2-sol/high, Python-only)"
    LOFT_MAX_STEPS=8 "$RUN" creative_v15_runtime gpt-6.2-sol high "$ROOT" &
    creative_pid="$!"
  fi
fi

log "wave15_3 waiting for wave15_2.finished (while creative_v15_runtime runs in parallel)..."
until [ -s "$DIR/wave15_2.finished" ]; do
  sleep 45
done
log "observed wave15_2.finished; starting Stage B1 (v15_context + v15_motion_mood in parallel)"

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

# Ensure creative_v15_runtime is done before v15_crab_seeds branches off loft/v15-runtime
if [ -n "$creative_pid" ]; then
  wait "$creative_pid" || true
fi
until [ -s "$DIR/creative_v15_runtime.done" ]; do
  sleep 30
done
log "creative_v15_runtime done (exit=$(cat "$DIR/creative_v15_runtime.done")); starting Stage B3 (v15_crab_seeds + v15_meta_seeds in parallel)"

# Stage B3: launch v15_crab_seeds and v15_meta_seeds in parallel (disjoint repos/files, staggered 25s)
b3_pids=()
for spec in "v15_crab_seeds:gpt-6.2-sol:high" "v15_meta_seeds:gpt-6.2-sol:high"; do
  name="${spec%%:*}"; rest="${spec#*:}"; model="${rest%%:*}"; effort="${rest##*:}"
  [ -s "$DIR/${name}.done" ] && { log "skipping already-done $name"; continue; }
  until calm; do sleep 60; done
  log "launching Stage-B3 worker: $name ($model/$effort)"
  "$RUN" "$name" "$model" "$effort" "$ROOT" &
  b3_pids+=("$!")
  sleep 25
done
for pid in "${b3_pids[@]}"; do
  wait "$pid" || true
done
log "Stage B3 finished"

# Stage B4: Astra final integration across loft + creative, smoke test, cleanup, tag v1.5-train-ready
step_sync iv15_3b gpt-6.2-astra high

echo done > "$DIR/wave15_3.finished"
log "wave15_3 FINISHED (v1.5-train-ready)"
