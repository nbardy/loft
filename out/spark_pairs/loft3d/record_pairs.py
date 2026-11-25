#!/usr/bin/env python3
"""Record loft3d stills into the common pair format.

Reads out/spark_pairs/loft3d/<chain>/step_XX.png (real `loft still` renders)
and writes, per pair, out/spark_pairs/<pair-id>/log.json in the aligner input
schema (same as pair-01..04), plus out/spark_pairs/loft3d/steps.jsonl in the
_logger schema. Run align_pairs.py afterwards to rebuild votes.json.

A chain records only its verified-fresh prefix (steps 0..last); stale steps
never enter the ballot.
"""
import hashlib
import json
import os
import subprocess
import sys
from datetime import datetime, timezone

from PIL import Image

ROOT = "out/spark_pairs"
L3D = os.path.join(ROOT, "loft3d")
TS = datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")
AGENT = "loft-still-01"

CHAINS = [
    ("golem-shape", "shape",
     "mossy stone golem guardian, orbit camera, 1280x800 still",
     "Which golem reads as the friendlier forest guardian? Judge silhouette only.",
     "Body column slims while shoulders widen, step 0 round to step 10 slab."),
    ("golem-texture", "texture",
     "mossy stone golem guardian, orbit camera, 1280x800 still",
     "Which golem has better moss integration? Judge moss coverage only.",
     "Seeded moss tufts accrete 5 to 80 percent, nested across steps."),
    ("golem-expression", "expression",
     "mossy stone golem guardian face, orbit camera, 1280x800 still",
     "Which golem looks friendlier? Judge face and smile only.",
     "Brow relaxes and mouth corners rise, stern step 0 to beaming step 10."),
    ("solar-material", "material",
     "desert solar-scavenger robot torso, orbit camera, 1280x800 still",
     "Which robot reads more solar-scavenger? Judge rust vs mirror only.",
     "Mirror pauldron ribs accrete on rust shoulders, step 0 rust to step 10 mirror."),
    ("abyss-lure", "shape",
     "brass diver helmet with lure stalk, orbit camera, 1280x800 still",
     "Which lure reads better at thumbnail size? Judge stalk silhouette only.",
     "Lure bulb travels wide side arc to tall centre-top arc."),
    ("rune-legibility", "texture",
     "carved stone tablet with six glyphs, orbit camera, 1280x800 still",
     "Which tablet is more legible? Judge glyph clarity vs ornament only.",
     "Stems first, ornament branches accrete to full carving step 10."),
    ("lantern-color", "color",
     "paper lantern with glow core, orbit camera, 1280x800 still",
     "Which lantern feels cozier? Judge glow color and warmth only.",
     "Paper cools amber to moonlight while cool ribs accrete, step 0 to 10."),
]

FRAMING = "orbit still, subject centred, 1280x800 PNG"


def prompt(chain_id, subject, t):
    return (f"Loft still render ({FRAMING}): {subject}; "
            f"chain {chain_id} iteration step {t} "
            f"(scene out/spark_pairs/chains/{chain_id}/chain.loft, param step={t}).")


def fingerprint(path):
    h = hashlib.sha256()
    with open(path, "rb") as f:
        h.update(f.read())
    with Image.open(path) as img:
        size = list(img.size)
    return h.hexdigest(), size


def scene_check(cid):
    scene = f"out/spark_pairs/chains/{cid}/chain.loft"
    try:
        r = subprocess.run(["./target/debug/loft", "check", scene],
                           capture_output=True, text=True, timeout=120)
        lines = (r.stdout + r.stderr).strip().splitlines()
        return {"scene": scene, "exit": r.returncode,
                "summary": [l.strip() for l in lines if l.strip()][:4]}
    except Exception as e:  # noqa: BLE001 - recorded, never fatal
        return {"scene": scene, "exit": None, "summary": [f"check failed: {e}"]}


def main():
    steps_rows = []
    want = []
    for entry in CHAINS:
        cid, axis, subject, criteria, reflect = entry[:5]
        last = entry[5] if len(entry) > 5 else 40
        paths = [os.path.join(L3D, cid, f"step_{t:02d}.png")
                 for t in range(last + 1)]
        missing = [p for p in paths if not os.path.exists(p)]
        if missing:
            print(f"record: SKIP {cid}: {len(missing)} stills missing "
                  f"(e.g. {missing[0]})", file=sys.stderr)
            continue
        check = scene_check(cid)
        meta = []
        for t, rel in enumerate(paths):
            sha, size = fingerprint(rel)
            meta.append({"t": t, "render": rel,
                         "prompt": prompt(cid, subject, t),
                         "sha256": sha, "size": size})
            steps_rows.append({
                "ts": TS, "agent": AGENT, "prompt": prompt(cid, subject, t),
                "seed": None, "params": {"chain": cid, "t": t,
                                         "scene": check["scene"],
                                         "scene_check": check["summary"]},
                "render_path": rel, "model_path": None,
                "note": f"t={t}/{last} {axis}: {reflect} "
                        f"[sha256:{sha[:12]}]"})
        seen = set()

        def add(a, b, kind):
            if (a, b) in seen:
                return
            seen.add((a, b))
            pid = f"loft3d-{cid}-{kind}-{a:02d}v{b:02d}"
            ma, mb = meta[a], meta[b]
            log = {
                "pair_id": pid,
                "criteria": criteria,
                "created_by": AGENT,
                "alignment": {"format": "png", "size": [1280, 800],
                              "framing": FRAMING},
                "scene_check": check,
                "renders": [
                    {"side": "a", "path": ma["render"],
                     "prompt": ma["prompt"],
                     "sha256": ma["sha256"], "size": ma["size"],
                     "params": {"chain": cid, "t": a, "kind": kind}},
                    {"side": "b", "path": mb["render"],
                     "prompt": mb["prompt"],
                     "sha256": mb["sha256"], "size": mb["size"],
                     "params": {"chain": cid, "t": b, "kind": kind}},
                ],
            }
            d = os.path.join(ROOT, pid)
            os.makedirs(d, exist_ok=True)
            with open(os.path.join(d, "log.json"), "w") as f:
                json.dump(log, f, indent=2, sort_keys=True)
                f.write("\n")
            want.append(pid)

        for t in range(last):
            add(t, t + 1, "adjacent")
        for t in range(1, last + 1):
            add(0, t, "baseline")
    with open(os.path.join(L3D, "steps.jsonl"), "w") as f:
        for r in steps_rows:
            f.write(json.dumps(r, sort_keys=True) + "\n")
    print(f"record: steps={len(steps_rows)} pair_logs={len(want)}")


if __name__ == "__main__":
    main()
