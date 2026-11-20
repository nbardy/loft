#!/usr/bin/env bash
# Build both revisions, then compare their native Metal capsules at 1280x800.
# Usage: tools/ab_legacy.sh LEGACY_WORKTREE NEW_OUTPUT_DIRECTORY [--matched]
set -euo pipefail
[[ $# == 2 || ( $# == 3 && $3 == --matched ) ]] ||
  { echo "usage: $0 LEGACY_WORKTREE NEW_OUTPUT_DIRECTORY [--matched]" >&2; exit 2; }
matched=${3:-}
root="$(cd "$(dirname "$0")/.." && pwd -P)"
legacy="$(cd "$1" && pwd -P)"
out="$(mkdir "$2" && cd "$2" && pwd -P)"
[[ "$(git -C "$legacy" rev-parse HEAD)" == "$(git -C "$root" rev-parse 'legacy/2026-11-19^{commit}')" ]] ||
  { echo "legacy worktree must be at tag legacy/2026-11-19" >&2; exit 2; }
[[ -z "$(git -C "$legacy" status --porcelain --untracked-files=no)" ]] ||
  { echo "legacy worktree has tracked changes" >&2; exit 2; }
cd "$root"
export CARGO_BUILD_JOBS=2

# The old shape API places rows by translation only. Supply the segment direction
# as data to the same analytic cylinder-and-hemisphere construction as its capsule.
cat >"$out/capsules.ts" <<'TS'
const { kind, matte, realtime, scene, thing } = await import(
  process.env.LOFT_AB_LEGACY + "/src/render/index.ts"
);

const capsule = kind(`
struct Capsule { axis:vec3f, half_height:f32, radius:f32 }
fn bound(s:Capsule)->Box {
  let e=abs(s.axis)*s.half_height+s.radius;
  return Box(-e,e);
}
fn hit(s:Capsule,r:Ray)->Hit {
  let y=dot(r.o,s.axis); let v=dot(r.d,s.axis);
  let o=r.o-y*s.axis; let d=r.d-v*s.axis;
  let a=dot(d,d); let b=dot(o,d);
  let h=b*b-a*(dot(o,o)-s.radius*s.radius);
  var t=r.t1; var n=vec3f(0);
  if a>0 && h>=0 {
    let roots=(vec2f(-sqrt(h),sqrt(h))-b)/a;
    for(var i=0u;i<2u;i++) {
      let q=roots[i];
      if q>=r.t0 && q<t && abs(y+q*v)<=s.half_height {t=q;n=normalize(o+q*d);}
    }
  }
  for(var j=0u;j<2u;j++) {
    let side=2*f32(j)-1;
    let p=r.o-side*s.half_height*s.axis;
    let e=dot(p,r.d); let disc=e*e-dot(p,p)+s.radius*s.radius;
    if disc<0 {continue;}
    let roots=vec2f(-sqrt(disc),sqrt(disc))-e;
    for(var i=0u;i<2u;i++) {
      let q=roots[i]; let x=p+q*r.d;
      if q>=r.t0 && q<t && side*dot(x,s.axis)>=0 {t=q;n=normalize(x);}
    }
  }
  if t==r.t1 {return miss;}
  return Hit(t,n,n.xy);
}`);

type V3 = [number, number, number];
const count = Number(process.env.LOFT_AB_COUNT);
if (count !== 1 && count !== 20) throw new Error("LOFT_AB_COUNT must be 1 or 20");
const cols = Math.ceil(Math.sqrt(count)), rows = Math.ceil(count / cols);
const things = [];
for (let i = 0; i < count; i++) {
  const x = (i % cols - (cols - 1) / 2) * 2.4;
  const z = (Math.floor(i / cols) - (rows - 1) / 2) * 2.4;
  const a = i * 1.7 + 0.4;
  const skin = [0.55 + 0.35 * Math.sin(a), 0.5 + 0.35 * Math.sin(a + 2.1), 0.5 + 0.35 * Math.sin(a + 4.2)];
  const f = i % 2 === 0 ? 1 : -1;

  function add(from: V3, to: V3, radius: number, tint: number) {
    const delta = to.map((v, k) => v - from[k]);
    const length = Math.hypot(...delta);
    const sphereAxis: V3 = [0, 1, 0];
    const axis = length === 0 ? sphereAxis : delta.map(v => v / length);
    const center: V3 = [x + (from[0] + to[0]) / 2, (from[1] + to[1]) / 2, z + (from[2] + to[2]) / 2];
    things.push(thing(capsule(axis, length / 2, radius), matte(skin.map(v => v * tint)), center));
  }

  add([-0.35 * f, 0.8, 0], [0.35 * f, 0.8, 0], 0.38, 1);
  add([0.78 * f, 1.12, 0], [0.78 * f, 1.12, 0], 0.27, 1.15);
  add([-0.68 * f, 0.9, 0], [-1.05 * f, 1.32, 0], 0.07, 0.7);
  for (let leg = 0; leg < 4; leg++) {
    const sx = leg % 2 === 0 ? 0.3 : -0.3;
    const sz = leg < 2 ? 0.22 : -0.22;
    add([sx, 0.62, sz], [sx * 1.3, 0.06, sz * 1.6], 0.09, 0.55);
  }
}

export default scene({
  things,
  engine: realtime(),
  sun: { towards: [0.45, 0.8, 0.35], power: [2.2, 2, 1.7] },
  cameras: { game: {
    kind: "perspective",
    eye: [0, 0.6 + 1.2 + 0.6 * Math.sqrt(count), 3.6 + 1.9 * Math.sqrt(count)],
    target: [0, 0.6, 0],
    fovY: 40 * Math.PI / 180,
  } },
});
TS

if [[ $matched == --matched ]]; then
  # Keep the capsule rows, count and orbit in the shipped fixture. Fork its
  # material and substitute the experimental library schedule in a scratch scene.
  mkdir -p "$out/fixture/engines"
  ln -s "$root/stdlib" "$out/fixture/stdlib"
  ln -s "$root/Cargo.toml" "$out/fixture/Cargo.toml"
  cp "$root/stdlib/engines/capsules.loft" "$out/fixture/engines/"
  awk '/^schedule Main/{exit} /^use .*::geometry::analytic/ {next} /^use .*::materials::bsdf/ {$0="use stdlib::materials::bsdf::{lambert}"} {sub(/ggx\(albedo, 0.55, 0\)/,"lambert(albedo)"); sub(/ray_capsule\(/,"matched_capsule("); print}' \
    "$root/stdlib/engines/capsules.loft" >"$out/fixture/engines/capsules-matched.loft"
  orbit=$(awk '/^  orbit\(/{print; exit}' "$root/stdlib/engines/capsules.loft")
  [[ -n $orbit ]]
  awk -v orbit="$orbit" '
    /^  orbit\(/ {print orbit; next}
    {gsub(/Drawables/, "Capsule"); print}
  ' "$root/stdlib/engines/realtime_matched.loft" >>"$out/fixture/engines/capsules-matched.loft"
  # Recenter closed-form hit arithmetic on both sides of the matched fixture.
  # From the far camera, the uncentered legacy quadratic can move the secondary
  # ray origin inside its own capsule, especially on the thin tails and legs.
  sed \
    -e 's/  let y=dot(r.o,s.axis);/  let shift=-dot(r.o,r.d); let ro=r.o+shift*r.d; let y=dot(ro,s.axis);/' \
    -e 's/let o=r.o-y/let o=ro-y/' \
    -e 's/let p=r.o-side/let p=ro-side/' \
    -e 's/let q=roots\[i\];/let q=roots[i]; let qt=q+shift;/' \
    -e 's/q>=r.t0 \&\& q<t/qt>=r.t0 \&\& qt<t/' \
    -e 's/{t=q;n=/{t=qt;n=/' \
    "$out/capsules.ts" >"$out/capsules-matched.ts"
  sed 's@engine: realtime(),@engine: (await import(process.env.LOFT_AB_REDUCED)).reduced,@' \
    "$out/capsules-matched.ts" >"$out/capsules-reduced.ts"
else
  # Keep the gate scratch report separate when run from the primary checkout.
  mkdir -p "$out/gate/tests/fixtures"
  ln -s "$root/engines" "$out/gate/engines"
  ln -s "$root/tests/fixtures/gate-refs" "$out/gate/tests/fixtures/gate-refs"
fi
git -C "$root" rev-parse HEAD >"$out/new-revision"
git -C "$legacy" rev-parse HEAD >"$out/legacy-revision"

tools/dev.sh check loft-cli >"$out/new-check.log" 2>&1
tools/dev.sh build loft-cli --fast >"$out/new-build.log" 2>&1
(cd "$legacy" && tools/dev.sh build loft-draw --fast -p loft-node) >"$out/legacy-build.log" 2>&1

if [[ $matched == --matched ]]; then
  for count in 1 20; do
    for split in a b; do
      src="$out/capsules-matched.ts"
      [[ $split == b ]] || src="$out/capsules-reduced.ts"
      (
        cd "$legacy"
        LOFT_AB_LEGACY="$legacy" LOFT_AB_COUNT="$count" LOFT_ENGINE=ts \
          LOFT_AB_REDUCED="$root/tools/ab_legacy/reduced.ts" \
          LOFT_NATIVE="$legacy/rust/target/lab/libloft_node.dylib" \
          /usr/bin/time -p bun src/cli/main.ts build "$src" --precision f32 --out "$out/$split-$count-bundle"
      ) >"$out/$split-$count-author-load.log" 2>&1
    done
    "$root/target/lab/loft" check "$out/fixture/engines/capsules-matched.loft" --set "count=$count" \
      >"$out/b-$count-check.log"
  done
  # This is a full-screen constant-color triangle in the same native recorder.
  # Strip every scheduled pass except Present from the compiled control.
  awk '
    /^schedule Main/ {print "fn ab_triangle(c: rgb) -> rgb { [0.25, 0.5, 0.75] }"}
    /tonemap_aces/ {print "  tonemap(ColorHdr, sky: sky_color, film: ab_triangle)"; next}
    {print}
  ' "$root/stdlib/engines/capsules.loft" >"$out/fixture/engines/triangle.loft"
  "$root/target/lab/loft" ir "$out/fixture/engines/triangle.loft" --set count=1 \
    | sed 's/(passes Cull (Vis 0) Classify (Shade 0) Present)/(passes Present)/' >"$out/triangle.sexp"
  [[ $(rg -o '\(passes Present\)' "$out/triangle.sexp") == '(passes Present)' ]]
fi

# Acquire both of the repository's compile slots while holding the shared GPU lock.
source tools/lib/build_gate.sh
tools/gpu_lock.sh perl -MFcntl=:flock -e '
  use strict;
  use warnings;
  my $dir = shift;
  mkdir $dir unless -d $dir;
  open(my $heavy, ">>", "$dir/slot.0") or die "heavy compile lock: $!";
  flock($heavy, LOCK_EX) or die "heavy compile lock: $!";
  open(my $light, ">>", "$dir/slot.light") or die "light compile lock: $!";
  flock($light, LOCK_EX) or die "light compile lock: $!";
  system(@ARGV) == 0 or die "timed benchmark failed (status $?)\n";
' "$(keel_gate_dir)" bash -euo pipefail -s "$root" "$legacy" "$out" "$matched" <<'TIMED'
root=$1
legacy=$2
out=$3
matched=$4
new="$root/target/lab/loft"
old="$legacy/rust/target/lab/loft-draw"
scene="$root/stdlib/engines/capsules.loft"
ps -Ao pid,comm >"$out/processes-before.txt"
date -u >"$out/gpu-start"
finish() {
  date -u >"$out/gpu-end"
  ps -Ao pid,comm >"$out/processes-after.txt"
}
trap finish EXIT
no_builds() {
  local status=0
  pgrep -lx 'cargo|rustc' >"$1" || status=$?
  if ((status != 1)); then
    printf 'Cannot verify idle compilers before %s (pgrep status %d)\n' "$1" "$status" >&2
    return 1
  fi
}
if [[ $matched == --matched ]]; then
  size=1280x800
  old_profile() {
    "$old" profile "$bundle" --view game --time 0 --size "$size" --frames 300 >"$prefix-old-profile.log"
  }
  new_profile() {
    "$new" profile "$scene" --set "count=$count" --time 0 --size "$size" --frames 300 \
      --warmup 1 --timestamps immediate --json "$prefix-new-profile.json"
  }
  old_load() {
    "$old" profile "$bundle" --view game --time 0 --size "$size" --frames 1 --loads 2 \
      --cold-nonce "$nonce" >"$prefix-old-load.log"
  }
  new_load() {
    "$new" profile "$scene" --set "count=$count" --time 0 --size "$size" --frames 1 \
      --warmup 1 --nonce "$nonce" --loads 2 --json "$prefix-new-load.json"
  }
  control() {
    local file=$1
    no_builds "$file-pgrep.txt"
    "$new" profile "$out/triangle.sexp" --time 0 --size "$size" --warmup 10 --frames 40 \
      --json "$file.json"
    jq -e '.passes == ["Present"] and (.gpu_pass_ms | length == 40)' "$file.json" >/dev/null
    jq '[.gpu_pass_ms[][0]] | sort | .[37]' "$file.json" | tee "$file-p95-ms"
  }

  # Capture lossless stills once. They remain useful if the timing control fails.
  for count in 1 20; do
    for split in a b; do
      bundle="$out/$split-$count-bundle"
      scene="$out/fixture/engines/capsules.loft"
      [[ $split == a ]] || scene="$out/fixture/engines/capsules-matched.loft"
      prefix="$out/$split-$count"
      "$old" still "$bundle" --view game --time 0 --size "$size" --out "$prefix-old.png" \
        >"$prefix-old-still.log" 2>&1
      "$new" still "$scene" "$prefix-new.png" --set "count=$count" --t 0 \
        >"$prefix-new-still.log" 2>&1
    done
  done

  control "$out/triangle-before"
  if ! jq -e '. < 0.15' "$out/triangle-before-p95-ms" >/dev/null; then
    echo 'Triangle p95 is at or above 0.15 ms. Skipping paired timings.' >&2
    exit 0
  fi
  for count in 1 20; do
    for split in a b; do
      bundle="$out/$split-$count-bundle"
      scene="$out/fixture/engines/capsules.loft"
      [[ $split == a ]] || scene="$out/fixture/engines/capsules-matched.loft"
      prefix="$out/$split-$count"
      no_builds "$prefix-pgrep-before.txt"
      # Reverse order between cases while keeping both samples adjacent.
      if [[ $count-$split == 1-a || $count-$split == 20-b ]]; then
        old_profile
        new_profile
      else
        new_profile
        old_profile
      fi
      no_builds "$prefix-pgrep-after.txt"
      nonce=$((16777216 + $(od -An -N4 -tu4 /dev/urandom) % 2000000000))
      old_load
      new_load
    done
  done
  control "$out/triangle-after"
  exit
fi

for count in 1 20; do
  bundle="$out/bundle-$count"
  (
    cd "$legacy"
    LOFT_AB_LEGACY="$legacy" LOFT_AB_COUNT="$count" LOFT_ENGINE=ts \
      LOFT_NATIVE="$legacy/rust/target/lab/libloft_node.dylib" \
      /usr/bin/time -p bun src/cli/main.ts build "$out/capsules.ts" --precision f32 --out "$bundle"
  ) >"$out/legacy-$count-author-load.log" 2>&1
  # Only the load probe uses a fresh driver key. Score frame time on the original shaders.
  nonce=$((16777216 + $(od -An -N4 -tu4 /dev/urandom) % 2000000000))
  "$old" profile "$bundle" --view game --time 0 --size 1280x800 --frames 1 --loads 2 \
    --cold-nonce "$nonce" >"$out/legacy-$count-load.log" 2>&1
  "$old" profile "$bundle" --view game --time 0 --size 1280x800 --frames 300 \
    >"$out/legacy-$count-profile.log" 2>&1
  "$new" profile "$scene" --set "count=$count" --time 0 --size 1280x800 --frames 300 \
    --warmup 1 --timestamps immediate --json "$out/new-$count-profile.json"
  "$old" still "$bundle" --view game --time 0 --size 1280x800 --out "$out/legacy-$count.png" \
    >"$out/legacy-$count-still.log" 2>&1
  "$new" still "$scene" "$out/new-$count.png" --set "count=$count" --t 0
done

# The gate writes its report before returning a verdict exit.
gate_status=0
(cd "$out/gate" && "$new" gate --only capsules-1,capsules-20) >"$out/new-gate.log" 2>&1 || gate_status=$?
case $gate_status in 0|2|3|4|7) ;; *) exit "$gate_status" ;; esac
jq -e 'with_entries(select(.key | startswith("capsules-")) | .value = .value.load)
       | select(length == 2 and all(.[]; . != null))' \
  "$out/gate/out/meta/gate.json" >"$out/new-load.json"
TIMED

if [[ $matched == --matched ]]; then
  uv run --no-project --with tiktoken==0.13.0 --with pillow==12.0.0 \
    python - "$out" "$root" <<'PYTHON'
import hashlib
import json
import math
from pathlib import Path
import sys

out, root = map(Path, sys.argv[1:])
sys.path.insert(0, str(root / "tools/ab_legacy"))
from score import compare, legacy_profile, open_frame, read


def p95(values):
    return sorted(values)[round(0.95 * (len(values) - 1))]


def ci95(values):
    # Equal-tail, distribution-free order-statistic interval for the p95.
    values = sorted(values)
    n = len(values)
    cdf = 0
    lower, upper = 0, n - 1
    for k in range(n + 1):
        cdf += math.comb(n, k) * 0.95**k * 0.05**(n - k)
        if cdf <= 0.025:
            lower = k
        if cdf >= 0.975:
            upper = min(k, n - 1)
            break
    return [values[lower], values[upper]]


def control(name):
    path = out / f"triangle-{name}-p95-ms"
    return read(path) if path.exists() else None


controls = [control("before"), control("after")]
results = {
    "triangle_p95_ms": controls,
    "uncontended": all(value is not None and value < 0.15 for value in controls),
    "cases": {},
}
for split in ("a", "b"):
    for count in (1, 20):
        prefix = out / f"{split}-{count}"
        images = [open_frame(Path(f"{prefix}-{side}.png")) for side in ("old", "new")]
        case = results["cases"][prefix.name] = {
            "pictures": compare(*images),
            "rgb_sha256": [hashlib.sha256(image.tobytes()).hexdigest() for image in images],
        }
        native_path = Path(f"{prefix}-new-profile.json")
        if not native_path.exists():
            continue
        old = legacy_profile(Path(f"{prefix}-old-profile.log"))
        new = read(native_path)
        samples = new["gpu_frame_ms"]
        assert old["frames"] == len(samples) == 300
        assert old["adapter"] in new["adapter"]
        old_load = legacy_profile(Path(f"{prefix}-old-load.log"))
        new_load = read(Path(f"{prefix}-new-load.json"))
        # The tagged profiler exports only four quantiles. Its observed median
        # and maximum bound the missing 95% order statistics conservatively.
        case["legacy"] = {"p95_ms": old["gpu_ms"][2], "p95_ci_ms": old["gpu_ms"][1::2],
                          "loads_ms": old_load["loads_ms"], "pass_ms": old["pass_ms"]}
        case["native"] = {"p95_ms": p95(samples), "p95_ci_ms": ci95(samples),
                          "loads_ms": new_load["device_loads_ms"],
                          "commands": sorted({frame["commands"] for frame in new["frame_stamps"]})}
(out / "results.json").write_text(json.dumps(results, indent=2) + "\n")
PYTHON
  printf 'Evidence: %s\n' "$out"
  if ! jq -e '.uncontended' "$out/results.json" >/dev/null; then
    echo 'Benchmark requires both triangle controls under 0.15 ms p95.' >&2
    exit 6
  fi
  exit
fi

# Columns are legacy/new. Rows are 1/20 instance groups. Keep RGBA originals in out/.
ffmpeg -v error -i "$out/legacy-1.png" -i "$out/new-1.png" \
  -i "$out/legacy-20.png" -i "$out/new-20.png" \
  -filter_complex 'xstack=inputs=4:layout=0_0|w0_0|0_h0|w0_h0' -frames:v 1 \
  -threads 1 "$out/sheet.png"
cwebp -quiet -q 95 "$out/sheet.png" -o "$out/sheet.webp"
uv run --no-project --with tiktoken==0.13.0 python - "$out/capsules.ts" "$root/stdlib/engines/capsules.loft" <<'PY' >"$out/source-tokens.tsv"
from pathlib import Path
import sys
import tiktoken

encoding = tiktoken.get_encoding("o200k_base")
for file in map(Path, sys.argv[1:]):
    print(file.name, len(encoding.encode_ordinary(file.read_text())), sep="\t")
PY
printf 'Evidence: %s\n' "$out"
