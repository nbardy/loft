#!/usr/bin/env bash
# Integrator checks for the workspace in Cargo.toml.
set -euo pipefail
root="$(cd "$(dirname "$0")/.." && pwd -P)"
cd "$root"
source tools/lib/cache-env.sh

static_checks() {
  local script crate reach
  for script in tools/*.sh tools/lib/*.sh tools/parity/d1d/*.sh; do
    case "$(head -1 "$script")" in *zsh) zsh -n "$script" ;; *) bash -n "$script" ;; esac
  done
  # A host can load bundles without a compiler, and the compiler can run without a device.
  for crate in loft-runner loft-std loft-passes loft-plan loft-parse loft-abi loft-ir loft-format; do
    reach="$(cargo tree --locked -p "$crate" --edges normal,build --prefix none --format '{p}' | awk '{print $1}')"
    case "$crate" in
      loft-runner) if grep -qx loft-passes <<<"$reach"; then echo "runner links compiler" >&2; return 1; fi ;;
      *) if grep -qx wgpu <<<"$reach"; then echo "$crate links wgpu" >&2; return 1; fi ;;
    esac
  done
  tools/check_core_words.sh
  if ! test -f "${KEEL_LOCK:-$HOME/git/room-runners-arena-lib/keel/Cargo.lock}"; then
    echo 'pins: skipped (no host lock available)'
  else
    tools/check_pins.sh
  fi
  uv run --no-project python tools/check_corpus.py
  uv run --script tools/check_loft_grammar.py
}

# Quick checks cover the workspace if a shared input changes. Otherwise check
# each edited member and its reverse dependencies. Git reports both sides of moves.
plan() {
  local base paths changed crate graph
  base="$(git merge-base HEAD "${LOFT_BASE:-origin/main}")"
  paths=(Cargo.toml Cargo.lock rust-toolchain.toml .cargo .config crates corpus stdlib engines scenes
         grammar docs/spec/loft.lark AGENT_CARD.md tests tools agent_notes/2026-11-20_meta/v1_syntax)
  changed="$(git diff --no-renames --name-only "$base" -- "${paths[@]}")"
  changed+=$'\n'"$(git ls-files --others --exclude-standard -- "${paths[@]}")"
  scope=()
  while IFS= read -r path; do
    case "$path" in
      '') continue ;;
      crates/*/Cargo.toml) scope=(--workspace); break ;;
      crates/*/*)
        crate=${path#crates/}; crate=${crate%%/*}
        if [[ ! -f "crates/$crate/Cargo.toml" ]]; then scope=(--workspace); break; fi
        graph="$(cargo tree --locked --workspace --invert "$crate" --edges normal,build,dev --prefix none --format '{p}')"
        while read -r member _; do
          [[ ! -f "crates/$member/Cargo.toml" ]] || scope+=("$member")
        done <<< "$graph" ;;
      *) scope=(--workspace); break ;;
    esac
  done < <(printf '%s\n' "$changed" | awk -F/ '$1 == "crates" && $3 != "Cargo.toml" { if (!seen[$2]++) print; next } { print }')
  if [[ ${#scope[@]} -gt 0 ]]; then
    local unique
    unique="$(printf '%s\n' "${scope[@]}" | sort -u)"
    scope=()
    while read -r member; do scope+=("$member"); done <<< "$unique"
  fi
  echo "loft plan ($base): ${scope[*]:-no workspace changes}"
}

wasm() {
  tools/dev.sh check loft-runner --target wasm32-unknown-unknown --features web-sys
}

parity() {
  tools/dev.sh build loft-cli --fast --bin loft
  tools/gpu_lock.sh "$CARGO_TARGET_DIR/lab/loft" controls "$@"
}

full_gate() {
  local out cli baseline_root failed=0 profile="${LOFT_GATE_PROFILE:-lab}"
  : "${LOFT_BASELINE_BIN:?full gate requires a prebuilt Loft binary from the baseline checkout}"
  if [[ -z ${LOFT_BASELINE_PICTURES:-} ]]; then
    : "${LOFT_BASELINE_BUILD_LOG:?full gate requires baseline captures or the baseline Cargo artifact log}"
  fi
  : "${LOFT_TIMING_MARGIN:?full gate requires an allowed paired delta as a fraction of the timing budget}"
  baseline_root="$(git -C "$(dirname "$LOFT_BASELINE_BIN")" rev-parse --show-toplevel)"
  out="$(mktemp -d "$root/out/check.XXXXXX")"
  tools/dev.sh check --workspace --target wasm32-unknown-unknown --features loft-runner/web-sys || failed=$?
  tools/dev.sh test --workspace --doc --profile "$profile" || failed=$?
  tools/dev.sh build --workspace --profile "$profile" --bins --tests --examples --message-format=json-render-diagnostics >"$out/build.jsonl"
  corpus_lint "$CARGO_TARGET_DIR/$profile/loft" || failed=$?
  tools/gpu_lock.sh uv run --no-project python tools/run_native_tests.py "$out/build.jsonl" || failed=$?
  cli="${CARGO_TARGET_DIR:-$root/target}/$profile/loft"
  tools/quiet_gpu.sh "$cli" gate >"$out/gates.log" 2>&1 || failed=$?
  [[ ! -f "$root/out/meta/gate.json" ]] || cp "$root/out/meta/gate.json" "$out/gate.json"
  tools/gpu_lock.sh uv run --script tools/parity/d1d/bead.py full "$(dirname "$cli")/examples/phase5_corpus" "$cli" "$out/bead" || failed=$?
  local baseline_pictures="${LOFT_BASELINE_PICTURES:-$out/main-pictures}"
  if [[ -z ${LOFT_BASELINE_PICTURES:-} ]]; then
    uv run --script tools/gates/pictures.py capture "$LOFT_BASELINE_BUILD_LOG" "$baseline_root" "$baseline_pictures"
  fi
  uv run --script tools/gates/pictures.py capture "$out/build.jsonl" "$root" "$out/candidate-pictures"
  uv run --script tools/gates/pictures.py deterministic "$baseline_pictures" "$out/candidate-pictures" --scorer "$cli" >"$out/pictures.json" || failed=$?
  tools/gpu_lock.sh uv run --script tools/gates/pictures.py stochastic "$LOFT_BASELINE_BIN" "$cli" "$out/path-pictures" >"$out/stochastic.json" || failed=$?
  uv run --no-project python tools/gates/timing.py "$LOFT_BASELINE_BIN" "$cli" "$out/timing" --margin "$LOFT_TIMING_MARGIN" || failed=$?
  uv run --no-project python tools/lines.py >"$out/lines.log" || failed=$?
  return "$failed"
}

corpus_lint() {
  # Enforce the budget on regenerated ports. Report warnings on legacy ports.
  find corpus/l5c corpus/l5d -type f -name '*.loft' -print0 |
    xargs -0 "$1" lint &&
  find corpus/l5e corpus/seeds -type f -name '*.loft' -print0 |
    xargs -0 "$1" lint --strict
}

case "${1:-}" in
  lint)
    if [[ $# == 2 ]]; then
      corpus_lint "$2"
    else
      tools/dev.sh build loft-cli --fast --bin loft
      corpus_lint "$CARGO_TARGET_DIR/lab/loft"
    fi ;;
  lines) uv run --no-project python tools/lines.py ;;
  static) static_checks ;;
  plan) plan ;;
  parity) shift; parity "$@" ;;
  quick)
    static_checks
    wasm
    plan
    for crate in ${scope[@]+"${scope[@]}"}; do
      tools/gpu_lock.sh tools/dev.sh test "$crate" --test-threads 1
    done ;;
  full) static_checks; mkdir -p "$root/out"; full_gate ;;
  *) echo 'usage: tools/check.sh lines|static|lint|plan|quick|full|parity [controls options]' >&2; exit 2 ;;
esac
