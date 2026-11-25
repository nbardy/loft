"""Reproduce moving-camera profiles and lossless comparison clips on the pinned fixtures."""
import argparse
import gzip
import hashlib
import json
import math
import re
import shutil
import subprocess
from pathlib import Path

notes = Path(__file__).resolve().parent
root = notes.parents[1]
out = root / "out/fresh"
scenes = dict(walrus="walrus/tq", heron="heron/h02_hero_q3", otter="otter/hero_q3",
              ball="start/ball", base="start/otter")

def setup():
    source = out / "source"
    shutil.copytree(notes / "fixtures", source / "fixtures", dirs_exist_ok=True)
    library = source / "stdlib"
    if not library.is_symlink():
        library.symlink_to(root / "stdlib")
    for name in scenes.values():
        path = source / "fixtures" / (name + ".loft")
        text = path.read_text()
        match = re.search(r'eye:\s*\[([^\]]+)\]@World,\s*at:\s*\[([^\]]+)\]@World', text)
        eye, at = ([float(s) for s in v.split(",")] for v in match.groups())
        radius = math.hypot(eye[0]-at[0], eye[2]-at[2])
        text = text[:match.start()] + (
            f'radius: {radius:.10f}, height: {eye[1]-at[1]:.10f}, period: 24.0, '
            f'at: [{match[2]}]@World') + text[match.end():]
        path.write_text(text)

def checkpoint(phase, filename, data, message):
    """Preserve completed measurements even when a later capture is interrupted."""
    dest = notes / "orbit" / phase / (filename + ".gz")
    dest.parent.mkdir(parents=True, exist_ok=True)
    content = gzip.compress(data, mtime=0)
    if dest.exists() and dest.read_bytes() == content:
        return
    dest.write_bytes(content)
    subprocess.run(["git", "add", "--", dest], cwd=root, check=True)
    subprocess.run(["git", "commit", "-m", message, "--", dest], cwd=root, check=True)


def run(phase, names, sizes, clip, record):
    folder = out / phase
    folder.mkdir(parents=True, exist_ok=True)
    if phase == "proxy":
        binary = out / "revised/proxy-study"
    elif phase == "guard":
        binary = out / "checked/loft"
    else:
        binary = folder / "loft"
    # A retry and the matching lossless clip must run exactly the same executable.
    # All capture calls writing this manifest hold the exclusive GPU slot.
    digest = hashlib.sha256(binary.read_bytes()).hexdigest()
    manifest = folder / "render-build.sha256"
    if manifest.exists() and manifest.read_text().strip() != digest:
        raise ValueError(f"{phase} already contains a different render build")
    manifest.write_text(digest + "\n")
    for size in sizes:
        for name in names:
            scene = out / "source/fixtures" / (scenes[name] + ".loft")
            prefix = folder / f"{name}-{size}"
            if phase == "proxy":
                if not (prefix / "frame_119.png").exists():
                    subprocess.run([binary, scene, *size.split("x"), prefix], check=True)
                if record:
                    for path in [prefix / "bounds.json", *sorted(prefix.glob("profile-*.json"))]:
                        checkpoint(phase, prefix.name + "-" + path.name, path.read_bytes(),
                                   f"Record direct proxy {prefix.name} {path.name}")
                continue
            if clip:
                dest = prefix.with_name(prefix.name + "-clip")
                if not (dest / "meta.json").exists():
                    subprocess.run([binary,"clip",scene,"--size",size,"--frames","120","--fps","60","--out",dest],check=True)
                assert len(list(dest.glob("frame_*.png"))) == 120
                if record:
                    checkpoint(phase, dest.name + "-meta.json", (dest / "meta.json").read_bytes(),
                               f"Record {phase} {prefix.name} moving clip")
                continue
            for attempt in range(2):
                dest = prefix.with_name(f"{prefix.name}-{attempt}.json")
                if not dest.exists():
                    subprocess.run([binary,"profile",scene,"--size",size,"--frames","120","--json",dest],check=True)
                d = json.loads(dest.read_text())
                vis = next(i for i,p in enumerate(d["passes"]) if p.startswith("Vis("))
                assert len(d["gpu_pass_ms"]) == 120 and all(t[vis] > 0 for t in d["gpu_pass_ms"])
                t = sorted(d["gpu_frame_ms"])
                print(name,size,attempt,t[59],t[113],d["contended"],flush=True)
                if record:
                    checkpoint(phase, dest.name, dest.read_bytes(),
                               f"Measure {phase} {prefix.name} attempt {attempt}: p95 {t[113]:.2f} ms")
                if not d["contended"]:
                    break

if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("phase")
    parser.add_argument("--scenes", nargs="+", default=["walrus","heron","otter"], choices=scenes)
    parser.add_argument("--sizes", nargs="+", default=["960x720","1280x800"])
    parser.add_argument("--clip", action="store_true")
    parser.add_argument("--record", action="store_true", help="commit completed measurements on this lane")
    args = parser.parse_args()
    if args.phase == "setup":
        setup()
    else:
        run(args.phase, args.scenes, args.sizes, args.clip, args.record)
