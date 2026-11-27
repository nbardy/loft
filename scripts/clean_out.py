#!/usr/bin/env python3
"""Responsible disk cleanup for loft.

Preserves all source code, .loft files, JSON/Markdown/log experiment reports,
and recent outputs. Reclaims regenerable heavy render/build outputs under out/
(raw .rgba/.f32/.u32/.bin/.rgba16f/.rg32u/.grid buffers, duplicated out/**/bin/
executables, check.* scratch dirs, and stale PNG/WebP frames) plus stale
unreferenced .git/cambium baselines and layers.

Defaults to --dry-run. Pass --apply to execute.
"""
from __future__ import annotations

import argparse
import glob
import json
import os
import shutil
import time
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent
RAW_BUFFER_EXTS = {".rgba", ".f32", ".u32", ".bin", ".rgba16f", ".rg32u", ".grid"}
FRAME_EXTS = {".png", ".webp", ".ppm", ".exr"}


def dir_bytes(path: Path) -> int:
    if path.is_file() or path.is_symlink():
        try:
            return path.stat().st_size
        except OSError:
            return 0
    total = 0
    for root, _, files in os.walk(path):
        for fn in files:
            try:
                total += os.path.getsize(os.path.join(root, fn))
            except OSError:
                pass
    return total


def fmt(n: int) -> str:
    for unit in ("B", "KB", "MB", "GB"):
        if n < 1024 or unit == "GB":
            return f"{n:.2f} {unit}" if unit != "B" else f"{n} B"
        n /= 1024.0
    return f"{n:.2f} GB"


def has_keep_marker(path: Path, stop: Path) -> bool:
    cur = path if path.is_dir() else path.parent
    while cur != stop and cur != cur.parent:
        if (cur / ".keep").exists() or (cur / "retention.json").exists():
            return True
        cur = cur.parent
    return False


def collect_out(older_than_hours: float, include_frames: bool) -> list[tuple[str, Path, int]]:
    out_root = REPO / "out"
    if not out_root.is_dir():
        return []
    cutoff = time.time() - older_than_hours * 3600
    actions: list[tuple[str, Path, int]] = []

    for child in sorted(out_root.iterdir()):
        if has_keep_marker(child, out_root):
            continue
        # Scratch check.* or legacy-build-cache dirs older than cutoff
        if child.is_dir() and (child.name.startswith("check.") or child.name == "legacy-build-cache"):
            try:
                if child.stat().st_mtime <= cutoff:
                    actions.append(("scratch-dir", child, dir_bytes(child)))
                    continue
            except OSError:
                continue

        if not child.is_dir():
            continue

        for root, dirs, files in os.walk(child):
            rpath = Path(root)
            if has_keep_marker(rpath, out_root):
                dirs.clear()
                continue
            # Duplicated compiled binaries inside out/**/bin/
            if "bin" in dirs:
                bdir = rpath / "bin"
                try:
                    if bdir.stat().st_mtime <= cutoff:
                        actions.append(("out-bin", bdir, dir_bytes(bdir)))
                        dirs.remove("bin")
                except OSError:
                    pass
            for fn in files:
                fpath = rpath / fn
                ext = fpath.suffix.lower()
                is_raw = ext in RAW_BUFFER_EXTS
                is_frame = include_frames and ext in FRAME_EXTS
                is_dup_elf = ext == "" and fn in ("loft", "probe", "bench", "viewer")
                if not (is_raw or is_frame or is_dup_elf):
                    continue
                try:
                    st = fpath.stat()
                except OSError:
                    continue
                if st.st_mtime <= cutoff:
                    kind = "raw-buffer" if is_raw else ("dup-binary" if is_dup_elf else "render-frame")
                    actions.append((kind, fpath, st.st_size))
    return actions


def collect_cambium(age_hours: float) -> list[tuple[str, Path, int]]:
    cambium = REPO / ".git/cambium"
    if not cambium.is_dir():
        return []
    ws_refs: set[str] = set()
    for wf in glob.glob(str(cambium / "workspace-meta/*.json")):
        try:
            d = json.load(open(wf))
            if d.get("base_commit"):
                ws_refs.add(d["base_commit"])
            for lid in d.get("layer_ids") or []:
                ws_refs.add(lid)
        except Exception:
            pass
    cutoff = time.time() - age_hours * 3600
    actions: list[tuple[str, Path, int]] = []
    for sub in ("baselines", "layers"):
        root = cambium / sub
        if not root.is_dir():
            continue
        for entry in root.iterdir():
            if entry.name in ws_refs:
                continue
            try:
                if entry.stat().st_mtime > cutoff:
                    continue
            except OSError:
                continue
            actions.append((f"cambium-{sub[:-1]}", entry, 0))
    return actions


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--apply", action="store_true", help="Actually delete selected regenerable items")
    ap.add_argument("--scheduled", action="store_true", help="Conservative defaults for automated daily cleanup")
    ap.add_argument("--older-than-hours", type=float, default=24.0, help="Min age of out/ artifacts before pruning (default: 24h)")
    ap.add_argument("--keep-frames", action="store_true", help="Keep old PNG/WebP render frames; only remove raw buffers and duplicated binaries")
    ap.add_argument("--cambium-age-hours", type=float, default=2.0, help="Min age of unreferenced Cambium caches")
    args = ap.parse_args()

    if args.scheduled:
        args.older_than_hours = max(args.older_than_hours, 48.0)

    actions = collect_out(args.older_than_hours, include_frames=not args.keep_frames)
    actions.extend(collect_cambium(args.cambium_age_hours))

    by_kind: dict[str, tuple[int, int]] = {}
    for kind, _, sz in actions:
        cnt, b = by_kind.get(kind, (0, 0))
        by_kind[kind] = (cnt + 1, b + sz)

    total = sum(sz for _, _, sz in actions)
    mode = "APPLY" if args.apply else "DRY-RUN"
    print(f"[loft clean_out] mode={mode} items={len(actions)} est_reclaim={fmt(total)}")
    for kind, (cnt, b) in sorted(by_kind.items(), key=lambda x: -x[1][1]):
        print(f"  {kind:18s}: {cnt:6d} items, {fmt(b) if b else 'shared/cache'}")

    if args.apply:
        errors = 0
        for _, path, _ in actions:
            try:
                if path.is_dir():
                    shutil.rmtree(path)
                else:
                    path.unlink(missing_ok=True)
            except OSError:
                errors += 1
        if errors:
            print(f"  WARN: {errors} items could not be removed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
