"""Repeat the native AB profile with its independent controls on the same device."""
import argparse
import json
import subprocess
from fresh import checkpoint, out, scenes

parser = argparse.ArgumentParser()
parser.add_argument("size")
parser.add_argument("phases", nargs="+")
args = parser.parse_args()
for name in ("heron", "walrus", "otter"):
    for phase in args.phases:
        folder = out / (phase + "-ab")
        folder.mkdir(exist_ok=True)
        path = out / "source/fixtures" / (scenes[name] + ".loft")
        for i in range(2):
            file = folder / f"{name}-{args.size}-{i}.json"
            if not file.exists():
                subprocess.run([out / phase / "loft", "profile", path, "--size", args.size,
                                "--frames", "120", "--timestamps", "ab", "--json", file], check=True)
            data = json.loads(file.read_text())
            for mode, run in data.items():
                vis = next(k for k,p in enumerate(run["passes"]) if p.startswith("Vis("))
                assert len(run["gpu_frame_ms"]) == 120 and min(v[vis] for v in run["gpu_pass_ms"]) > 0
            ring = data["ring"]
            tail = sorted(ring["gpu_frame_ms"])[113]
            checkpoint(phase+"-ab", file.name, file.read_bytes(),
                       f"Record {phase} {name} {args.size} ring p95 {tail:.2f} ms")
            if not ring["contended"]:
                break
