# /// script
# dependencies = ["pillow"]
# ///
"""Render available corpus candidates and assemble batch sheets outside the reference tree."""
import argparse
import json
from pathlib import Path
import subprocess
from PIL import Image, ImageDraw, ImageOps

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("--cli", type=Path)
parser.add_argument("--legacy", type=Path)
args = parser.parse_args()
root = Path(__file__).resolve().parents[1]
corpus = root / "corpus"
out = root / "out/meta/i5-fix"
approval = out / "approval"
photos = out / "stills"
for directory in [approval, photos]:
    directory.mkdir(parents=True, exist_ok=True)
manifest = json.loads((corpus / "manifest.json").read_text())
catalog = {p.stem: p for p in corpus.glob("l5[ced]/*.loft")}
catalog.update({p.stem: p for p in corpus.glob("seeds/batch-*/*.loft")})
catalog["examples__bead"] = catalog.pop("bead")
for short in ["lantern", "lantern_nothing"]:
    catalog[f"examples__first__{short}"] = catalog.pop(f"first__{short}")

def stem(identity):
    return identity.removesuffix(".ts").replace("/", "__")

batch_paths = [[catalog.get(stem(scene)) for scene in batch["scenes"]] for batch in manifest["batches"]]
(out / "candidate-scenes.txt").write_text("".join(str(p.relative_to(root)) + "\n" for paths in batch_paths for p in paths if p))
if args.cli is None:
    raise SystemExit(0)
reports = []
for batch, paths in zip(manifest["batches"], batch_paths):
    if not any(paths):
        continue
    sheet = Image.new("RGB", (1000, 244 * ((len(paths) + 4) // 5)), "#171717")
    label = ImageDraw.Draw(sheet)
    for i, (identity, path) in enumerate(zip(batch["scenes"], paths)):
        x, y = (i % 5) * 200, (i // 5) * 244
        status = "No candidate"
        if path:
            render_dir = photos / stem(identity)
            photo = render_dir / "frame_000.png"
            proc = subprocess.run([str(args.cli.resolve()), "clip", str(path), "--out", str(render_dir), "--size", "512x512", "--frames", "1", "--fps", "16"], capture_output=True, cwd=root, text=True)
            status = "Render failed" if proc.returncode else "Owner review"
            reports.append({"scene": identity, "file": str(path.relative_to(root)), "exit": proc.returncode, "log": proc.stdout + proc.stderr})
            if proc.returncode == 0:
                with Image.open(photo) as image:
                    sheet.paste(ImageOps.contain(image.convert("RGB"), (194, 194)), (x + 3, y + 3))
                    if args.legacy:
                        prefix = "" if identity.startswith("examples/") else "seed__"
                        old_dir = args.legacy / f"{prefix}{stem(identity)}"
                        old = sorted(old_dir.glob("*@0@512x512.png"))
                        if identity == "examples/bead.ts":
                            old = [old_dir / "whole@0@512x512.png"]
                        if old and old[0].is_file():
                            with Image.open(old[0]) as reference:
                                reference.save(approval / f"{stem(identity)}_old.webp", quality=95)
                            image.save(approval / f"{stem(identity)}_new.webp", quality=95)
        short = identity.removeprefix("examples/").removeprefix("crab-actions-v1/").removeprefix("crab-improve-v1/")
        label.text((x + 5, y + 202), short, fill="white")
        label.text((x + 5, y + 220), status, fill="#cccccc")
    sheet.save(approval / f"batch-{batch['batch']:02}.webp", quality=95)
(out / "approval-renders.json").write_text(json.dumps(reports, indent=2) + "\n")
if any(report["exit"] for report in reports):
    raise SystemExit(1)
