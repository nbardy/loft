"""Hold projected geometry and the draw order fixed across the two native builds."""
import argparse
import shutil
import subprocess

from fresh import checkpoint, notes, out, scenes

parser = argparse.ArgumentParser()
parser.add_argument("size")
parser.add_argument("--scene", choices=("walrus", "otter"), default="walrus")
parser.add_argument("--tiles", action="store_true", help="render the stable tile build against the ordered reference")
args = parser.parse_args()
source = out / "source/fixtures" / (scenes[args.scene] + ".loft")
fixed = source.with_name("fixed_order.loft")
# A dispatch resolves its template paths relative to the authoring schedule.
local = fixed.parent / "core"
local.mkdir(exist_ok=True)
for template in ["cull.wgsl", "cull_single.wgsl"]:
    shutil.copy2(notes.parents[1] / "stdlib/core" / template, local / template)
body = (notes.parents[1] / "stdlib/core.loft").read_text().split("pub schedule cull_indirect", 1)[1].split("\n}\n", 1)[0]
choice = 'if drawn == 1 { "core/cull_single.wgsl" } else { "core/cull_multi.wgsl" }'
assert choice in body and source.read_text().count("cull_indirect(") == 1
fixed.write_text(source.read_text().replace("cull_indirect(", "cull_ordered(")
                 + "\nschedule cull_ordered" + body.replace(choice, '"core/cull_single.wgsl"') + "\n}\n")
for variant, executable, path in [
    ("repeat-original", "original", source),
    ("ordered-original", "original", fixed),
    ("ordered-guard", "revised", fixed),
    ("ordered-scissor", "scissor", fixed),
    ("ordered-tiles", "tiles", fixed),
]:
    if args.tiles and variant not in ("ordered-original", "ordered-tiles"):
        continue
    if not args.tiles and variant == "ordered-tiles":
        continue
    if args.scene == "otter" and variant == "ordered-guard":
        continue
    dest = out / variant / f"{args.scene}-{args.size}-clip"
    if not (dest / "meta.json").exists():
        subprocess.run([out / executable / "loft", "clip", path, "--size", args.size,
                        "--frames", "120", "--fps", "60", "--out", dest], check=True)
    assert len(list(dest.glob("frame_*.png"))) == 120
    checkpoint(variant, dest.name + "-meta.json", (dest / "meta.json").read_bytes(),
               f"Record {variant} {args.scene} {args.size} reference clip")
