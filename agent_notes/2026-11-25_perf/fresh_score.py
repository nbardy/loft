"""Score orbit profiles and corresponding lossless beauty frames against the original."""
# /// script
# dependencies = ["pillow", "numpy"]
# ///
import argparse
from hashlib import sha256
import json
from pathlib import Path

import numpy as np
from PIL import Image

from fresh import checkpoint, commit_path, notes, out, scenes


def profile(folder, stem, probe):
    attempts = sorted((folder / stem).glob("profile-*.json") if probe else folder.glob(f"{stem}-[0-9].json"))
    if not attempts:
        return None
    values = [json.loads(p.read_text()) for p in attempts]
    index = next((i for i, d in enumerate(values) if not d["contended"]), 0)
    d = values[index]
    frames = d["gpu_frame_ms"]
    assert len(frames) == len(d["gpu_pass_ms"]) == 120
    vis = next(i for i, p in enumerate(d["passes"]) if p.startswith("Vis("))
    assert min(t[vis] for t in d["gpu_pass_ms"]) > 0
    median_index = sorted(range(120), key=frames.__getitem__)[59]
    return dict(selected=attempts[index].name, adapter=d["adapter"], contended=d["contended"],
                brackets=[[v["load_control_before_p50_ms"], v["load_control_after_p50_ms"]] for v in values],
                frame_p50=frames[median_index], frame_p95=sorted(frames)[113],
                median_frame_passes=dict(zip(d["passes"], d["gpu_pass_ms"][median_index])))


def decoded(path):
    with Image.open(path) as im:
        return np.asarray(im.convert("RGBA"))


def compare(before, after, size, diff_path):
    frames = [f"frame_{i:03}.png" for i in range(120)]
    assert all(sorted(p.name for p in folder.glob("frame_*.png")) == frames for folder in [before, after])
    hashes, counts = [], []
    worst = (0, 0)
    for i, name in enumerate(frames):
        a = decoded(before / name)
        b = decoded(after / name)
        assert a.shape == b.shape == (size[1], size[0], 4)
        digest = [sha256(im).hexdigest() for im in [a, b]]
        hashes.append(digest)
        if digest[0] == digest[1]:
            counts.append(0)
            continue
        delta = np.abs(a.astype(np.int16) - b)
        count = int(np.count_nonzero(np.any(delta, axis=2)))
        counts.append(count)
        score = (int(delta.max()), count)
        if score > worst:
            worst = score
            worst_frame = i
            rgb = np.maximum(delta[:, :, :3], delta[:, :, 3:4])
            diff = Image.fromarray(np.minimum(255, 16 * rgb).astype(np.uint8))
    if worst[0]:
        diff_path.parent.mkdir(exist_ok=True, parents=True)
        diff.save(diff_path)
    return dict(exact=counts.count(0), max_channel=worst[0], max_pixels=max(counts),
                total_pixel_differences=sum(counts), counts=counts, rgba_sha256=hashes,
                worst_frame=worst_frame if worst[0] else None, diff=diff_path.name if worst[0] else None)


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("phase")
    parser.add_argument("--reference", default="original", help="phase containing reference clips")
    parser.add_argument("--record", action="store_true")
    args = parser.parse_args()
    phase = args.phase
    probe = phase == "proxy"
    folder = out / phase
    report = {}
    for res in ["960x720", "1280x800"]:
        for name in scenes:
            stem = f"{name}-{res}"
            result = profile(folder, stem, probe)
            before = out / args.reference / (stem + "-clip")
            after = folder / (stem if probe else stem + "-clip")
            if phase != "original" and before.exists() and (after / "frame_119.png").exists():
                result = result or {}
                diff = (notes / "orbit" / phase if args.record else out / "diffs") / f"{phase}-{stem}.png"
                result["pixels"] = compare(before, after, tuple(map(int, res.split("x"))), diff)
                result["reference"] = args.reference
                if args.record:
                    if result["pixels"]["diff"]:
                        commit_path(diff, f"Save {phase} {stem} worst-frame difference")
                    checkpoint(phase, stem + "-score.json", json.dumps(result, sort_keys=True).encode(),
                               f"Score {phase} {stem}: {result['pixels']['exact']}/120 exact moving frames")
            if result:
                report[stem] = result
                summary = result.get("pixels", {})
                print(stem, result.get("frame_p50"), result.get("frame_p95"), result.get("contended"),
                      summary.get("exact"), summary.get("max_channel"), summary.get("max_pixels"), flush=True)
    (folder / "score.json").write_text(json.dumps(report, indent=2))
