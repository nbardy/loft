#!/usr/bin/env bash
# Usage: tools/gpu_lock.sh COMMAND [ARG ...]. The persistent guard serializes PID-file replacement.
set -euo pipefail
[[ $# -gt 0 ]] || { echo "usage: gpu_lock.sh COMMAND [ARG ...]" >&2; exit 2; }
owner=$$
slot=/tmp/loft-gpu.lock
# A full gate can invoke a focused gate while holding the same device.
if [[ ${LOFT_GPU_HOLDER:-} =~ ^[1-9][0-9]*$ ]] && [[ -r "$slot" ]] &&
   [[ $(cat "$slot") == "$LOFT_GPU_HOLDER" ]] && kill -0 "$LOFT_GPU_HOLDER" 2>/dev/null; then
  exec "$@"
fi
claim() {
  perl -MFcntl=:flock -e '
    my ($slot,$pid,$op)=@ARGV;
    open my $guard, ">>", "$slot.guard" or die $!;
    flock $guard, LOCK_EX or die $!;
    my $other;
    if (-e $slot) {
      open my $in, "<", $slot or die $!;
      chomp($other=<$in> // "");
    }
    if ($op eq "release") { unlink($slot) or die $! if defined($other) && $other eq $pid; exit }
    if (defined($other) && $other =~ /^[1-9][0-9]*$/ && (kill(0,$other) || $!{EPERM})) {exit 75}
    open my $out, ">", $slot or die $!;
    print $out "$pid\n"; close $out or die $!;
  ' "$slot" "$owner" "$1"
}
child=""
stop() {
  trap - HUP INT TERM
  if [[ -n "$child" ]]; then kill "$child" 2>/dev/null || true; wait "$child" 2>/dev/null || true; fi
  exit 143
}
trap 'claim release' EXIT
trap stop HUP INT TERM
until claim acquire; do code=$?; [[ $code == 75 ]] || exit "$code"; sleep 1; done
export LOFT_GPU_HOLDER="$owner"
"$@" <&0 &
child=$!
wait "$child"
