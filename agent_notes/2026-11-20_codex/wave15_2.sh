#!/usr/bin/env bash
# Wave v1.5-2 conductor: calm-gated, file-only (zero ps/pgrep/lsof), parallel Phase 2.
# 1. Waits for final.finished + wave15.finished + ix4.done.
# 2. Runs Phase 1 solo (v15_exec — R2 core refactor + folded v15a/b/c). If already live, waits for v15_exec.done.
# 3. Resolves LOFT_WAVE_BASE (origin/loft/v15-exec if ahead of origin/main, else origin/main).
# 4. Launches ALL 6 Phase-2 workers in parallel (staggered 25s to avoid concurrent git bursts;
#    compiles serialize via tools/lib/build_gate.sh and GPU runs serialize via tools/gpu_lock.sh / quiet_gpu.sh;
#    all workers receive the mandatory "write all code first, compile rarely" prompt from run.sh).
# 5. Once all 6 Phase-2 workers complete, runs iv15_2 (Astra) alone on a quiet GPU to merge, verify, and close Loft v1.5.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
DIR="$ROOT/agent_notes/2026-11-20_codex"
RUN="$DIR/run.sh"
export LOFT_MAX_STEPS="${LOFT_MAX_STEPS:-6}"

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

# Wait until prior waves + anti-freeze wrap-up finish
until [ -s "$DIR/final.finished" ] && [ -s "$DIR/wave15.finished" ] && [ -s "$DIR/ix4.done" ]; do
  sleep 30
done

# Phase 1: wait for live v15_exec if already running, or run it solo if not yet started
if [ ! -s "$DIR/v15_exec.done" ]; then
  if [ -s "$DIR/v15_exec.log" ]; then
    log "v15_exec already in progress; waiting for v15_exec.done before launching Phase 2"
    until [ -s "$DIR/v15_exec.done" ]; do
      # Guard against a dead/aborted v15_exec slot: if no active slot holds it and log is idle >10m, resume it
      sleep 30
    done
    log "observed v15_exec.done (exit=$(cat "$DIR/v15_exec.done"))"
  else
    step_sync v15_exec gpt-6.2-sol high
  fi
fi

# Resolve the canonical base ref for Phase 2 so every worker builds on v15_exec even if it pushed a branch without auto-merging
git -C "$ROOT" fetch origin --prune >/dev/null 2>&1 || true
BASE_REF="origin/main"
if git -C "$ROOT" rev-parse --verify origin/loft/v15-exec >/dev/null 2>&1; then
  ahead="$(git -C "$ROOT" rev-list --count origin/main..origin/loft/v15-exec 2>/dev/null || echo 0)"
  if [ "${ahead:-0}" -gt 0 ]; then
    BASE_REF="origin/loft/v15-exec"
  fi
fi
export LOFT_WAVE_BASE="$BASE_REF"
log "Phase 2 base ref resolved to: $LOFT_WAVE_BASE (LOFT_MAX_STEPS=$LOFT_MAX_STEPS)"

# Phase 2: launch ALL 6 conflict-free streams in parallel (staggered 25s apart to prevent concurrent git bursts)
phase2_pids=()
launch_bg() {
  local name="$1" model="$2" effort="$3"
  [ -s "$DIR/${name}.done" ] && { log "skipping already-done $name"; return 0; }
  until calm; do sleep 60; done
  log "launching Phase-2 worker: $name ($model/$effort) base=$LOFT_WAVE_BASE"
  "$RUN" "$name" "$model" "$effort" "$ROOT" &
  phase2_pids+=("$!")
  sleep 25
}

launch_bg v15_stdlib_finish gpt-6.2-sol high
launch_bg v15_perf          gpt-6.2-sol high
launch_bg v15_native        gpt-6.2-sol high
launch_bg cap1c             gpt-6.2-sol xhigh
launch_bg v15_bloat         gpt-6.2-sol medium
launch_bg cap1b             gpt-6.2-sol xhigh

log "all ${#phase2_pids[@]} Phase-2 workers launched; waiting for completion..."
for pid in "${phase2_pids[@]}"; do
  wait "$pid" || true
done
log "all Phase-2 workers finished"

# Phase 3: Wave v1.5-2 integrator (Astra) runs alone on a quiet GPU to merge, verify, and close Loft v1.5
unset LOFT_WAVE_BASE
step_sync iv15_2 gpt-6.2-astra high

echo done > "$DIR/wave15_2.finished"
log "wave15_2 FINISHED"
