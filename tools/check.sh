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
  for crate in loft-runner loft-passes loft-plan loft-parse loft-abi loft-ir loft-format; do
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

full_gate() {
  local out cli
  out="$(mktemp -d "$root/out/check.XXXXXX")"
  tools/dev.sh build --workspace --fast --bins --tests --examples --message-format=json-render-diagnostics >"$out/build.jsonl"
  tools/gpu_lock.sh uv run --no-project python tools/run_native_tests.py "$out/build.jsonl"
  tools/dev.sh test --workspace --doc --fast
  wasm
  cli="${CARGO_TARGET_DIR:-$root/target}/lab/loft"
  tools/gpu_lock.sh uv run --script tools/parity/d1d/bead.py full "$(dirname "$cli")/examples/phase5_corpus" "$cli" "$out/bead"
  tools/gpu_lock.sh "$cli" gate
}

case "${1:-}" in
  static) static_checks ;;
  plan) plan ;;
  quick)
    static_checks
    wasm
    plan
    for crate in ${scope[@]+"${scope[@]}"}; do
      tools/gpu_lock.sh tools/dev.sh test "$crate" --test-threads 1
    done ;;
  full) static_checks; mkdir -p "$root/out"; full_gate ;;
  *) echo 'usage: tools/check.sh static|plan|quick|full' >&2; exit 2 ;;
esac
