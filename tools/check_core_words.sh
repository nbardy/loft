#!/usr/bin/env bash
set -eu
# An optional root lets the mutant test exercise this same gate on an isolated tree.
cd "${1:-$(dirname "$0")/..}"
scan() {
  if rg --hidden --no-ignore "$@"; then exit 1; else local status=$?; [[ $status == 1 ]] || exit "$status"; fi
}
# Runner & IR must stay domain-free and host-IO-free.
scan -i -w 'foot|gait|creature|trunk|pbr|shadow|sea|splat|mpm' crates/loft-{runner,ir}
scan -i -w 'body|law|solver|plan|bsdf|visrec' crates/loft-runner
scan -i -w 'std::(time::Instant|fs|thread)' crates/loft-runner/src

# Architectural red lines (docs/PHILOSOPHY.md §6) across all core crates:
# 1. No hardcoded scene/style/creature names in Rust core.
shopt -s nullglob
core_srcs=(crates/*/src)
if [[ ${#core_srcs[@]} -gt 0 ]]; then
  scan -w 'woodblock_pigment|spectral_dielectric|CreatureHeader|BodyRow|ChainRow|SegmentRow|FootPin|ManipState|TrailNode|PoseJoint' "${core_srcs[@]}"
  scan -F '"sun_sky"' "${core_srcs[@]}"
  scan '\b(dbg!|todo!|unimplemented!)\s*\(' "${core_srcs[@]}"
fi
# 2. No brittle WGSL .replace(...) string surgery inside loft-emit (provenance.rs marks diagnostic spans only).
if [[ -d crates/loft-emit/src ]]; then
  scan -g '!provenance.rs' -F '.replace(' crates/loft-emit/src
fi
