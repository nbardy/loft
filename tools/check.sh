#!/usr/bin/env bash
# Gates for the root Loft v1 workspace:
#   tools/check.sh static  every check that compiles nothing, concurrently, buffered, with per-job seconds
#   tools/check.sh quick   static, then only what the change set can break (the plan below), under the HEAVY lock
#   tools/check.sh plan    print quick's plan and run nothing
#   tools/check.sh full    the integrator's, once per wave: static, Clippy and nextest on the workspace, and doctests
# Change set (quick, plan): paths changed since merge-base(HEAD, LOFT_BASE, default origin/main), plus staged,
# unstaged and untracked files. A missing base is an error, never an empty or whole-tree selection.
set -euo pipefail
root="$(cd "$(dirname "$0")/.." && pwd -P)"
cd "$root"
source tools/lib/cache-env.sh
source tools/lib/gate_flags.sh
source tools/lib/build_gate.sh
toolchain="$(sed -n 's/^channel = "\([^"]*\)"/\1/p' rust-toolchain.toml)"
cargo() { rustup run "$toolchain" cargo "$@"; }
now() { perl -MTime::HiRes=time -e 'printf "%.3f\n", time'; }
members() { cargo metadata --locked --no-deps --format-version 1 | perl -MJSON::PP -0ne 'print "$_->{name}\n" for @{decode_json($_)->{packages}}'; }
# deps CRATE: every package CRATE reaches through normal and build edges, one name per line.
deps() { cargo tree --locked -p "$1" -e normal,build --prefix none --format '{p}' | awk '{print $1}' | sort -u; }

# The runner owns the device. The CLI also reaches wgpu for the GPU measurements.
check_layers() {
  local workspace_members member reach status=0
  workspace_members="$(members)" || return 1
  for member in $workspace_members; do
    reach="$(deps "$member")" || return 1
    case "$member" in
      loft-runner|loft-cli) ;;
      *) ! grep -qx wgpu <<<"$reach" || { echo "layers: $member reaches wgpu; only the runner and CLI may" >&2; status=1; } ;;
    esac
    if [[ "$member" == loft-runner ]] && grep -qx loft-passes <<<"$reach"; then
      echo "layers: loft-runner reaches loft-passes; loading a program must not require the compiler" >&2
      status=1
    fi
  done
  return "$status"
}

# Cargo links each file or subdirectory with a main.rs under tests/ as a separate binary.
check_test_layout() {
  local dir file count status=0
  for dir in crates/*/tests; do
    [[ -d "$dir" ]] || continue
    count=0
    for file in "$dir"/*.rs "$dir"/*/main.rs; do
      [[ -f "$file" ]] || continue
      count=$((count + 1))
    done
    if (( count > 1 )); then
      echo "test layout: $dir has $count integration binaries; use one with modules" >&2
      status=1
    fi
  done
  return "$status"
}

STATIC_TMP="" STATIC_PIDS=() STATIC_NAMES=()
static_cleanup() {
  [[ ${#STATIC_PIDS[@]} -eq 0 ]] || kill "${STATIC_PIDS[@]}" 2>/dev/null || true
  [[ -z "$STATIC_TMP" ]] || rm -rf "$STATIC_TMP"
}
# static_job NAME CMD...: one check in the background; output in NAME.out, "exit start end" in NAME.rc.
static_job() {
  local name="$1"
  shift
  STATIC_NAMES+=("$name")
  ( start="$(now)"; code=0; "$@" >"$STATIC_TMP/$name.out" 2>&1 || code=$?; echo "$code $start $(now)" >"$STATIC_TMP/$name.rc" ) &
  STATIC_PIDS+=($!)
}

static_checks() {
  local wall_start pid name code start end line summed=0 failed=()
  STATIC_TMP="$(mktemp -d "${TMPDIR:-/tmp}/loft-static.XXXXXX")"
  trap static_cleanup EXIT
  wall_start="$(now)"
  static_job layers check_layers
  static_job test-layout check_test_layout
  static_job sh-syntax bash -c 'for f in tools/*.sh tools/lib/*.sh; do bash -n "$f" || exit 1; done'
  # Exit 2 (no Keel checkout on this machine) is a named skip, printed with its reason; a mismatch (1) fails.
  static_job pins bash -c 'tools/check_pins.sh && exit 0; c=$?; [[ $c -eq 2 ]] && { echo "pins: SKIPPED (no Keel lock)"; exit 0; }; exit $c'
  for pid in "${STATIC_PIDS[@]}"; do wait "$pid" || true; done
  STATIC_PIDS=()
  # Passing jobs first, failing jobs last (their output is the part to read), then the verdict.
  for pass in ok fail; do
    for name in "${STATIC_NAMES[@]}"; do
      read -r code start end <"$STATIC_TMP/$name.rc"
      [[ ("$pass" == ok && "$code" == 0) || ("$pass" == fail && "$code" != 0) ]] || continue
      line="$(awk -v s="$start" -v e="$end" 'BEGIN { printf "%.1f", e - s }')"
      summed="$(awk -v a="$summed" -v b="$line" 'BEGIN { printf "%.1f", a + b }')"
      case "$code" in
        0) echo "static ok     ${line}s  $name"; [[ ! -s "$STATIC_TMP/$name.out" ]] || sed 's/^/    /' "$STATIC_TMP/$name.out" ;;
        *) failed+=("$name"); echo "static FAILED ${line}s  $name (exit $code)" >&2; sed 's/^/    /' "$STATIC_TMP/$name.out" >&2 ;;
      esac
    done
  done
  echo "loft check static: ${#STATIC_NAMES[@]} jobs, wall $(awk -v s="$wall_start" -v e="$(now)" 'BEGIN { printf "%.1f", e - s }')s (serial sum ${summed}s)"
  static_cleanup
  trap - EXIT
  [[ ${#failed[@]} -eq 0 ]] || { echo "loft check static: FAILED: ${failed[*]}" >&2; exit 1; }
}

# Workspace-wide inputs select the full suite. Crate edits select their reverse dependencies.
plan() {
  local base path member changed untracked workspace_members reach crates=()
  local inputs=(Cargo.toml Cargo.lock rust-toolchain.toml .cargo .config crates lib engines
    tools/check.sh tools/check_pins.sh tools/dev.sh tools/lib)
  base="$(git merge-base HEAD "${LOFT_BASE:-origin/main}")" || {
    echo "loft check: no merge base with ${LOFT_BASE:-origin/main}; fetch it or set LOFT_BASE" >&2
    exit 2
  }
  # Compare paths independently so moving a file out of the workspace still checks its former callers.
  changed="$(git diff --name-only --no-renames "$base" -- "${inputs[@]}")"
  untracked="$(git ls-files --others --exclude-standard -- "${inputs[@]}")"
  PLAN_FULL="" PLAN_CRATES=()
  while IFS= read -r path; do
    case "$path" in
      "") ;;
      crates/*/Cargo.toml) PLAN_FULL="${PLAN_FULL:-$path changes the crate graph}" ;;
      crates/*/*) path="${path#crates/}"; crates+=("${path%%/*}") ;;
      *) PLAN_FULL="${PLAN_FULL:-$path is a workspace-wide input}" ;;
    esac
  done < <(printf '%s\n' "$changed" "$untracked" | sort -u)
  if [[ -z "$PLAN_FULL" && -n "${crates[*]:-}" ]]; then
    workspace_members="$(members)"
    for member in $workspace_members; do
      reach="$(deps "$member")"
      for path in "${crates[@]}"; do
        ! grep -qxF "$path" <<<"$reach" || { PLAN_CRATES+=("$member"); break; }
      done
    done
  fi
  echo "loft plan: base $base"
  [[ -z "$PLAN_FULL" ]] || echo "loft plan: FULL gate: $PLAN_FULL"
  echo "loft plan: crates: ${PLAN_CRATES[*]:-none}"
}

full_gate() {
  keel_gate_acquire "loft check.sh full ($root)"
  cargo clippy "${LOFT_GATE_CLIPPY_ARGS[@]}" --workspace -- "${LOFT_GATE_LINT_ARGS[@]}"
  cargo nextest run "${LOFT_GATE_TEST_ARGS[@]}" --workspace
  cargo test "${LOFT_GATE_DOCTEST_ARGS[@]}" --workspace
  keel_gate_release
  echo "loft check full: ok"
}

quick_gate() {
  local scope=() crate
  [[ -z "$PLAN_FULL" ]] || { full_gate; return; }
  for crate in ${PLAN_CRATES[@]+"${PLAN_CRATES[@]}"}; do scope+=(-p "$crate"); done
  if [[ -n "${scope[*]:-}" ]]; then
    keel_gate_acquire "loft check.sh quick ($root)"
    cargo clippy "${LOFT_GATE_CLIPPY_ARGS[@]}" "${scope[@]}" -- "${LOFT_GATE_LINT_ARGS[@]}"
    cargo nextest run "${LOFT_GATE_TEST_ARGS[@]}" "${scope[@]}"
    keel_gate_release
  fi
  echo "loft check quick: ok"
}

case "${1:-}" in
  static) static_checks ;;
  plan) plan ;;
  quick) static_checks; plan; quick_gate ;;
  full) static_checks; full_gate ;;
  *) echo 'Usage: tools/check.sh static|quick|plan|full' >&2; exit 2 ;;
esac
