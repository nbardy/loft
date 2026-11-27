#!/usr/bin/env bash
# Render every base model once with the workspace loft binary (needs Metal:
# run from a normal terminal, not from the sandboxed agent session).
# Usage: bash out/spark_pairs/models/render_all.sh
set -euo pipefail
cd "$(dirname "$0")/../../.."
for m in moss-archer rust-knight ember-imp frost-warden tide-priest \
         dune-merchant kelp-diver ash-monk cloud-herder bone-collector; do
  ./target/debug/loft still \
    "out/spark_pairs/models/$m/model.loft" \
    "out/spark_pairs/models/$m/model.png" --t 0
done
echo ALL_RENDERED
ls out/spark_pairs/models/*/model.png | wc -l
