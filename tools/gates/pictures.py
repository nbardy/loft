# /// script
# dependencies = ["numpy", "pillow"]
# ///
"""Score lossless pictures and exact structural planes or a seeded path sequence."""

import argparse
import hashlib
import json
import math
import re
import shutil
import subprocess
import sys
from dataclasses import dataclass
from pathlib import Path

import numpy as np
from PIL import Image

ROOT = Path(__file__).resolve().parents[2]
MANIFEST = Path(__file__).with_name("pictures.json")


@dataclass(frozen=True)
class Deterministic:
    size: tuple
    approved_hash: str
    approved: Path
    baseline: Path
    candidate: Path
    stem: str


@dataclass(frozen=True)
class Stochastic:
    config: dict
    reference: Path
    convergence: Path
    frames: tuple


def rgba(path, size):
    with Image.open(path) as image:
        if image.format != "PNG" or image.mode != "RGBA" or image.size != size:
            raise ValueError(f"{path}: expected lossless RGBA8 PNG at {size}")
        return np.array(image)


def rgba_hash(image):
    return hashlib.sha256(image.tobytes()).hexdigest()


def deterministic(mode):
    approved = rgba(mode.approved, mode.size)
    if rgba_hash(approved) != mode.approved_hash:
        raise ValueError(f"{mode.approved}: approved RGBA hash mismatch")
    baseline = rgba(mode.baseline / f"{mode.stem}.png", mode.size)
    candidate = rgba(mode.candidate / f"{mode.stem}.png", mode.size)
    # Main defines the drift budget. Report the frozen comparison for review.
    comparisons = {}
    for name, reference in (("main", baseline), ("approved", approved)):
        delta = np.abs(candidate.astype(np.int16) - reference).max(axis=2)
        changed = int(np.count_nonzero(delta))
        worst = int(delta.max())
        comparisons[name] = {
            "changed_pixels": changed, "max_code_delta": worst,
            "held": worst <= 1 and changed * 1000 < delta.size,
        }
    structural = {}
    for name in ("id.u32", "depth.f32"):
        planes = [(arm / f"{mode.stem}.{name}").read_bytes()
                  for arm in (mode.baseline, mode.candidate)]
        if any(len(plane) != math.prod(mode.size) * 4 for plane in planes):
            raise ValueError(f"{mode.stem}.{name}: expected one 32-bit word per pixel")
        structural[name] = planes[0] == planes[1]
    return {
        "pixels": math.prod(mode.size), "rgba_sha256": rgba_hash(candidate),
        "comparisons": comparisons, "structure": structural,
        "held": comparisons["main"]["held"] and all(structural.values()),
    }


def psnr(a, b):
    mse = np.mean(np.square(a - b))
    # JSON has no infinity. Zero MSE is a separate, exact observation.
    return None if mse == 0 else float(-10 * np.log10(mse))


def stochastic(mode):
    cfg = mode.config
    size = tuple(cfg["size"])
    reference = rgba(mode.reference, size)[..., :3] / 255
    convergence = rgba(mode.convergence, size)[..., :3] / 255
    frames = np.stack([rgba(path, size)[..., :3] / 255 for path in mode.frames])
    convergence_db = psnr(reference, convergence)
    frame_db = [psnr(frame, reference) for frame in frames]
    # Variance is measured per pixel and channel before spatial averaging.
    variance = float(np.var(frames, axis=0, ddof=1).mean())
    converged = convergence_db is None or convergence_db >= cfg["min_reference_psnr_db"]
    return {
        "reference_psnr_db": convergence_db, "frame_psnr_db": frame_db,
        "temporal_variance": variance,
        "held": converged and variance <= cfg["max_temporal_variance"]
        and all(db is None or db >= cfg["min_psnr_db"] for db in frame_db),
    }


def score(mode):
    match mode:
        case Deterministic():
            return deterministic(mode)
        case Stochastic():
            return stochastic(mode)
    raise TypeError(mode)


def sequence(cfg, folder, frame_prefix="seed"):
    seeds = cfg["frame_seeds"]
    ref_seed = cfg["reference_seed"]
    if len(seeds) < 2 or len(set([*seeds, ref_seed, ref_seed + 1])) != len(seeds) + 2:
        raise ValueError("frame and reference seeds must be distinct")
    return Stochastic(cfg, folder / "reference.png", folder / "convergence.png",
                      tuple(folder / f"{frame_prefix}-{seed}.png" for seed in seeds))


def capture_path(reference_cli, cli, name, cfg, out):
    candidate = sequence(cfg, out)
    mutant = sequence(cfg, out, "mutant")
    seeds = cfg["frame_seeds"]
    ref_seed = cfg["reference_seed"]
    # Reuse the production estimator. Specialize its secondary-path stream in
    # scratch because the public render CLI exposes sample count but no RNG seed.
    fixture = (ROOT / cfg["scene"]).read_text()
    sources = {}
    for arm, binary in (("reference", reference_cli), ("candidate", cli)):
        root = next((parent for parent in binary.parents
                     if (parent / "stdlib/engines/reference.loft").is_file()), None)
        if root is None:
            raise ValueError(f"{binary}: expected a binary inside its source checkout")
        overlay = out / arm
        overlay.mkdir()
        for path in ("stdlib", "Cargo.toml"):
            (overlay / path).symlink_to(root / path)
        engine = (root / "stdlib/engines/reference.loft").read_text().split("pub schedule Reference {", 1)[0]
        anchor = "    var ray_o = origin;"
        if engine.count(anchor) != 1 or engine.count("return radiance;") != 1:
            raise ValueError(f"{root}: reference estimator instrumentation point changed")
        sources[arm] = overlay, engine, anchor

    def render(arm, binary, image, seed, samples, gain):
        overlay, engine, anchor = sources[arm]
        start = engine.index(anchor)
        body = re.sub(r"\bseed\b", "gate_seed", engine[start:])
        code = engine[:start] + f"    let gate_seed = seed ^ {seed}u;\n" + body
        code = code.replace("return radiance;", f"return {gain} * radiance;")
        source = overlay / f"{image.stem}.loft"
        source.write_text(code + "\n" + fixture)
        subprocess.run([
            binary, "render", source, "--out", image,
            "--size", "x".join(map(str, cfg["size"])), "--time", str(cfg["time"]),
            "--samples", str(samples),
        ], cwd=overlay, stdout=sys.stderr, check=True)

    render("reference", reference_cli, candidate.reference, ref_seed, cfg["reference_samples"], 1.0)
    render("reference", reference_cli, candidate.convergence, ref_seed + 1, 2 * cfg["reference_samples"], 1.0)
    for seed, good, broken in zip(seeds, candidate.frames, mutant.frames, strict=True):
        render("candidate", cli, good, seed, cfg["samples"], 1.0)
        render("candidate", cli, broken, seed, cfg["samples"], 0.1)
    return {
        name: score(candidate),
        f"{name}-mutant": score(mutant),
    }


def deterministic_pack(config, baseline, candidate):
    manifest = json.loads((ROOT / "engines/gates.json").read_text())
    if sorted(config) != sorted(scene["name"] for scene in manifest["scenes"]):
        raise ValueError("every Pictures gate scene needs a mode declaration")
    if (baseline / "picture-inputs.json").read_bytes() != (candidate / "picture-inputs.json").read_bytes():
        raise ValueError("deterministic captures require the same scene settings and sample times")
    if (baseline / "adapter.txt").read_text() != (candidate / "adapter.txt").read_text():
        raise ValueError("deterministic captures require the same adapter")
    results = {}
    for scene in manifest["scenes"]:
        for time in scene.get("picture_frames", manifest["frames"]):
            stamp = format(time, "g")
            name = scene["name"]
            stem = f"{name}_t{stamp}"
            results[stem] = score(Deterministic(
                tuple(manifest["size"]), manifest["refs"][f"{name}@{stamp}"],
                ROOT / "tests/fixtures/gate-refs" / f"{stem}.png",
                baseline, candidate, stem,
            ))
    return results


def write_capture_inputs(root, out):
    manifest = json.loads((root / "engines/gates.json").read_text())
    inputs = {
        "size": manifest["size"],
        "scenes": {scene["name"]: [scene["file"], scene["set"], scene.get("picture_frames", manifest["frames"])]
                   for scene in manifest["scenes"]},
    }
    (out / "picture-inputs.json").write_text(json.dumps(inputs, sort_keys=True) + "\n")


def capture(build_log, root, out):
    crates = {"loft_ir", "loft_passes", "loft_runner", "naga", "png", "pollster",
              "serde", "serde_json", "wgpu"}
    libraries = {crate: set() for crate in crates}
    with build_log.open() as messages:
        for line in messages:
            message = json.loads(line)
            if message["reason"] != "compiler-artifact":
                continue
            crate = message["target"]["name"]
            if crate in crates:
                libraries[crate].update(Path(p) for p in message["filenames"] if p.endswith(".rlib"))
    if any(len(artifacts) != 1 for artifacts in libraries.values()):
        raise ValueError("use a lab build log with exactly one rlib per capture dependency")
    deps = {crate: paths.pop().resolve(strict=True) for crate, paths in libraries.items()}
    expected = root / "target"
    if not all(path.is_relative_to(expected) for path in deps.values()):
        raise ValueError(f"capture libraries must come from {expected}")
    out.mkdir(parents=True)
    scratch = out / "instrument"
    native = scratch / "native"
    shutil.copytree(root / "crates/loft-cli/src/native", native)
    here = Path(__file__).parent
    for source in ("capture.rs", "capture_planes.rs"):
        shutil.copy2(here / source, scratch)
    module = native / "mod.rs"
    code = module.read_text()
    for before, after in (
        ('size, VIS, T::RENDER_ATTACHMENT | T::TEXTURE_BINDING);',
         'size, VIS, T::RENDER_ATTACHMENT | T::TEXTURE_BINDING | T::COPY_SRC);'),
        ('"depth", size, DEPTH, T::RENDER_ATTACHMENT);',
         '"depth", size, DEPTH, T::RENDER_ATTACHMENT | T::COPY_SRC);'),
        ('wgpu::StoreOp::Store, wgpu::StoreOp::Discard),',
         'wgpu::StoreOp::Store, wgpu::StoreOp::Store),'),
    ):
        if code.count(before) != 1:
            raise ValueError(f"{root}: native picture attachment declaration changed: {before}")
        code = code.replace(before, after)
    module.write_text(code + '\ninclude!("../capture_planes.rs");\n')
    executable = scratch / "capture"
    externs = [word for crate, path in sorted(deps.items()) for word in ("--extern", f"{crate}={path}")]
    search = [word for path in sorted({p.parent for p in deps.values()})
              for word in ("-L", f"dependency={path}")]
    rustc = subprocess.check_output(["rustup", "which", "rustc"], cwd=root, text=True).strip()
    subprocess.run([ROOT / "tools/lib/build_gate.sh", "run", rustc, "--edition=2024",
                    "-C", "opt-level=2", *search, *externs,
                    scratch / "capture.rs", "-o", executable], cwd=root, check=True)
    subprocess.run([ROOT / "tools/gpu_lock.sh", executable, root, out], check=True)
    write_capture_inputs(root, out)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(required=True)
    comparison = commands.add_parser("deterministic")
    comparison.add_argument("baseline", type=Path)
    comparison.add_argument("candidate", type=Path)
    comparison.set_defaults(mode="deterministic")
    raw = commands.add_parser("capture")
    raw.add_argument("build_log", type=Path)
    raw.add_argument("checkout", type=Path)
    raw.add_argument("out", type=Path)
    raw.set_defaults(mode="capture")
    path = commands.add_parser("stochastic")
    path.add_argument("reference_cli", type=Path)
    path.add_argument("cli", type=Path)
    path.add_argument("out", type=Path, help="a new scratch directory under the checkout's out/")
    path.set_defaults(mode="stochastic")
    replay = commands.add_parser("sequence")
    replay.add_argument("scene")
    replay.add_argument("captures", type=Path)
    replay.set_defaults(mode="sequence")
    args = parser.parse_args()
    config = json.loads(MANIFEST.read_text())
    mutants = set()
    match args.mode:
        case "capture":
            capture(args.build_log, args.checkout.resolve(strict=True), args.out.resolve())
            return 0
        case "deterministic":
            results = deterministic_pack(config["deterministic"], args.baseline, args.candidate)
        case "sequence":
            if args.scene not in config["stochastic"]:
                parser.error(f"{args.scene}: no stochastic mode declaration")
            results = {args.scene: score(sequence(config["stochastic"][args.scene], args.captures))}
        case "stochastic":
            out = args.out.resolve()
            if not out.is_relative_to(ROOT / "out"):
                parser.error("stochastic output must be under the checkout's out/")
            out.mkdir(parents=True)
            results = {}
            for name, cfg in config["stochastic"].items():
                work = out / name
                work.mkdir()
                results.update(capture_path(args.reference_cli.resolve(strict=True),
                                            args.cli.resolve(strict=True), name, cfg, work))
                mutants.add(f"{name}-mutant")
    print(json.dumps(results, indent=2, allow_nan=False))
    return not all(result["held"] == (name not in mutants) for name, result in results.items())


if __name__ == "__main__":
    raise SystemExit(main())
