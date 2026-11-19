#!/usr/bin/env bash
# The Loft gate (plan 2026-11-19 §6, after Keel's keel/scripts/check.sh):
#   tools/check.sh static  every check that compiles nothing, concurrently, buffered, with per-job seconds
#   tools/check.sh quick   static, then only what the change set can break (the plan below), under the HEAVY lock
#   tools/check.sh plan    print quick's plan and run nothing
#   tools/check.sh full    the integrator's, once per wave: static, clippy and nextest on the workspace, doctests, Bun
# Change set (quick, plan): paths changed since merge-base(HEAD, LOFT_BASE, default origin/main), plus staged,
# unstaged and untracked files. A missing base is an error, never an empty or whole-tree selection.
set -euo pipefail
root="$(cd "$(dirname "$0")/.." && pwd -P)"
cd "$root"
source tools/lib/cache-env.sh
source tools/lib/gate_flags.sh
source tools/lib/build_gate.sh
toolchain="$(sed -n 's/^channel = "\([^"]*\)"/\1/p' rust/rust-toolchain.toml)"
cargo() { (cd "$root/rust" && rustup run "$toolchain" cargo "$@"); }
now() { perl -MTime::HiRes=time -e 'printf "%.3f\n", time'; }
members() { cargo metadata --locked --no-deps --format-version 1 | perl -MJSON::PP -0ne 'print "$_->{name}\n" for @{decode_json($_)->{packages}}'; }
# deps CRATE: every package CRATE reaches through normal and build edges, one name per line.
deps() { cargo tree --locked -p "$1" -e normal,build --prefix none --format '{p}' | awk '{print $1}' | sort -u; }

# Layering (plan §1.2, §5.2): the compiler never links wgpu and a game loads bundles without the compiler.
# loft-loader is the default executor (loft-view, CORE.md "Where we are"); loft-gpu is the Bundle V1 host.
check_layers() {
  local member reach status=0
  for member in $(members); do
    reach="$(deps "$member")"
    case "$member" in
      loft-gpu|loft-draw|loft-web|loft-loader) ;;
      *) ! grep -qx wgpu <<<"$reach" || { echo "layers: $member reaches wgpu; only loft-gpu, loft-draw and loft-web may" >&2; status=1; } ;;
    esac
    case "$member" in
      loft-gpu) ! grep -qx loft-core <<<"$reach" || { echo "layers: loft-gpu reaches loft-core; a game must load bundles without the compiler" >&2; status=1; } ;;
      *) ;;
    esac
  done
  return "$status"
}

# One integration-test binary per crate (Keel 2026-10-30: 70 test executables, 309 MB, ~15 s per link): tests live as
# modules of tests/it/main.rs. A stray tests/*.rs becomes its own executable; an undeclared module is never compiled.
check_test_layout() {
  local status=0 file name
  for file in rust/crates/*/tests/*; do
    [[ -e "$file" && "$file" != */tests/it ]] || continue
    echo "test layout: $file is outside tests/it/; make it a module of tests/it/main.rs" >&2
    status=1
  done
  for file in rust/crates/*/tests/it/*; do
    [[ -e "$file" && "$file" != */main.rs ]] || continue
    name="$(basename "$file" .rs)"
    grep -Eq "^\s*(pub(\([a-z]+\))?\s+)?mod\s+$name\s*;" "${file%/*}/main.rs" || {
      echo "test layout: $file is never compiled; add \`mod $name;\` to ${file%/*}/main.rs" >&2
      status=1
    }
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
  local wall_start member pid name code start end line summed=0 failed=()
  STATIC_TMP="$(mktemp -d "${TMPDIR:-/tmp}/loft-static.XXXXXX")"
  trap static_cleanup EXIT
  wall_start="$(now)"
  static_job tsc node_modules/.bin/tsc --noEmit
  static_job loader-closure bun tools/check_loader_closure.ts
  static_job layers check_layers
  static_job test-layout check_test_layout
  static_job sh-syntax bash -c 'for f in tools/*.sh tools/lib/*.sh; do bash -n "$f" || exit 1; done'
  # Exit 2 (no Keel checkout on this machine) is a named skip, printed with its reason; a mismatch (1) fails.
  static_job pins bash -c 'tools/check_pins.sh && exit 0; c=$?; [[ $c -eq 2 ]] && { echo "pins: SKIPPED (no Keel lock)"; exit 0; }; exit $c'
  # rustfmt one member per job (Keel 2026-11-01: --all as one job was the critical path, 12.0 s against 3.1 s sharded).
  for member in $(members); do static_job "fmt-$member" cargo fmt --check -p "$member"; done
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

# The plan: each changed path is classified once into one kind. Sets PLAN_FULL (the reason, or empty), PLAN_CRATES
# (changed members plus their reverse dependencies) and PLAN_BUN (test files). Unmapped paths are printed.
plan() {
  local base path test member crates=() unmapped=()
  base="$(git merge-base HEAD "${LOFT_BASE:-origin/main}")" || {
    echo "loft check: no merge base with ${LOFT_BASE:-origin/main}; fetch it or set LOFT_BASE" >&2
    exit 2
  }
  PLAN_FULL="" PLAN_CRATES=() PLAN_BUN=()
  while IFS= read -r path; do
    # Top-level files by name: a case `*` also matches `/`, so `rust/*.toml` caught every crate's Cargo.toml.
    case "$path" in
      rust/Cargo.toml|rust/Cargo.lock|rust/rust-toolchain.toml|rust/rustfmt.toml|rust/.config/*|package.json|pnpm-lock.yaml|bunfig.toml|tsconfig.json|tests/setup.ts|tools/lib/*)
        PLAN_FULL="${PLAN_FULL:-$path is a workspace-wide input}" ;;
      rust/crates/*) crates+=("$(cut -d/ -f3 <<<"$path")") ;;
      tests/*.test.ts) PLAN_BUN+=("$path") ;;
      src/*|tests/*)
        # Test files that import the changed module by its path (direct importers; the full gate covers the rest).
        while IFS= read -r test; do PLAN_BUN+=("$test"); done < <(grep -lF "/$(sed "s/\.ts$//" <<<"${path#*/}")" tests/*.test.ts || true) ;;
      docs/*|agent_notes/*|key_learnings/*|prompts/*|*.md|tools/*.sh|.cambium.toml) ;;
      *) unmapped+=("$path") ;;
    esac
  done < <({ git diff --name-only "$base"; git ls-files --others --exclude-standard; } | sort -u)
  for member in $(members); do
    for path in ${crates[@]+"${crates[@]}"}; do
      ! deps "$member" | grep -qx "$path" || { PLAN_CRATES+=("$member"); break; }
    done
  done
  PLAN_BUN=($(printf '%s\n' ${PLAN_BUN[@]+"${PLAN_BUN[@]}"} | sort -u))
  echo "loft plan: base $base"
  [[ -z "$PLAN_FULL" ]] || echo "loft plan: FULL gate: $PLAN_FULL"
  echo "loft plan: crates: ${PLAN_CRATES[*]:-none}"
  echo "loft plan: bun: ${PLAN_BUN[*]:-none}"
  for path in ${unmapped[@]+"${unmapped[@]}"}; do echo "loft plan: unmapped (no check selected): $path"; done
}

full_gate() {
  keel_gate_acquire "loft check.sh full ($root)"
  cargo clippy "${LOFT_GATE_CLIPPY_ARGS[@]}" --workspace -- "${LOFT_GATE_LINT_ARGS[@]}"
  cargo nextest run "${LOFT_GATE_TEST_ARGS[@]}" --workspace
  cargo test "${LOFT_GATE_DOCTEST_ARGS[@]}" --workspace
  keel_gate_release
  # Outside the compile lock, niced: the Bun suite renders on the GPU (tests/setup.ts admits one per user; exit 75).
  nice -n 15 bun test --timeout 120000
  echo "loft check full: ok"
}

quick_gate() {
  local scope=() crate
  [[ -z "$PLAN_FULL" ]] || { full_gate; return; }
  for crate in ${PLAN_CRATES[@]+"${PLAN_CRATES[@]}"}; do scope+=(-p "$crate"); done
  if [[ ${#scope[@]} -gt 0 ]]; then
    keel_gate_acquire "loft check.sh quick ($root)"
    cargo clippy "${LOFT_GATE_CLIPPY_ARGS[@]}" "${scope[@]}" -- "${LOFT_GATE_LINT_ARGS[@]}"
    cargo nextest run "${LOFT_GATE_TEST_ARGS[@]}" "${scope[@]}"
    keel_gate_release
  fi
  [[ ${#PLAN_BUN[@]} -eq 0 ]] || nice -n 15 bun test --timeout 120000 "${PLAN_BUN[@]/#/./}"
  echo "loft check quick: ok"
}

case "${1:-}" in
  static) static_checks ;;
  plan) plan ;;
  quick) static_checks; plan; quick_gate ;;
  full) static_checks; full_gate ;;
  *) echo 'Usage: tools/check.sh static|quick|plan|full' >&2; exit 2 ;;
esac
