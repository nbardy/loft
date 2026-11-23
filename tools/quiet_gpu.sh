#!/usr/bin/env bash
# Usage: tools/quiet_gpu.sh COMMAND [ARG ...] | --wait
# A timing window: the GPU with no build or simulation running beside it. agent-build grants the GPU
# and every other slot together or none of them, so the GPU is never held while this queues behind a
# build. Exit 69: the queue was busy and nothing ran. Exit 6: a build was seen during the window, so
# the measurement is void.
set -euo pipefail
root="$(cd "$(dirname "$0")" && pwd -P)"
if [[ ${1:-} != --locked ]]; then
  [[ $# -gt 0 ]] || { echo 'usage: quiet_gpu.sh COMMAND [ARG ...] | --wait' >&2; exit 2; }
  if ! command -v agent-build >/dev/null 2>&1; then
    echo 'quiet_gpu: agent-build is not installed, so only the build watch below guards this window' >&2
    LOFT_GPU_HOLDER=$$ exec "$root/quiet_gpu.sh" --locked "$@"
  fi
  exec agent-build --gpu --heavy --light --sim --max-minutes "${LOFT_QUIET_MINUTES:-30}" 'loft timing window' \
    "$root/quiet_gpu.sh" --locked "$@"
fi
shift
[[ ${LOFT_GPU_HOLDER:-} =~ ^[1-9][0-9]*$ ]] || { echo 'quiet_gpu: --locked is internal' >&2; exit 2; }
trap 'exit 143' HUP INT TERM

# The slots only cover jobs that ask for one. A cargo started directly is seen here instead.
running_builds() {
  if [[ $OSTYPE == darwin* ]]; then
    python3 "$root/gates/darwin_builds.py"
    return
  fi
  ps -ax -o stat= -o comm= | awk '$1 !~ /(^Z|E)/ && $NF ~ /(^|\/)(cargo|rustc)$/'
}
# Bounded: every slot is held from here on, so a long wait would stall every other agent.
for (( waited = 0; ; waited++ )); do
  builds="$(running_builds)"
  [[ -n "$builds" ]] || break
  if (( waited >= 60 )); then
    echo 'quiet_gpu: a build outside the shared slots is still running; nothing was timed' >&2
    printf '%s\n' "$builds" >&2
    exit 6
  fi
  sleep 1
done
if [[ $1 == --wait ]]; then exit 0; fi

# Discard any window where a build was observed, even if it finished before the timed command did.
interference="$(mktemp)"
trap 'kill ${monitor:+"$monitor"} 2>/dev/null || true; rm -f "$interference"' EXIT
(
  while kill -0 "$$" 2>/dev/null; do
    if ! builds="$(running_builds)"; then
      echo 'could not inspect build processes' >>"$interference"
      break
    fi
    if [[ -n "$builds" ]]; then
      printf '%s\n' "$builds" >>"$interference"
      break
    fi
    sleep 0.2
  done
) &
monitor=$!
status=0
LOFT_QUIET_GPU="$LOFT_GPU_HOLDER" "$@" || status=$?
kill "$monitor" 2>/dev/null || true
wait "$monitor" 2>/dev/null || true
running_builds >>"$interference" || echo "final build observation failed" >>"$interference"
if [[ -s "$interference" ]]; then
  echo 'quiet_gpu: build overlap invalidated the timing window' >&2
  cat "$interference" >&2
  exit 6
fi
exit "$status"
