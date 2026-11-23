#!/usr/bin/env python3
"""Collect and score paired Speed and Load batches under a single quiet GPU reservation."""

import argparse
import hashlib
import json
import math
import os
import subprocess
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
PAIR_COUNT = 6
CONTROL_CAP_MS = 0.150
METRICS = (("speed", "p95_ms"), ("load", "cold_s"), ("load", "warm_s"))


def read_json(path):
    return json.loads(path.read_text())


def number(value):
    if type(value) not in (int, float) or not math.isfinite(value) or value < 0:
        raise ValueError(f"invalid measurement: {value!r}")
    return value


def median_ci(values):
    """Two-sided exact sign interval for the median of independent paired batches."""
    ordered = sorted(values)
    n = len(ordered)
    tail = 0
    for cut in range(1, (n + 1) // 2 + 1):
        tail += math.comb(n, cut - 1)
        if 40 * tail > 2**n:
            break
        bounds = [ordered[cut - 1], ordered[-cut]]
    return bounds


def interval_verdict(bounds, limit):
    lo, hi = bounds
    if hi <= limit:
        return "held"
    if lo > limit:
        return "missed"
    return "unknown"


def speed_contended(speed):
    if "blocks" in speed:
        return bool(speed["contended"])
    # Older binaries took one control before the batch.
    return number(speed.get("control_p95_ms", 0)) >= CONTROL_CAP_MS


def score(pairs, scenes, budgets, margin):
    if len(pairs) < PAIR_COUNT or any(len(pair) != 2 for pair in pairs):
        raise ValueError(f"need at least {PAIR_COUNT} complete same-run pairs")
    adapters = {report["adapter"] for pair in pairs for report in pair}
    if len(adapters) != 1:
        raise ValueError(f"adapter changed within the run: {adapters}")
    results = {}
    preflight = [[number(report["preflight_control_p95_ms"]) for report in pair] for pair in pairs]
    for name in scenes:
        controls = [[number(report[name]["speed"].get("control_p95_ms", report["preflight_control_p95_ms"])) for report in pair]
                    for pair in pairs]
        # The preflight is one cold sample and says nothing about the batch; the gate's bracketed blocks decide.
        contended = any(speed_contended(report[name]["speed"]) for pair in pairs for report in pair)
        metrics = {}
        for gate, metric in METRICS:
            observations = [[number(report[name][gate][metric]) for report in pair] for pair in pairs]
            candidate_ci = median_ci([candidate for _, candidate in observations])
            delta_ci = median_ci([candidate - baseline for baseline, candidate in observations])
            cap = number(budgets[name][metric])
            absolute = interval_verdict(candidate_ci, cap)
            relative = interval_verdict(delta_ci, cap * margin)
            verdict = "held"
            if contended:
                verdict = "unknown"
            elif "missed" in (absolute, relative):
                verdict = "missed"
            elif "unknown" in (absolute, relative):
                verdict = "unknown"
            metrics[metric] = {
                "pairs": observations, "candidate_ci95": candidate_ci, "paired_delta_ci95": delta_ci,
                "budget": cap, "allowed_delta": cap * margin, "verdict": verdict,
            }
        results[name] = {"preflight_control_p95_ms": preflight, "triangle_control_ms": controls,
                         "contended": contended, "metrics": metrics}
    return {"adapter": adapters.pop(), "scenes": results}


def checkout(cli):
    root = subprocess.check_output(["git", "-C", cli.parent, "rev-parse", "--show-toplevel"], text=True)
    return Path(root.strip())


def legacy_checks(log):
    gate = ""
    failures = []
    for line in log.splitlines():
        if line.split(": ", 1)[0] in ("Pictures", "Twin", "Speed", "Load", "Truth"):
            gate = line.split(": ", 1)[0]
        if line.startswith("  FAIL "):
            _, scene, check = line.strip().split(maxsplit=2)
            if (gate == "Speed" and check.startswith("p95 ")) or (
                gate == "Load" and check.startswith(("cold ", "warm "))
            ):
                continue
            failures.append(f"{gate} {scene}: {check}")
    return failures


def collect(args, work):
    if not os.environ.get("LOFT_QUIET_GPU") or os.environ["LOFT_QUIET_GPU"] != os.environ.get("LOFT_GPU_HOLDER"):
        raise ValueError("paired collection requires quiet_gpu.sh")
    binaries = (args.baseline, args.candidate)
    def binary_hash(cli):
        with cli.open("rb") as binary:
            return hashlib.file_digest(binary, "sha256").hexdigest()

    hashes = tuple(binary_hash(cli) for cli in binaries)
    roots = tuple(checkout(cli) for cli in binaries)
    manifests = [read_json(root / "engines/gates.json") for root in roots]
    # Older checkouts provide the gate in loft itself.
    gate_binaries = tuple(
        cli.with_name("loft-verify") if (root / "tools/loft-verify/Cargo.toml").is_file() else cli
        for cli, root in zip(binaries, roots)
    )
    gate_hashes = tuple(binary_hash(binary) for binary in gate_binaries)
    baseline, candidate = manifests
    selected = args.only.split(",") if args.only else [
        scene["name"] for scene in candidate["scenes"] if "budget" in scene]
    if not selected or len(set(selected)) != len(selected):
        raise ValueError("select distinct budgeted scenes")
    for key in ("size", "frames", "timing_frames"):
        if baseline[key] != candidate[key]:
            raise ValueError(f"paired runs require matching {key}")
    by_name = [{scene["name"]: scene for scene in manifest["scenes"] if "budget" in scene}
               for manifest in manifests]
    for name in selected:
        for key in ("file", "set"):
            if by_name[0][name][key] != by_name[1][name][key]:
                raise ValueError(f"{name}: paired runs require matching {key}")
    budgets = {name: by_name[1][name]["budget"] for name in selected}

    arms = [work / name for name in ("baseline", "candidate")]
    for arm, root in zip(arms, roots):
        arm.mkdir()
        for path in ("stdlib", "engines", "tests", "Cargo.toml"):
            (arm / path).symlink_to(root / path)
        # Other sources may live outside engines. Preserve the checkout's scene path.
        for name in selected:
            source = Path(by_name[1][name]["file"])
            top = source.parts[0]
            if not (arm / top).exists():
                (arm / top).symlink_to(root / top)
    first = by_name[1][selected[0]]
    size = "x".join(map(str, candidate["size"]))
    overrides = [word for key, value in first["set"].items() for word in ("--set", f"{key}={value}")]
    selection = ["--only", ",".join(selected)] if args.only else []
    failures = set()
    for index in range(PAIR_COUNT):
        # Alternate AB and BA so a systematic first-run effect does not favor one arm.
        for arm_index in (range(2) if index % 2 == 0 else (1, 0)):
            arm = arms[arm_index]
            cli = gate_binaries[arm_index]
            control_file = arm / f"control-{index}.json"
            report_file = arm / "out/meta/gate.json"
            report_file.unlink(missing_ok=True)
            log_path = arm / f"pair-{index}.log"
            with log_path.open("w") as log:
                # The candidate can record a p95 control for an older baseline whose
                # profile command only emitted a median. Use it in both arms.
                # It compiles the candidate's own sources in both arms: the control measures machine
                # load, and a candidate cannot compile a baseline stdlib that predates a stock pass
                # moving into stdlib (E520 unknown pass `cull_indirect`, 23 Nov integration).
                subprocess.run([binaries[1], "profile", arms[1] / first["file"], "--size", size,
                                "--frames", "1", "--warmup", "0", "--json", control_file,
                                *overrides], cwd=arms[1], stdout=log, stderr=subprocess.STDOUT, check=True)
                preflight = number(read_json(control_file)["control_p95_ms"])
                # A loaded preflight is recorded, not fatal: the gate's bracketed blocks classify contention.
                raw = subprocess.run([cli, "gate", *selection],
                                     cwd=arm, stdout=log, stderr=subprocess.STDOUT)
            if raw.returncode not in (0, 2, 3, 4, 7):
                raise ValueError(f"{cli}: gate exit {raw.returncode}, see {log_path}")
            if arm_index == 1:
                failures.update(legacy_checks(log_path.read_text()))
            report = read_json(report_file)
            control = read_json(control_file)
            if report["adapter"] != control["adapter"]:
                raise ValueError("triangle and gate used different adapters")
            for name in selected:
                speed = report[name]["speed"]
                complete = (speed["frames"] >= 0.7 * candidate["timing_frames"] if "blocks" in speed
                            else speed["frames"] == candidate["timing_frames"])
                if len(speed["ms"]) != speed["frames"] or not (complete or speed.get("contended")):
                    raise ValueError(f"{name}: incomplete frame batch")
            report.update(preflight_control_p95_ms=control["control_p95_ms"], raw_gate_exit=raw.returncode)
            (arm / f"pair-{index}.json").write_text(json.dumps(report, allow_nan=False) + "\n")
            report_file.unlink()
    if (hashes != tuple(binary_hash(cli) for cli in binaries)
            or gate_hashes != tuple(binary_hash(binary) for binary in gate_binaries)):
        raise ValueError("a measured binary changed during collection")
    inputs = {"scenes": selected, "budgets": budgets, "margin": args.margin}
    inputs["binaries"] = [{"path": str(cli), "sha256": sha} for cli, sha in zip(binaries, hashes)]
    inputs["gate_binaries"] = [{"path": str(binary), "sha256": sha}
                               for binary, sha in zip(gate_binaries, gate_hashes)]
    inputs["other_gate_failures"] = sorted(failures)
    inputs["size"] = candidate["size"]
    inputs["timing_frames"] = candidate["timing_frames"]
    (work / "timing-inputs.json").write_text(json.dumps(inputs, indent=2, allow_nan=False) + "\n")


def replay(work):
    if (work / "guard-exit.txt").read_text().strip() != "0":
        raise ValueError("the recorded timing window failed the quiet GPU guard")
    inputs = read_json(work / "timing-inputs.json")
    pairs = [[read_json(work / arm / f"pair-{index}.json") for arm in ("baseline", "candidate")]
             for index in range(PAIR_COUNT)]
    result = score(pairs, inputs.pop("scenes"), inputs.pop("budgets"), inputs["margin"])
    result.update(inputs)
    return result


def verdict(result):
    return int(bool(result["other_gate_failures"]) or
               any(row["verdict"] != "held" for data in result["scenes"].values()
                   for row in data["metrics"].values()))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    if sys.argv[1:2] == ["replay"]:
        parser.add_argument("replay")
        parser.add_argument("out", type=Path)
        result = replay(parser.parse_args().out)
        print(json.dumps(result, indent=2, allow_nan=False))
        return verdict(result)
    parser.add_argument("baseline", type=lambda p: Path(p).resolve(strict=True))
    parser.add_argument("candidate", type=lambda p: Path(p).resolve(strict=True))
    parser.add_argument("out", type=lambda p: Path(p).resolve(), help="new evidence directory")
    parser.add_argument("--only", help="comma-separated scene names")
    parser.add_argument("--margin", type=float, required=True, help="allowed delta as a fraction of the candidate budget")
    parser.add_argument("--collect", action="store_true", help=argparse.SUPPRESS)
    args = parser.parse_args()
    if not math.isfinite(args.margin) or not 0 <= args.margin < 1:
        parser.error("--margin must be finite and in [0, 1)")
    if args.collect:
        collect(args, args.out)
        return 0
    args.out.mkdir(parents=True)
    with tempfile.TemporaryDirectory(dir=args.out) as tmp:
        work = Path(tmp)
        cmd = [ROOT / "tools/quiet_gpu.sh", sys.executable, Path(__file__).resolve(),
               args.baseline, args.candidate, work, "--margin", str(args.margin), "--collect"]
        if args.only:
            cmd += ["--only", args.only]
        status = subprocess.run(cmd).returncode
        # Preserve the raw attempts even when the guard invalidated the run.
        (args.out / "guard-exit.txt").write_text(f"{status}\n")
        for evidence in work.iterdir():
            evidence.rename(args.out / evidence.name)
    if status != 0:
        return status
    result = replay(args.out)
    (args.out / "timing.json").write_text(json.dumps(result, indent=2, allow_nan=False) + "\n")
    for scene, data in result["scenes"].items():
        print(scene, {metric: row["verdict"] for metric, row in data["metrics"].items()})
    for failure in result["other_gate_failures"]:
        print(failure)
    return verdict(result)


if __name__ == "__main__":
    raise SystemExit(main())
