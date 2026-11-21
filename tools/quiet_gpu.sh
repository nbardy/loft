#!/usr/bin/env bash
# Reserved timing window: GPU first, then both shared compile slots.
set -euo pipefail
root="$(cd "$(dirname "$0")" && pwd -P)"
if [[ ${1:-} != --locked ]]; then
  [[ $# -gt 0 ]] || { echo 'usage: quiet_gpu.sh COMMAND [ARG ...] | --wait' >&2; exit 2; }
  exec "$root/gpu_lock.sh" "$root/quiet_gpu.sh" --locked "$@"
fi
shift
[[ -n ${LOFT_GPU_HOLDER:-} && -r /tmp/loft-gpu.lock &&
   $(cat /tmp/loft-gpu.lock) == "$LOFT_GPU_HOLDER" ]] || exit 2
source "$root/lib/build_gate.sh"
trap 'exit 143' HUP INT TERM
if keel_gate_held '0|light'; then
  echo 'quiet_gpu: finish the build before requesting a timing window' >&2
  exit 2
fi

# Claim both slots together. A light build can need the heavy slot, so
# release heavy immediately if light is busy. Bash keeps the inherited flocks.
dir="$(keel_gate_dir)"
mkdir -p "$dir"
exec 8>>"$dir/slot.0" 9>>"$dir/slot.light"
claim_build_slots() {
  perl -MFcntl=:flock -e '
    open my $heavy, ">&=", 8 or die $!;
    open my $light, ">&=", 9 or die $!;
    flock $heavy, LOCK_EX | LOCK_NB or do { exit 75 if $!{EWOULDBLOCK}; die $! };
    flock $light, LOCK_EX | LOCK_NB or do {
      my ($busy, $error) = ($!{EWOULDBLOCK}, "$!");
      flock $heavy, LOCK_UN or die $!;
      exit 75 if $busy;
      die $error;
    };
  '
}
until claim_build_slots; do
  code=$?
  [[ $code == 75 ]] || exit "$code"
  sleep 1
done
for slot in 0 light; do
  printf 'holder=%s owner=%s label=loft quiet GPU\n' "$$" "$$" >"$dir/slot.$slot.info"
done
unset KEEL_GATE_HELD
running_builds() {
  ps -ax -o stat= -o comm= | awk '$1 !~ /(^Z|E)/ && $NF ~ /(^|\/)(cargo|rustc)$/'
}
while :; do
  builds="$(running_builds)"
  [[ -n "$builds" ]] || break
  sleep 1
done
if [[ $1 == --wait ]]; then exit 0; fi

# Direct cargo invocations can bypass the shared slots. Discard any window
# where one was observed, even if it finished before the timed command did.
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
) 8>&- 9>&- &
monitor=$!
status=0
LOFT_QUIET_GPU="$LOFT_GPU_HOLDER" "$@" 8>&- 9>&- || status=$?
kill "$monitor" 2>/dev/null || true
wait "$monitor" 2>/dev/null || true
running_builds >>"$interference"
if [[ -s "$interference" ]]; then
  echo 'quiet_gpu: build overlap invalidated the timing window' >&2
  cat "$interference" >&2
  exit 6
fi
exit "$status"
