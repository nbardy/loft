"""Score lossless pictures and exact structural planes or a seeded path sequence."""

import argparse
import hashlib
import json
import os
import re
import shutil
import subprocess
import sys
from dataclasses import dataclass
from pathlib import Path


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


def rgba_hash(image):
    return hashlib.sha256(image.tobytes()).hexdigest()


def score_pack(modes, cli=None):
    requests = {}
    for name, mode in modes.items():
        match mode:
            case Deterministic():
                requests[name] = {"deterministic": vars(mode)}
            case Stochastic():
                fields = ("size", "min_reference_psnr_db", "min_psnr_db", "max_temporal_variance")
                requests[name] = {"stochastic": {
                    **{key: mode.config[key] for key in fields},
                    "reference": mode.reference, "convergence": mode.convergence, "frames": mode.frames,
                }}
    if cli is None:
        cli = os.environ.get("LOFT_PICTURE_SCORER", ROOT / "target/lab/loft")
    request = json.dumps(requests, default=os.fspath)
    result = subprocess.run([cli, "gate", "--score-pictures"], input=request,
                            text=True, capture_output=True)
    if result.returncode != 0:
        raise ValueError(result.stderr.strip() or result.stdout.strip())
    return json.loads(result.stdout)


def score(mode, cli=None):
    return score_pack({"picture": mode}, cli)["picture"]


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
    return score_pack({name: candidate, f"{name}-mutant": mutant}, cli)


def deterministic_pack(config, baseline, candidate, cli=None):
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
            results[stem] = Deterministic(
                tuple(manifest["size"]), manifest["refs"][f"{name}@{stamp}"],
                ROOT / "tests/fixtures/gate-refs" / f"{stem}.png",
                baseline, candidate, stem,
            )
    return score_pack(results, cli)


def write_capture_inputs(root, out):
    manifest = json.loads((root / "engines/gates.json").read_text())
    inputs = {
        "size": manifest["size"],
        "scenes": {scene["name"]: [scene["file"], scene["set"], scene.get("picture_frames", manifest["frames"])]
                   for scene in manifest["scenes"]},
    }
    (out / "picture-inputs.json").write_text(json.dumps(inputs, sort_keys=True) + "\n")


def capture(build_log, root, out):
    legacy = not (root / "crates/loft-native").is_dir()
    crates = {"loft_passes", "naga", "png", "serde_json"}
    if legacy:
        crates.update({"loft_ir", "loft_runner", "pollster", "serde", "wgpu"})
    else:
        crates.add("loft_native")
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
    scratch.mkdir()
    here = Path(__file__).parent
    for filename in ("capture.rs", "ordered_cull.rs"):
        shutil.copy2(here / filename, scratch)
    if legacy:
        native = scratch / "native"
        shutil.copytree(root / "crates/loft-cli/src/native", native)
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
        shutil.copy2(here / "capture_planes.rs", scratch)
    executable = scratch / "capture"
    externs = [word for crate, path in sorted(deps.items()) for word in ("--extern", f"{crate}={path}")]
    search = [word for path in sorted({p.parent for p in deps.values()})
              for word in ("-L", f"dependency={path}")]
    rustc = subprocess.check_output(["rustup", "which", "rustc"], cwd=root, text=True).strip()
    cfg = ["--cfg", "legacy_native_capture"] if legacy else []
    release = all(path.parent.parent.name == "release" for path in deps.values())
    # Release rlibs can contain LLVM bitcode the system linker cannot read.
    # Use rustc's own linker optimization, just as Cargo does for release.
    codegen = ["-C", "opt-level=3", "-C", "lto=thin", "-C", "codegen-units=1"] if release else ["-C", "opt-level=2"]
    subprocess.run([ROOT / "tools/lib/build_gate.sh", "run", rustc, "--edition=2024", *cfg,
                    *codegen, *search, *externs,
                    scratch / "capture.rs", "-o", executable], cwd=root, check=True)
    subprocess.run([ROOT / "tools/gpu_lock.sh", executable, root, out], check=True)
    write_capture_inputs(root, out)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(required=True)
    comparison = commands.add_parser("deterministic")
    comparison.add_argument("baseline", type=Path)
    comparison.add_argument("candidate", type=Path)
    comparison.add_argument("--scorer", type=Path)
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
            results = deterministic_pack(config["deterministic"], args.baseline, args.candidate, args.scorer)
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
