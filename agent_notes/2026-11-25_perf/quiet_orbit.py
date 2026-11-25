"""One load-control-bracketed block per candidate at the actual game times."""
import argparse
import json
import subprocess
from fresh import checkpoint, out, scenes

parser=argparse.ArgumentParser()
parser.add_argument("size")
args=parser.parse_args()
folder=out/"warm-device"
folder.mkdir(exist_ok=True)
for scene in ("heron","walrus","otter"):
    name=scenes[scene].split("/")[-1]
    for phase in ("original","revised","scissor"):
        path=out/phase/f"check-{name}.ir" if phase!="scissor" else out/"source/fixtures"/(scenes[scene]+".loft")
        dest=folder/f"{phase}-{scene}-{args.size}.json"
        if not dest.exists():
            subprocess.run([folder/"fresh_blocks",path,*args.size.split("x"),dest],check=True)
        runs=json.loads(dest.read_text())
        selected=next((r for r in runs if not r["contended"]),runs[0])
        p95=sorted(selected["gpu_frame_ms"])[113]
        checkpoint("warm-device",dest.name,dest.read_bytes(),f"Measure warm-device {phase} {scene} {args.size} p95 {p95:.2f} ms")
