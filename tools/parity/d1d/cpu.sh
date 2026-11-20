#!/usr/bin/env bash
# CPU admission for the unchanged anchor, large import and fixture controls.
set -euo pipefail
if [[ $# != 3 ]]; then
  echo 'usage: cpu.sh LOFT I5_FIXTURES OUT' >&2
  exit 2
fi
loft=$1
prior_fixtures=$2
out=$3
mkdir -p "$out/import"
for ext in loft gltf bin; do
  cp "$prior_fixtures/triangles.$ext" "$out/import/"
done
# The historical witness used the flat prelude. Move its import into the
# scratch entry so the pinned source and binary assets can be replayed as-is.
printf '\nuse stdlib::geometry::mesh::{Mesh}\n' >> "$out/import/triangles.loft"
assets=crates/loft-passes/tests/d1_assets
"$loft" build crates/loft-passes/tests/phase5/anchor.loft -o "$out/anchor.loftb" > "$out/anchor.log"
"$loft" build "$assets/primitives.loft" -o "$out/primitives.loftb" > "$out/primitives.log"
for control in clearcoat:E602 unmapped:E601; do
  fixture=${control%:*}
  code=${control#*:}
  if "$loft" build "$assets/$fixture.loft" > "$out/$fixture.log" 2>&1; then
    echo "$fixture unexpectedly built" >&2
    exit 1
  fi
  rg -F "error[$code]" "$out/$fixture.log"
done
uv run --no-project python - "$loft" "$out" <<'PY'
import json
from pathlib import Path
import subprocess
import sys
import time

loft, out = Path(sys.argv[1]), Path(sys.argv[2])
fixture = out / "import" / "triangles.loft"
count = json.loads(fixture.with_suffix(".gltf").read_bytes())["accessors"][0]["count"] // 3
if count != 100_000:
    raise RuntimeError(f"expected 100,000 triangles, got {count}")
start = time.perf_counter()
with (out / "import.log").open("w") as log:
    code = subprocess.run([loft, "build", fixture], stdout=log, stderr=subprocess.STDOUT).returncode
result = {"triangles": count, "end_to_end_ms": (time.perf_counter() - start) * 1000.0, "exit": code}
(out / "import.json").write_text(json.dumps(result, indent=2) + "\n")
print(json.dumps(result))
raise SystemExit(0 if code == 0 and result["end_to_end_ms"] < 100.0 else 1)
PY
