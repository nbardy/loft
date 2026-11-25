"""One load-control-bracketed block per candidate at the actual game times."""
import argparse
import json
import subprocess
from fresh import checkpoint, out, scenes

parser=argparse.ArgumentParser()
parser.add_argument("size")
parser.add_argument("--phases", nargs="+", choices=("original","revised","scissor","tiles"),
                    default=("original","revised","scissor"))
args=parser.parse_args()
folder=out/"warm-device"
folder.mkdir(exist_ok=True)
for scene in ("heron","walrus","otter"):
    name=scenes[scene].split("/")[-1]
    for phase in args.phases:
        path=out/phase/f"check-{name}.ir" if phase in ("original","revised") else out/"source/fixtures"/(scenes[scene]+".loft")
        dest=folder/f"{phase}-{scene}-{args.size}.json"
        if not dest.exists():
            executable = (out/"tiles" if phase=="tiles" else folder)/"fresh_blocks"
            subprocess.run([executable,path,*args.size.split("x"),dest],check=True)
        runs=json.loads(dest.read_text())
        selected=next((r for r in runs if not r["contended"]),runs[0])
        p95=sorted(selected["gpu_frame_ms"])[113]
        checkpoint("warm-device",dest.name,dest.read_bytes(),f"Measure warm-device {phase} {scene} {args.size} p95 {p95:.2f} ms")
