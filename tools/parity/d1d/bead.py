#!/usr/bin/env -S uv run --script
# /// script
# dependencies = ["pillow"]
# ///
"""Render and score the authored observation against the frozen R0g frames."""

import argparse
import csv
import hashlib
import json
from pathlib import Path
import subprocess

from PIL import Image, ImageChops


def compare(ref, frame, cli, dst, transects):
    score = subprocess.run([cli, "diff", ref, frame], capture_output=True, text=True, check=False)
    if score.returncode not in (0, 3):
        raise RuntimeError(score.stderr)
    with Image.open(ref) as old, Image.open(frame) as new:
        if old.mode != "RGBA" or new.mode != "RGBA" or old.size != new.size:
            raise ValueError(f"expected matching lossless RGBA for {ref.name}")
        delta = ImageChops.difference(old, new)
        channels = [sum(channel.histogram()[2:]) for channel in delta.split()]
        if transects:
            # The through view has its outer edge near the top row and the
            # bore across the center. The whole view shows both at different heights.
            with (dst / f"{ref.stem}-transects.csv").open("x") as output:
                writer = csv.writer(output)
                writer.writerow(["x", "y", "reference_r", "reference_g", "reference_b", "reference_depth",
                                 "candidate_r", "candidate_g", "candidate_b", "candidate_depth"])
                for y in (0, 128, 256):
                    for x in range(old.width):
                        writer.writerow([x, y, *old.getpixel((x, y)), *new.getpixel((x, y))])
    return {
        "picture": json.loads(score.stdout),
        "channels_over_one": dict(zip(("red", "green", "blue", "depth"), channels)),
        "reference_sha256": hashlib.sha256(ref.read_bytes()).hexdigest(),
        "candidate_sha256": hashlib.sha256(frame.read_bytes()).hexdigest(),
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("mode", choices=["initial", "full"])
    parser.add_argument("binary", type=Path)
    parser.add_argument("cli", type=Path, help="loft executable from this workspace")
    parser.add_argument("out", type=Path)
    args = parser.parse_args()
    refs = Path("agent_notes/2026-11-19_design/render_impl/R0g")
    dst = args.out
    dst.mkdir(parents=True, exist_ok=False)
    names = list(sorted(refs.glob("*.png")))
    if len(names) != 9:
        raise ValueError("expected all nine frozen bead frames")
    initial = args.mode == "initial"
    if initial:
        names = [name for name in names if name.name.endswith("@0@512x512.png")]
    cases = {}
    for ref in names:
        view, time, size = ref.stem.split("@")
        frame = dst / ref.name
        cmd = [args.binary.resolve(), "--observe", "corpus/l5c/bead.loft", size, time, frame,
               {"whole": "view=0", "through": "view=1"}[view],
               "lib/migration/trace_coverage.loft", "lib/migration/bead_observation.loft"]
        render = subprocess.run(cmd, capture_output=True, text=True, check=False)
        (dst / f"{ref.stem}.log").write_text(render.stdout + render.stderr)
        if render.returncode:
            cases[ref.stem] = {"render_exit": render.returncode, "passed": False}
            continue
        cases[ref.stem] = compare(ref, frame, args.cli.resolve(), dst, initial)
        cases[ref.stem]["passed"] = cases[ref.stem]["picture"]["pass"]
    with (dst / "score.json").open("x") as output:
        json.dump(cases, output, indent=2)
        output.write("\n")
    passed = sum(row["passed"] for row in cases.values())
    print(f"{passed}/{len(cases)} bead frames passed the frozen rule")
    return 0 if passed == len(cases) else 1


if __name__ == "__main__":
    raise SystemExit(main())
