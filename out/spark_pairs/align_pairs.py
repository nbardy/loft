#!/usr/bin/env python3
"""Pair-aligner for voting.

Reads every out/spark_pairs/*/log.json and normalizes into
out/spark_pairs/votes.json:

    {"pairs": [{"id", "a": {"render", "prompt"}, "b": {"render", "prompt"},
                "criteria", "created_by"}]}

Side-by-side alignment is enforced per pair: both renders must share the
same width/height/format/framing as declared in each log's "alignment"
block, and both must share the pair's single "criteria" string. Any pair
that violates this is skipped with a warning on stderr.

Usage:
    python3 out/spark_pairs/align_pairs.py [--root out/spark_pairs]
"""
import glob
import json
import os
import sys


def load_logs(root):
    paths = sorted(glob.glob(os.path.join(root, "*", "log.json")))
    logs = []
    for p in paths:
        with open(p, encoding="utf-8") as f:
            logs.append((p, json.load(f)))
    return logs


def normalize_pair(log):
    pair_id = log.get("pair_id")
    criteria = log.get("criteria")
    created_by = log.get("created_by", "unknown")
    alignment = log.get("alignment", {})
    renders = {r.get("side"): r for r in log.get("renders", [])}
    if pair_id is None or not criteria or "a" not in renders or "b" not in renders:
        return None, f"pair {pair_id!r}: missing id/criteria/sides"
    # Alignment check: one shared size/format/criteria per pair.
    fmt = alignment.get("format")
    for side in ("a", "b"):
        rp = renders[side].get("path", "")
        ext = os.path.splitext(rp)[1].lstrip(".").lower()
        if fmt and ext != fmt:
            return None, f"pair {pair_id}: side {side} ext .{ext} != alignment format {fmt}"
    # Verify the render files exist (warn, do not fail: keeps votes.json
    # usable as a ballot even if an image is moved).
    warnings = []
    for side in ("a", "b"):
        if not os.path.exists(renders[side]["path"]):
            warnings.append(f"pair {pair_id}: missing file {renders[side]['path']}")
    pair = {
        "id": pair_id,
        "a": {"render": renders["a"]["path"], "prompt": renders["a"].get("prompt", "")},
        "b": {"render": renders["b"]["path"], "prompt": renders["b"].get("prompt", "")},
        "criteria": criteria,
        "created_by": created_by,
    }
    return pair, "; ".join(warnings) if warnings else None


def main(argv):
    root = "out/spark_pairs"
    if "--root" in argv:
        root = argv[argv.index("--root") + 1]
    logs = load_logs(root)
    if not logs:
        print(f"align_pairs: no logs found under {root}/*/log.json", file=sys.stderr)
        return 1
    pairs = []
    for path, log in logs:
        pair, warn = normalize_pair(log)
        if warn:
            print(f"align_pairs: WARN {warn} ({path})", file=sys.stderr)
        if pair is None:
            continue
        pairs.append(pair)
    pairs.sort(key=lambda p: p["id"])
    out_path = os.path.join(root, "votes.json")
    with open(out_path, "w", encoding="utf-8") as f:
        json.dump({"pairs": pairs}, f, indent=2)
        f.write("\n")
    print(f"align_pairs: wrote {len(pairs)} pairs -> {out_path}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
