#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/../.."
dst=out/slice/bead
ref=agent_notes/2026-11-19_design/render_impl/R0g
LOFT_ENGINE=rust bun src/cli/main.ts build examples/slice/bead.ts --naga --out "$dst"
tools/dev.sh build loft-draw --fast --example parity-diff
args=()
for reference in "$ref"/*.png; do
  shot=$(basename "$reference" .png)
  IFS=@ read -r view time size <<< "$shot"
  if [[ ${#args[@]} -eq 0 ]]; then
    args=(--view "$view" --time "$time" --size "$size" --out "$dst/$shot.png")
  else
    args+=(--shot "$shot=$dst/$shot.png")
  fi
done
rust/target/lab/loft-draw still "$dst" "${args[@]}"
status=0
for reference in "$ref"/*.png; do
  rust/target/lab/examples/parity-diff "$reference" "$dst/$(basename "$reference")" || status=1
done
exit "$status"
