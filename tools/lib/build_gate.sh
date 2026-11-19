#!/usr/bin/env bash
# Copied verbatim from Keel (room-runners-arena-lib keel-complete b82d8b8c, keel/scripts/lib/build_gate.sh), plan 2026-11-19 §6.
# Loft and Keel take the SAME lock files (~/Library/Caches/keel-build-gate/slot.0 and slot.light) and the same
# KEEL_GATE_HELD export, so a Loft build waits behind a Keel gate instead of fighting it. Change it in Keel first.
# Machine-wide compile locks: Cargo entry points in keel/scripts take one of two locks by job size.
#
#   HEAVY (slot.0)      check.sh quick/full, warm.sh, dev.sh build/run, build-web.sh, build-worker.sh, check-worker.sh,
#                       check-seeded-lane.sh, `build_gate.sh run`. One holder per machine, full -j (CARGO_BUILD_JOBS
#                       defaults to ncpu). Same file and KEEL_GATE_HELD form ("PID:0") as the 2026-11-01 single lock,
#                       so lanes that have not rebased still exclude against it.
#   LIGHT (slot.light)  single-crate `dev.sh check|lint|test`. One holder per machine, CARGO_BUILD_JOBS defaults to 4
#                       and the caller is reniced (+10), so a crate check never waits behind a 20-minute full gate.
#   => at most two compiles per machine: one heavy at full -j plus one light at 4 jobs.
#   SIM (slot.sim)      not a compile lock and not taken here: `keel_cli tidewar` takes it itself (std File::lock, the
#                       same flock(2)) while it plays matches, so one simulation run plays per machine (decision #115).
#                       Independent of heavy and light. Only `status` reads it.
#
# Why two (owner request 2026-11-03): with one lock, an agent's 10 s `dev.sh check game_bships` queued behind another
# lane's full gate for 10-20 minutes. Why not more: 2026-11-01 many concurrent compiles drove load to 60-200 and a small
# release rebuild took 8 min wall for 54 s CPU; a bounded light lane (4 jobs, niced) costs the heavy one little.
# A waiter prints who holds its lock, then waits; it never fails.
#
#   source "$keel/scripts/lib/build_gate.sh"; keel_gate_acquire "check.sh full"         # heavy
#   source "$keel/scripts/lib/build_gate.sh"; keel_gate_acquire_light "dev.sh check x"  # light
#   build_gate.sh run CMD [ARG ...]   take the HEAVY lock, then exec CMD (direct cargo). Compiles only: long
#                                     simulations and browser audits run outside it under `nice -n 15` (2026-11-02:
#                                     a 70-minute balance batch holding the lock stalled every lane's compile).
#   build_gate.sh status              who holds each lock
# KEEL_BUILD_GATE=off skips both (a machine of your own, e.g. a cloud container); KEEL_GATE_DIR moves the state (tests).
#
# Nesting: a script started under the HEAVY lock (gallery.sh -> dev.sh, warm.sh -> dev.sh) sees a live KEEL_GATE_HELD
# and takes neither lock again. Under the LIGHT lock only a light request is free; a heavy request (a test process
# that runs `dev.sh build` or `check.sh`) waits for the heavy slot like any other caller (2026-11-04 INFRA review: it
# used to skip it and compile at full size beside the real heavy holder, three compiles on the machine). No deadlock:
# a heavy holder never waits for the light slot. Two holders in the SAME worktree still serialize on Cargo's own
# build-dir lock; the light lane helps across worktrees.
#
# keel_gate_release ends a lock this process took (not a parent's): `dev.sh run` builds under the heavy lock, releases
# it, then execs the program, so a long run never holds the lock (2026-11-02 incident above).
#
# Why a holder process: the lock must survive `exec cargo` and end with the caller. A lock fd handed down to cargo
# would also be inherited by the sccache server cargo starts, which outlives the build and would hold the lock
# forever. So a small perl process takes flock(2) and keeps it while the caller's PID ($$) lives; the kernel drops
# it when the holder exits, also after kill -9 of the caller, so there is never a stale lock to clean up. macOS has
# no flock(1); perl's flock is in the base system.
#
# State: ~/Library/Caches/keel-build-gate (Linux: ~/.cache/keel-build-gate): slot.0, slot.light and slot.sim (each
# + .info).

keel_gate_dir() {
  if [[ -n "${KEEL_GATE_DIR:-}" ]]; then
    printf '%s\n' "$KEEL_GATE_DIR"
  elif [[ "$(uname -s)" == Darwin ]]; then
    printf '%s\n' "$HOME/Library/Caches/keel-build-gate"
  else
    printf '%s\n' "${XDG_CACHE_HOME:-$HOME/.cache}/keel-build-gate"
  fi
}

# keel_gate_held SLOTS: true when KEEL_GATE_HELD ("PID:0" heavy, "PID:light" light) names one of SLOTS (a regex
# alternation: "0" or "0|light") and a live process that its slot's info file says is the holder (not a recycled PID
# or a stale export).
keel_gate_held() {
  [[ "${KEEL_GATE_HELD:-}" =~ ^([0-9]+):($1)$ ]] || return 1
  kill -0 "${BASH_REMATCH[1]}" 2>/dev/null || return 1
  [[ "$(cat "$(keel_gate_dir)/slot.${BASH_REMATCH[2]}.info" 2>/dev/null)" == "holder=${BASH_REMATCH[1]} "* ]]
}

# The holder. Args: DIR SLOT OWNER_PID LABEL. Prints "PID WAITED_SECONDS" once it holds slot SLOT, then keeps it while
# OWNER_PID lives. The waiting notice goes to stderr.
keel_gate_holder_perl='
use strict; use warnings;
use Fcntl qw(:flock);
use Time::HiRes qw(sleep time);
my ($dir, $slot, $owner, $label) = @ARGV;
$label =~ s/[\r\n]+/ /g;
sub alive { return kill(0, $_[0]) || $!{EPERM}; }
mkdir $dir;
my $file = "$dir/slot.$slot";
open(my $lock, ">>", $file) or die "keel gate: $file: $!\n";
my $start = time;
if (!flock($lock, LOCK_EX | LOCK_NB)) {
  my $info = "";
  if (open(my $i, "<", "$file.info")) { $info = <$i> // ""; chomp $info; close $i; }
  my $kind = $slot eq "0" ? "heavy" : $slot;
  print STDERR "keel gate: waiting for the $kind compile lock ($label); held by $info\n";
  flock($lock, LOCK_EX) or die "keel gate: lock: $!\n";
}
exit 0 unless alive($owner);
open(my $i, ">", "$file.info.$$") or die "keel gate: info: $!\n";
printf $i "holder=%d owner=%d since=%d label=%s\n", $$, $owner, time, $label;
close $i;
rename("$file.info.$$", "$file.info");
printf STDOUT "%d %.1f\n", $$, time - $start;
close STDOUT;
# Keep no pipe of the caller open: `out=$(dev.sh ... 2>&1)` must not wait for the holder.
open(STDERR, ">", "/dev/null");
sleep 0.25 while alive($owner);
'

# keel_gate_take SLOT LABEL: block until this process holds SLOT (0 or light) or any lock is already held above it.
# Prints the holder PID on success, nothing if gating is off/nested/unavailable. Never fails: if the holder cannot
# start (no perl), it says so loudly and the build proceeds ungated.
keel_gate_take() {
  local slot="$1" label="$2" tmp reply
  tmp="$(mktemp -d "${TMPDIR:-/tmp}/keel-gate.XXXXXX")"
  mkfifo "$tmp/fifo"
  # Background, watching this shell's PID ($$, which `exec cargo` keeps), not this function.
  perl -e "$keel_gate_holder_perl" "$(keel_gate_dir)" "$slot" "$$" "$label" >"$tmp/fifo" &
  reply=""
  read -r reply <"$tmp/fifo" || true
  rm -rf "$tmp"
  if [[ ! "$reply" =~ ^([0-9]+)\ ([0-9.]+)$ ]]; then
    echo "keel gate: UNAVAILABLE (holder failed to start: perl missing or $(keel_gate_dir) unwritable); building ungated" >&2
    return 1
  fi
  export KEEL_GATE_HELD="${BASH_REMATCH[1]}:$slot"
  KEEL_GATE_WAITED="${BASH_REMATCH[2]}"
  # Not exported: only this shell may end the holder it started.
  keel_gate_taken="${BASH_REMATCH[1]}"
}

# keel_gate_release: end the holder this shell started (nothing if gating was off, nested or unavailable) and drop the
# export, so the process that runs next acquires for itself. The kernel frees the flock when the holder exits.
keel_gate_release() {
  [[ -n "${keel_gate_taken:-}" ]] || return 0
  kill "$keel_gate_taken" 2>/dev/null || true
  # The holder is this shell's background child: wait reaps it (kill -0 would still see a zombie as alive).
  wait "$keel_gate_taken" 2>/dev/null || true
  unset KEEL_GATE_HELD keel_gate_taken
}

# keel_gate_acquire [LABEL]: the HEAVY lock, full -j. Unchanged contract since 2026-11-01.
keel_gate_acquire() {
  local label="${1:-$(basename "$0") $PWD}"
  [[ "${KEEL_BUILD_GATE:-on}" != off ]] || return 0
  ! keel_gate_held 0 || return 0
  keel_gate_take 0 "$label" || return 0
  export CARGO_BUILD_JOBS="${CARGO_BUILD_JOBS:-$(sysctl -n hw.ncpu 2>/dev/null || nproc)}"
  echo "keel gate: heavy compile lock for $label (waited ${KEEL_GATE_WAITED}s; CARGO_BUILD_JOBS=$CARGO_BUILD_JOBS)" >&2
}

# keel_gate_acquire_light [LABEL]: the LIGHT lock for a single-crate check/lint/test: 4 jobs unless set, reniced +10.
keel_gate_acquire_light() {
  local label="${1:-$(basename "$0") $PWD}"
  [[ "${KEEL_BUILD_GATE:-on}" != off ]] || return 0
  ! keel_gate_held '0|light' || return 0
  keel_gate_take light "$label" || return 0
  export CARGO_BUILD_JOBS="${CARGO_BUILD_JOBS:-4}"
  # Lower this shell's priority; `exec cargo` keeps the PID and its children inherit the niceness.
  renice -n 10 -p $$ >/dev/null 2>&1 || echo "keel gate: renice failed; light build runs at normal priority" >&2
  echo "keel gate: light compile lock for $label (waited ${KEEL_GATE_WAITED}s; CARGO_BUILD_JOBS=$CARGO_BUILD_JOBS, niced)" >&2
}

keel_gate_status() {
  local slot kind info holder
  for slot in 0 light sim; do
    case "$slot" in
      0) kind=heavy ;;
      light) kind=light ;;
      sim) kind=sim ;;
    esac
    info="$(cat "$(keel_gate_dir)/slot.$slot.info" 2>/dev/null || true)"
    holder="$(sed -n 's/^holder=\([0-9]*\) .*/\1/p' <<<"$info")"
    if [[ -n "$holder" ]] && kill -0 "$holder" 2>/dev/null; then
      echo "keel gate: $kind held  $info"
    else
      echo "keel gate: $kind free"
    fi
  done
}

if [[ "${BASH_SOURCE[0]}" == "$0" ]]; then
  set -euo pipefail
  case "${1:-}" in
    status) keel_gate_status ;;
    run)
      shift
      [[ $# -ge 1 ]] || { echo 'Usage: build_gate.sh run CMD [ARG ...]' >&2; exit 2; }
      keel_gate_acquire "run $*"
      exec "$@"
      ;;
    *) echo 'Usage: build_gate.sh status | build_gate.sh run CMD [ARG ...]   (or source it: keel_gate_acquire[_light] LABEL)' >&2; exit 2 ;;
  esac
fi
