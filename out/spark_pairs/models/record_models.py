#!/usr/bin/env python3
"""Round-robin ballot over distinct base models.

Reads out/spark_pairs/models/<name>/model.png (one `loft still` each) and
writes, per matchup, out/spark_pairs/<pair-id>/log.json in the aligner input
schema, plus out/spark_pairs/models/steps.jsonl in the _logger schema. Run
align_pairs.py afterwards to rebuild votes.json.

Pair ids look like loft3d-m-moss-archer-v-rust-knight (kind "model").
"""
import hashlib
import json
import os
import subprocess
import sys
from datetime import datetime, timezone

from PIL import Image

ROOT = "out/spark_pairs"
MODELS = os.path.join(ROOT, "models")
TS = datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")
AGENT = "loft-model-01"
FRAMING = "orbit still, subject centred, 1280x800 PNG"

NAMES = ["moss-archer", "rust-knight", "ember-imp", "frost-warden",
         "tide-priest", "dune-merchant", "kelp-diver", "ash-monk",
         "cloud-herder", "bone-collector"]


def fingerprint(path):
    h = hashlib.sha256()
    with open(path, "rb") as f:
        h.update(f.read())
    with Image.open(path) as img:
        size = list(img.size)
    return h.hexdigest(), size


def scene_check(name):
    scene = f"out/spark_pairs/models/{name}/model.loft"
    try:
        r = subprocess.run(["./target/debug/loft", "check", scene],
                           capture_output=True, text=True, timeout=120)
        lines = (r.stdout + r.stderr).strip().splitlines()
        return {"scene": scene, "exit": r.returncode,
                "summary": [l.strip() for l in lines if l.strip()][:4]}
    except Exception as e:  # noqa: BLE001 - recorded, never fatal
        return {"scene": scene, "exit": None, "summary": [f"check failed: {e}"]}


def prompt(name):
    return (f"Loft still render ({FRAMING}): base model {name}; "
            f"(scene out/spark_pairs/models/{name}/model.loft).")


def main():
    meta = {}
    for name in NAMES:
        rel = os.path.join(MODELS, name, "model.png")
        if not os.path.exists(rel):
            print(f"record-models: SKIP {name}: still missing", file=sys.stderr)
            continue
        sha, size = fingerprint(rel)
        meta[name] = {"render": rel, "prompt": prompt(name),
                      "sha256": sha, "size": size,
                      "check": scene_check(name)}
    names = sorted(meta)
    rows = [{"ts": TS, "agent": AGENT, "prompt": meta[n]["prompt"],
             "seed": None, "params": {"model": n,
                                      "scene_check": meta[n]["check"]["summary"]},
             "render_path": meta[n]["render"], "model_path": None,
             "note": f"base model {n} [sha256:{meta[n]['sha256'][:12]}]"}
            for n in names]
    with open(os.path.join(MODELS, "steps.jsonl"), "w") as f:
        for r in rows:
            f.write(json.dumps(r, sort_keys=True) + "\n")
    want = []
    for i in range(len(names)):
        for j in range(i + 1, len(names)):
            a, b = names[i], names[j]
            ma, mb = meta[a], meta[b]
            pid = f"loft3d-m-{a}-v-{b}"
            log = {
                "pair_id": pid,
                "criteria": (f"Which character reads stronger, {a} or {b}? "
                             f"Base models head to head. Judge overall appeal only."),
                "created_by": AGENT,
                "alignment": {"format": "png", "size": [1280, 800],
                              "framing": FRAMING},
                "renders": [
                    {"side": "a", "path": ma["render"],
                     "prompt": ma["prompt"],
                     "sha256": ma["sha256"], "size": ma["size"],
                     "params": {"model": a, "kind": "model"}},
                    {"side": "b", "path": mb["render"],
                     "prompt": mb["prompt"],
                     "sha256": mb["sha256"], "size": mb["size"],
                     "params": {"model": b, "kind": "model"}},
                ],
            }
            d = os.path.join(ROOT, pid)
            os.makedirs(d, exist_ok=True)
            with open(os.path.join(d, "log.json"), "w") as f:
                json.dump(log, f, indent=2, sort_keys=True)
                f.write("\n")
            want.append(pid)
    print(f"record-models: models={len(names)} pair_logs={len(want)}")


if __name__ == "__main__":
    main()
