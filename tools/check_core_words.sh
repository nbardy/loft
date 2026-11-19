#!/usr/bin/env bash
set -eu
# An optional root lets the mutant test exercise this same gate on an isolated tree.
cd "${1:-$(dirname "$0")/..}"
scan() {
  if rg --hidden --no-ignore -i -w "$@"; then exit 1; else local status=$?; [[ $status == 1 ]] || exit "$status"; fi
}
scan 'foot|gait|creature|trunk|pbr|shadow|sea|splat|mpm' crates/loft-{runner,ir}
scan 'body|law|solver|plan|bsdf|visrec' crates/loft-runner
scan 'std::(time::Instant|fs|thread)' crates/loft-runner/src
