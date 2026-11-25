"""One contact sheet from a folder of scenes, as one GPU job, with a trail of every render.

    SHEET_TOPIC=boss uv run --no-project --with pillow tools/render_sheet.py DIR WORKER ROUND

Renders every scene in DIR (a .loft file with a `schedule`; sibling modules are libraries and are skipped) at
960x720, one frame each, with the primary checkout's release binary. Frames are cached by the hash of the scene
and every other .loft file in DIR, so a rerun renders only what changed. Writes, under the primary checkout's
out/meta/TOPIC/WORKER/ (a worktree's own out/ disappears with the worktree):

    ROUND/<scene>.webp      each still, q95, and ROUND/<scene>.loft, the program that made it
    ROUND/_lib/             the sibling modules as they were for this round
    ROUND.webp              the labelled sheet, four columns
    view/ROUND.webp         the same sheet 1600 px wide: look at this one
    trail.jsonl             one line per render: time, round, scene, label, parent, hash, paths, frame time

A scene's first line is its label (`# what this variant changes`) and its second names what it was edited from
(`# parent: ROUND/scene`, or `# parent: none`). With those two lines the trail of kept and rejected variants
needs nothing else from the author.

Environment:
    SHEET_TOPIC   folder under out/meta (default `sheets`)
    SHEET_REF     a picture shown as the first tile, such as a reference photograph (default none)
    SHEET_AOVS    comma list of the renderer's channel views added after each scene's tile: albedo and normal show
                  the base colour and the surface normal without light (also depth, id, sdf_steps,
                  lipschitz_ratio, overdraw). Roughness, material masks and a normal tilt applied inside a
                  lobe have no view: render those from the scene as flat colour.

The time on a tile is ONE COLD FRAME: it flags a scene that is far too slow and is not a measurement. Frame
time means the warmed median and p95 from `loft profile SCENE --size WxH --frames 120 --json OUT`, whose JSON
also gives the time per pass and says whether the run was contended.

The script takes the GPU slot itself (agent-build --gpu). Do not wrap it and do not run two at once.
"""
import hashlib
import json
import os
import shutil
import subprocess
import sys
import time
from pathlib import Path

LOFT = Path.home() / "git/loft"  # the primary checkout: its binary reads its own stdlib, and its out/ survives worktrees
BIN = LOFT / "target/release/loft"
ROOT = LOFT / "out/meta" / os.environ.get("SHEET_TOPIC", "sheets")
REF = os.environ.get("SHEET_REF", "none")
AOVS = [a for a in os.environ.get("SHEET_AOVS", "").split(",") if a]

if len(sys.argv) != 4:
    sys.exit(__doc__)
src_dir, worker, rnd = Path(sys.argv[1]).resolve(), sys.argv[2], sys.argv[3]
if not os.environ.get("RENDER_SHEET_IN_SLOT"):
    os.environ["RENDER_SHEET_IN_SLOT"] = "1"
    cmd = [str(Path.home() / ".local/bin/agent-build"), "--gpu", "--max-wait", "90", "--max-minutes", "10",
           f"sheet {worker}/{rnd}", sys.executable, *sys.argv]
    sys.exit(subprocess.run(cmd).returncode)

from PIL import Image, ImageDraw  # noqa: E402


def clip(scene, out_dir, aov, timing=None):
    cmd = [str(BIN), "clip", str(scene), "--out", str(out_dir), "--size", "960x720", "--frames", "1", "--fps", "1", "--start", "0",
           "--aov", aov, "--max-build-ms", "20000", "--max-frame-ms", "400"] + (["--json", str(timing)] if timing else [])
    return subprocess.run(cmd, cwd=src_dir, capture_output=True, text=True)


files = sorted(src_dir.glob("*.loft"))
scenes = [f for f in files if "\nschedule " in f.read_text()]
if not scenes:
    sys.exit(f"no scenes (a .loft with a schedule) in {src_dir}")
out = ROOT / worker / rnd
out.mkdir(parents=True, exist_ok=True)
work = ROOT / "work"
tiles, bad = ([] if REF == "none" else [(Path(REF).expanduser(), "reference " + Path(REF).stem)]), 0
trail = ROOT / worker / "trail.jsonl"
seen = {(r["round"], r["scene"], r["sha"]) for r in map(json.loads, trail.read_text().splitlines())} if trail.exists() else set()
libraries = [f for f in files if f not in scenes]
if libraries:
    (out / "_lib").mkdir(exist_ok=True)
    for f in libraries:
        shutil.copyfile(f, out / "_lib" / f.name)
for s in scenes:
    h = hashlib.sha256(s.read_bytes())
    for f in libraries:
        h.update(f.read_bytes())
    sha = h.hexdigest()[:12]
    d = work / f"{s.stem}-{sha}"
    d.mkdir(parents=True, exist_ok=True)
    png, status = d / "frame_000.png", "ok"
    if not png.exists():
        r = subprocess.run([str(BIN), "check", str(s)], cwd=src_dir, capture_output=True, text=True)
        if r.returncode:
            status = "CHECK FAIL " + (r.stdout + r.stderr)[-500:]
        else:
            r = clip(s, d, "beauty", d / "timing.json")
            if r.returncode or not png.exists():
                status = "CLIP FAIL " + (r.stdout + r.stderr)[-500:]
    ms = -1.0
    if status == "ok":
        Image.open(png).convert("RGB").save(out / f"{s.stem}.webp", quality=95)
        t = d / "timing.json"
        ms = json.loads(t.read_text()).get("frame_gpu_p95_ms", -1.0) if t.exists() else -1.0
    else:
        bad += 1
    head = s.read_text().splitlines()[:2] + ["", ""]
    label = head[0].lstrip("# ").strip()[:84]
    parent = head[1].lstrip("# ").strip()[7:].strip() if head[1].lstrip("# ").startswith("parent:") else None
    shutil.copyfile(s, out / f"{s.stem}.loft")
    if (rnd, s.stem, sha) not in seen:
        with trail.open("a") as t:
            t.write(json.dumps(dict(time=time.strftime("%Y-%m-%dT%H:%M:%S"), round=rnd, scene=s.stem, label=label, parent=parent,
                                    sha=sha, loft=str(out / f"{s.stem}.loft"), image=str(out / f"{s.stem}.webp") if status == "ok" else None,
                                    gpu_ms=round(ms, 1), status="ok" if status == "ok" else status[:200])) + "\n")
    tiles.append((out / f"{s.stem}.webp", f"{s.stem}: {label}  [cold {ms:.0f} ms]" if status == "ok" else f"{s.stem}: FAILED"))
    for aov in AOVS if status == "ok" else []:
        da = work / f"{s.stem}-{sha}-{aov}"
        da.mkdir(parents=True, exist_ok=True)
        if not (da / "frame_000.png").exists():
            clip(s, da, aov)
        if (da / "frame_000.png").exists():
            Image.open(da / "frame_000.png").convert("RGB").save(out / f"{s.stem}.{aov}.webp", quality=95)
        tiles.append((out / f"{s.stem}.{aov}.webp", f"{s.stem}: {aov}"))
    print(f"{s.stem:24s} {ms:6.1f} ms  {' '.join(status.split())}")

W, H, LAB, COLS = 640, 480, 22, 4
rows = -(-len(tiles) // COLS)
sheet = Image.new("RGB", (COLS * (W + 4) - 4, rows * (H + LAB + 4) - 4), (14, 14, 14))
draw = ImageDraw.Draw(sheet)
for i, (path, text) in enumerate(tiles):
    x, y = (i % COLS) * (W + 4), (i // COLS) * (H + LAB + 4)
    if Path(path).exists():
        im = Image.open(path).convert("RGB")
        k = min(W / im.width, H / im.height)
        im = im.resize((round(im.width * k), round(im.height * k)), Image.Resampling.LANCZOS)
        sheet.paste(im, (x + (W - im.width) // 2, y + LAB + (H - im.height) // 2))
    draw.text((x + 4, y + 5), text, fill=(235, 235, 235))
full = ROOT / worker / f"{rnd}.webp"
sheet.save(full, quality=92)
view = ROOT / worker / "view" / f"{rnd}.webp"
view.parent.mkdir(exist_ok=True)
sheet.resize((1600, round(sheet.height * 1600 / sheet.width)), Image.Resampling.LANCZOS).save(view, quality=88)
print("sheet", full)
print("view ", view)
index = LOFT / "out/meta/gallery/build_index.py"  # the owner's local gallery, when this machine has one
if index.exists():
    subprocess.run(["uv", "run", "--no-project", "python", str(index)], capture_output=True)
sys.exit(1 if bad else 0)
