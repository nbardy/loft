#!/usr/bin/env bash
# Lane 4.2's Linux/Vulkan run (plan 2026-11-19 §8.2 lane 4.2, §2.3): the entrypoint of a Linux GPU host or container.
#
#   tools/parity/linux.sh MAC_R0_DIR      MAC_R0_DIR: the Mac's out/parity/R0, copied here (rsync -a mac:loft/out/parity/R0/ ...)
#
# Steps, each a pass/fail line; the exit is 1 if any failed:
#   1. tape bits equal macOS: loft-core's d_tape_digest prints c711752d05cd8265 (lane 1.3 (d));
#   2. the corpus builds and renders on Vulkan into out/parity/linux (run.ts reference);
#   3. every frame meets the cross-backend rule against MAC_R0_DIR, and every build ends as it did there (run.ts cross);
#   4. the Vulkan pipeline cache: Titan loads twice into an empty cache, then twice from the saved one (loft-draw profile).
# The process ends when the run does: a container started for it scales to zero on exit. Nothing here starts one.
set -euo pipefail
root="$(cd "$(dirname "$0")/../.." && pwd -P)"
r0="$(cd "${1:?usage: tools/parity/linux.sh MAC_R0_DIR}" && pwd -P)"
[[ "$(uname -s)" == Linux ]] || { echo "linux.sh: this is the Linux/Vulkan run; on macOS use run.ts directly" >&2; exit 2; }
command -v vulkaninfo >/dev/null && vulkaninfo --summary >/dev/null 2>&1 \
  || { echo "linux.sh: no Vulkan device (vulkaninfo --summary failed). Fix: install the driver's ICD and vulkan-tools" >&2; exit 6; }
[[ -f "$r0/r0" ]] || { echo "linux.sh: $r0 holds no R0 stamp; copy the Mac's out/parity/R0 whole" >&2; exit 2; }
cd "$root"
[[ "$(git rev-parse HEAD)" == "$(cat "$r0/r0")" ]] \
  || echo "linux.sh: HEAD $(git rev-parse --short HEAD) is not R0 $(cut -c1-9 "$r0/r0"); pictures then compare two commits too" >&2
status=0
mark() { echo "linux.sh: $1 $2"; [[ "$1" == PASS ]] || status=1; }
echo "linux.sh: start load1 $(cut -d' ' -f1 /proc/loadavg), cpus $(nproc)"

tools/dev.sh build loft-draw --fast
tools/dev.sh build loft-node --fast
cargo build --release --locked --manifest-path tools/parity/Cargo.toml

mkdir -p out/lr13
bun rust/crates/loft-core/dump_math.ts > out/lr13/math.json
digest="$(tools/dev.sh test loft-core -E 'test(d_tape_digest)' --no-capture 2>&1 | sed -n 's/.*tape digest \([0-9a-f]*\).*/\1/p')"
[[ "$digest" == c711752d05cd8265 ]] && mark PASS "tape digest $digest" || mark FAIL "tape digest '${digest}', macOS c711752d05cd8265"

export LOFT_PARITY_OUT=out/parity/linux
bun tools/parity/run.ts reference && mark PASS "corpus built and drawn (out/parity/linux/manifest.json)" || mark FAIL "corpus reference run"
bun tools/parity/run.ts cross "$r0" && mark PASS "cross rule against $r0" || mark FAIL "cross rule (out/parity/linux/cross.json)"

titan="$LOFT_PARITY_OUT/build/examples__bosses__reefback_titan"
rm -rf out/parity/linux/cache
for k in cold warm; do
  rust/target/lab/loft-draw profile "$titan" --loads 2 --frames 16 --wait --pipeline-cache out/parity/linux/cache \
    | tail -1 > "out/parity/linux/cache_$k.json" && mark PASS "pipeline cache $k: $(cat "out/parity/linux/cache_$k.json")" \
    || mark FAIL "pipeline cache $k"
done
echo "linux.sh: end load1 $(cut -d' ' -f1 /proc/loadavg)"
exit "$status"
