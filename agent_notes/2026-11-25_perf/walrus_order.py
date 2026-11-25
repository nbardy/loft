"""Hold projected geometry and the draw order fixed across the two native builds."""
import argparse
import subprocess

from fresh import checkpoint, notes, out

parser = argparse.ArgumentParser()
parser.add_argument("size")
args = parser.parse_args()
source = out / "source/fixtures/walrus/tq.loft"
fixed = source.with_name("fixed_order.loft")
body = (notes.parents[1] / "stdlib/core.loft").read_text().split("pub schedule cull_indirect", 1)[1].split("\n}\n", 1)[0]
choice = 'if drawn == 1 { "core/cull_single.wgsl" } else { "core/cull_multi.wgsl" }'
assert choice in body and source.read_text().count("cull_indirect(") == 1
fixed.write_text(source.read_text().replace("cull_indirect(", "cull_ordered(")
                 + "\nschedule cull_ordered" + body.replace(choice, '"core/cull_single.wgsl"') + "\n}\n")
for variant, executable, path in [
    ("repeat-original", "original", source),
    ("ordered-original", "original", fixed),
    ("ordered-guard", "revised", fixed),
]:
    dest = out / variant / f"walrus-{args.size}-clip"
    if not (dest / "meta.json").exists():
        subprocess.run([out / executable / "loft", "clip", path, "--size", args.size,
                        "--frames", "120", "--fps", "60", "--out", dest], check=True)
    assert len(list(dest.glob("frame_*.png"))) == 120
    checkpoint(variant, dest.name + "-meta.json", (dest / "meta.json").read_bytes(),
               f"Record {variant} walrus {args.size} reference clip")
