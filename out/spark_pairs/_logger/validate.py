#!/usr/bin/env python3
"""Validate out/spark_pairs/_logger/logger.json (JSONL) against schema.json.

Usage (from repo root):
    python3 out/spark_pairs/_logger/validate.py

Checks per row: one JSON object per non-blank line; required keys
ts/agent/prompt/render_path; types for seed/params/model_path/note;
no unknown keys; render_path and model_path (when set) exist on disk.
Exit 0 when all rows pass, 1 otherwise.
"""
import json
import os
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__)))))
# validate.py lives at out/spark_pairs/_logger/validate.py -> repo root is 4 levels up.
# Fall back to cwd if layout differs.
if not os.path.isdir(os.path.join(ROOT, "out")):
    ROOT = os.getcwd()

LOG = os.path.join(ROOT, "out", "spark_pairs", "_logger", "logger.json")
ALLOWED = {"ts", "agent", "prompt", "seed", "params", "render_path", "model_path", "note"}
REQUIRED = ("ts", "agent", "prompt", "render_path")


def fail(msg):
    print(f"INVALID: {msg}")
    return False


def main():
    ok = True
    try:
        with open(LOG, encoding="utf-8") as f:
            lines = f.read().splitlines()
    except FileNotFoundError:
        print(f"INVALID: log file not found: {LOG}")
        return 1
    rows = [(i + 1, ln) for i, ln in enumerate(lines) if ln.strip()]
    if not rows:
        print("INVALID: log file has no rows")
        return 1
    for lineno, ln in rows:
        try:
            row = json.loads(ln)
        except json.JSONDecodeError as e:
            ok = fail(f"line {lineno}: not valid JSON ({e})") and ok
            continue
        if not isinstance(row, dict):
            ok = fail(f"line {lineno}: row must be a JSON object") and ok
            continue
        unknown = set(row) - ALLOWED
        if unknown:
            ok = fail(f"line {lineno}: unknown keys {sorted(unknown)}") and ok
        for key in REQUIRED:
            if not isinstance(row.get(key), str) or not row[key].strip():
                ok = fail(f"line {lineno}: required key '{key}' must be a non-empty string") and ok
        if "seed" in row and row["seed"] is not None and not isinstance(row["seed"], int):
            ok = fail(f"line {lineno}: 'seed' must be integer or null") and ok
        if "params" in row and not isinstance(row["params"], dict):
            ok = fail(f"line {lineno}: 'params' must be an object") and ok
        if "model_path" in row and row["model_path"] is not None and not isinstance(row["model_path"], str):
            ok = fail(f"line {lineno}: 'model_path' must be a string or null") and ok
        if "note" in row and not isinstance(row["note"], str):
            ok = fail(f"line {lineno}: 'note' must be a string") and ok
        rp = row.get("render_path")
        if isinstance(rp, str) and rp.strip() and not os.path.isfile(os.path.join(ROOT, rp)):
            ok = fail(f"line {lineno}: render_path not found: {rp}") and ok
        mp = row.get("model_path")
        if isinstance(mp, str) and mp.strip() and not os.path.isfile(os.path.join(ROOT, mp)):
            ok = fail(f"line {lineno}: model_path not found: {mp}") and ok
    if ok:
        print(f"VALID: {len(rows)} rows in {os.path.relpath(LOG, ROOT)}")
        return 0
    return 1


if __name__ == "__main__":
    sys.exit(main())
