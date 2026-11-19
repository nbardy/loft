#!/usr/bin/env bash
# Both the WGSL writer and Metal executor are R0's sources. Only the measured look is added.
set -euo pipefail
cd "$(dirname "$0")/../.."
base=out/slice/r0
dst=agent_notes/2026-11-19_design/render_impl/R0g
mkdir -p "$base" "$dst" "$base/tools/lib"
git archive d3674db src examples/bead.ts | tar -xf - -C "$base"
if [[ ! -e "$base/rust/crates/loft-loader" ]]; then
  git archive d3674db rust/loft-loader | tar -xf - -C "$base"
  mkdir -p "$base/rust/crates"
  mv "$base/rust/loft-loader" "$base/rust/crates/loft-loader"
fi
cp tools/dev.sh "$base/tools/"
cp -R tools/lib/. "$base/tools/lib/"
cp rust/rust-toolchain.toml "$base/rust/"
cp "$base/rust/crates/loft-loader/Cargo.lock" "$base/rust/"
cat > "$base/rust/Cargo.toml" <<'TOML'
[workspace]
members = ["crates/*"]
resolver = "2"
[profile.lab]
inherits = "release"
lto = false
TOML
"$base/tools/dev.sh" build loft-loader --fast --bin loft-view
cat > "$base/look.ts" <<'TS'
import bead from "./examples/bead.ts";
import { vec3 } from "./src/index.ts";
// Three FNV-1a bytes per draw label. Retain the authored ink function and width.
function flat(name:string) {
  let h=2166136261;for(const c of name) h=Math.imul(h^c.charCodeAt(0),16777619);
  return vec3((h>>>16&255)/255,(h>>>8&255)/255,(h&255)/255);
}
// R0's ground tests its analytic depth but never stores it. Include that plane in the distance observation.
// RGB is resolved flat colour. Alpha stores log2(1+distance)/10, with code 255 for the sky.
const depth = {label:"flat distance",wgsl:`
@fragment fn fs(in:PostIn)->@location(0) vec4f {
  let px=in.px.xy;
  let world=scene_position(px);
  let dir=normalize(world-frame.eye.xyz);
  let plane=-frame.eye.y/dir.y;
  var t=select(1e30,length(world-frame.eye.xyz),scene_depth(px)<1);
  if plane>0 { t=min(t,plane); }
  return vec4f(textureLoad(loft_post_colour,vec2i(px),0).rgb, min(log2(1+t)/10,1));
}`};
export default {...bead,style:{...bead.style,shade:()=>flat("bead"),ground:()=>flat("ground"),sky:()=>flat("sky"),post:[depth]}};
TS
LOFT_ENGINE=ts bun "$base/src/cli/main.ts" build "$base/look.ts" --out "$base/files"
for view in whole through; do
  for t in 0 0.5 1.37 2.9; do
    "$base/rust/target/lab/loft-view" "$base/files" --view "$view" --time "$t" --size 512x512 --out "$dst/$view@$t@512x512.png"
  done
done
"$base/rust/target/lab/loft-view" "$base/files" --view whole --time 1.37 --size 1280x800 --out "$dst/whole@1.37@1280x800.png"
