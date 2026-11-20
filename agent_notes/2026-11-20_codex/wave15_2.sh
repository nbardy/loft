#!/usr/bin/env bash
# Wave v1.5-2 conductor: waits for wave15 and wave4, runs R2 v15_exec SOLO first,
# then fans out the remaining R1-R9 / cap1b-d lanes through the shared LOFT_MAX_STEPS pool,
# and finishes with iv15_2 (Astra).
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
DIR="$ROOT/agent_notes/2026-11-20_codex"
RUN="$DIR/run.sh"
caffeinate -i -w "$$" &

# Wait for both wave15 and wave4 to complete before starting R2's solo core refactor
while [ ! -f "$DIR/wave15.finished" ] || [ ! -f "$DIR/wave4.finished" ]; do
  sleep 20
done

# Phase 1: R2 requires ONE solo core-refactor owner before parallel work resumes
if [ ! -f "$DIR/v15_exec.done" ]; then
  "$RUN" v15_exec gpt-6.2-sol high "$ROOT"
fi

# Phase 2: Parallel R1-R9 + cap1b-d lanes (bounded by LOFT_MAX_STEPS inside run.sh)
PIDS=()
for lane in \
  "v15_sky:high" \
  "v15_fma:high" \
  "cap1b:high" \
  "cap1c:high" \
  "cap1d:high" \
  "v15_bloat:medium" \
  "v15_metalfx:high"
do
  name="${lane%%:*}"
  effort="${lane##*:}"
  if [ ! -f "$DIR/${name}.done" ]; then
    "$RUN" "$name" gpt-6.2-sol "$effort" "$ROOT" &
    PIDS+=($!)
    sleep 20
  fi
done

for p in "${PIDS[@]}"; do
  wait "$p" || true
done

# Phase 3: Wave v1.5-2 integrator (Astra)
if [ ! -f "$DIR/iv15_2.done" ]; then
  "$RUN" iv15_2 gpt-6.2-astra high "$ROOT"
fi

touch "$DIR/wave15_2.finished"
