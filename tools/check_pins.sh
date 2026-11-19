#!/usr/bin/env bash
# Verify the wgpu and naga versions shared by Loft and a host-owned Keel device. Compare the full set of wgpu
# crates used by both workspaces. Keel runs the corresponding comparison from its side.
#   tools/check_pins.sh [KEEL_LOCK]     (default $KEEL_LOCK, else ~/git/room-runners-arena-lib/keel/Cargo.lock)
# Exit 0: equal. 1: a mismatch, or a crate resolved twice. 2: a lock is missing (named, with the fix).
set -euo pipefail
root="$(cd "$(dirname "$0")/.." && pwd -P)"
loft="$root/Cargo.lock"
keel="${1:-${KEEL_LOCK:-$HOME/git/room-runners-arena-lib/keel/Cargo.lock}}"
for lock in "$loft" "$keel"; do
  [[ -f "$lock" ]] || { echo "pins: no lock at $lock. Fix: pass Keel's Cargo.lock as the argument or set KEEL_LOCK" >&2; exit 2; }
done
# The versions a lock resolves for one crate, one per line.
versions() { awk -v n="$2" '$1 == "name" { hit = ($3 == "\"" n "\"") } hit && $1 == "version" { gsub(/"/, "", $3); print $3; hit = 0 }' "$1" | sort -u; }
status=0
for crate in wgpu wgpu-core wgpu-hal wgpu-types naga; do
  l="$(versions "$loft" "$crate")" k="$(versions "$keel" "$crate")"
  for side in "loft:$l" "keel:$k"; do
    [[ "$(wc -w <<<"${side#*:}")" -eq 1 ]] || { echo "pins: $crate resolves to '${side#*:}' in ${side%%:*}'s lock; it must resolve once" >&2; status=1; }
  done
  [[ "$l" == "$k" ]] || {
    echo "pins: $crate is $l in Loft ($loft) and $k in Keel ($keel). Fix: bump both repos to one version in one change" >&2
    status=1
  }
done
[[ $status -ne 0 ]] || echo "pins: wgpu $(versions "$loft" wgpu), naga $(versions "$loft" naga), equal to Keel"
exit $status
