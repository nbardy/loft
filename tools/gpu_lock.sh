#!/usr/bin/env bash
# Usage: tools/gpu_lock.sh [--heavy|--light|--sim ...] COMMAND [ARG ...]
# One job on the GPU at a time, through agent-build's gpu slot. Name every other slot the command will
# need (`--heavy` for a command that builds) so they are granted together: a job that takes the GPU
# first and then queues for a build holds the GPU idle for as long as the build ahead of it runs.
# Exit 69: the queue was busy and nothing ran. Exit 75: stopped at the hold limit, or cancelled.
set -euo pipefail
classes=(--gpu)
while [[ ${1:-} == --heavy || ${1:-} == --light || ${1:-} == --sim ]]; do classes+=("$1"); shift; done
[[ $# -gt 0 ]] || { echo 'usage: gpu_lock.sh [--heavy|--light|--sim ...] COMMAND [ARG ...]' >&2; exit 2; }
if ! command -v agent-build >/dev/null 2>&1; then
  echo 'gpu_lock: agent-build is not installed, so this runs without the shared GPU slot' >&2
  exec "$@"
fi
exec agent-build "${classes[@]}" "$@"
