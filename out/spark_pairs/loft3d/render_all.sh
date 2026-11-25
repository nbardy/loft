#!/usr/bin/env bash
# Render all 41-step chains with the workspace loft binary (needs Metal:
# run from a normal terminal, not from the sandboxed agent session).
# Usage: bash out/spark_pairs/loft3d/render_all.sh
set -euo pipefail
cd "$(dirname "$0")/../../.."
for c in golem-shape lantern-color golem-texture golem-expression \
         solar-material abyss-lure rune-legibility; do
  mkdir -p "out/spark_pairs/loft3d/$c"
  for t in $(seq 0 40); do
    n=$(printf %02d "$t")
    sed "s/param step: u = [0-9]* in/param step: u = $t in/" \
      "out/spark_pairs/chains/$c/chain.loft" \
      > "out/spark_pairs/loft3d/$c/step_$n.loft"
    ./target/debug/loft still \
      "out/spark_pairs/loft3d/$c/step_$n.loft" \
      "out/spark_pairs/loft3d/$c/step_$n.png" --t 0
  done
  echo "CHAIN $c DONE"
done
echo ALL_RENDERED
ls out/spark_pairs/loft3d/*/step_*.png | wc -l
