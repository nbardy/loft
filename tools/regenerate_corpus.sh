#!/usr/bin/env bash
# Replay the pinned Phase 5 CPU sampler without editing the legacy source archive.
set -euo pipefail
cd "$(dirname "$0")/.."
root="$PWD"
mkdir -p "$root/out"
scratch="$(mktemp -d "$root/out/seed-sampler.XXXXXX")"
git archive a19a5ae src tools/migrate.ts package.json pnpm-lock.yaml pnpm-workspace.yaml patches |
  tar -x -C "$scratch"
if [[ -n "${LOFT_SEED_NODE_MODULES:-}" ]]; then
  ln -s "$LOFT_SEED_NODE_MODULES" "$scratch/node_modules"
else
  pnpm --dir "$scratch" install --frozen-lockfile --ignore-scripts
fi
cp tools/regenerate_corpus.ts "$scratch/regenerate_corpus.ts"
# The historical evaluator interns expressions for the lifetime of its process.
# Bound that cache without changing the pinned sampler or repeating a seed.
if [[ $# -gt 0 ]]; then
  printf '%s\n' "$@"
else
  uv run --no-project python - "$root/corpus/manifest.json" <<'PYTHON'
import json
import sys
with open(sys.argv[1]) as manifest:
    for batch in json.load(manifest)["batches"]:
        for identity in batch["scenes"]:
            if not identity.startswith("examples/"):
                print(identity)
PYTHON
fi | xargs -n 6 bun "$scratch/regenerate_corpus.ts" "$root"
