"""Run from the lane root with tools/gpu_lock.sh tools/quiet_gpu.sh."""
import hashlib
import json
import os
from pathlib import Path
import random
import subprocess
import sys

loft, out = Path(sys.argv[1]).resolve(), Path(sys.argv[2]).resolve()
out.mkdir(parents=True, exist_ok=True)
scenes = Path("scenes/showcase/cap1c")
generated = Path("out/cap1c-src")
generated.mkdir(parents=True, exist_ok=True)
source = scenes / "variety.loft"
hand = generated / "hand.loft"
hand_text = source.read_text().replace(
    "use stdlib::materials::families::{standard_pbr, layered_coat, stylized_ink, dielectric_glass, emissive, foliage}",
    """use stdlib::materials::families::{dielectric_glass, emissive}
use stdlib::materials::bsdf::{ggx, lambert, coat, sss, sheen}

fn grain_control(base: rgb, p: v3, scale: f, seed: f) -> rgb =
  wgsl(base: rgb, p: v3, scale: f, seed: f) -> rgb {
    return base * (0.86 + 0.14 * sin(scale * p.x + seed + 0.35 * sin(scale * p.y - seed)));
  }
fn ink_control(base: rgb, p: v3, scale: f, seed: f) -> rgb =
  wgsl(base: rgb, p: v3, scale: f, seed: f) -> rgb {
    return base * (0.55 + 0.45 * smoothstep(-0.05, 0.05, sin(scale * (p.x + 0.3 * p.y) + seed)));
  }""")
for old, new in [
    ("standard_pbr(m.base, m.rough, m.metal, coords(p), m.grain, m.seed)",
     "ggx(grain_control(m.base, coords(p), m.grain, m.seed), m.rough, m.metal)"),
    ("stylized_ink(m.base, coords(p), m.grain, m.seed)",
     "lambert(ink_control(m.base, coords(p), m.grain, m.seed))"),
    ("layered_coat(m.base, m.rough, m.metal, 0.6, 0.2, coords(p), m.grain, m.seed)",
     "ggx(grain_control(m.base, coords(p), m.grain, m.seed), m.rough, m.metal) + coat(0.6, 0.2)"),
    ("foliage(m.base, m.rough, 0.16, coords(p), m.grain, m.seed)",
     "sss(grain_control(m.base, coords(p), m.grain, m.seed), 0.16) + sheen(grain_control(m.base, coords(p), m.grain, m.seed) * 0.2, m.rough)"),
]:
    hand_text = hand_text.replace(old, new)
hand.write_text(hand_text)
(out / "hand.loft").write_text(hand_text)

def run(verb, scene, *extra, settings=(), stdout=None, check=True):
    cmd = [str(loft), verb, str(scene), *extra]
    for key, value in settings:
        cmd += ["--set", f"{key}={value}"]
    with (out / "commands.jsonl").open("a") as log:
        log.write(json.dumps(cmd) + "\n")
    return subprocess.run(cmd, check=check, stdout=stdout, stderr=subprocess.STDOUT)

def plan(label, scene, settings=()):
    path = out / f"{label}.plan.json"
    with path.open("w") as stream:
        run("show", scene, "--plan", "--json", settings=settings, stdout=stream)
    doc = json.loads(path.read_text())
    for module in doc["modules"]:
        (out / f"{label}.{module['name']}.wgsl").write_text(module["wgsl"])
    return doc

def specialize(n, varied_geometry=False):
    kinds = []
    for i in range(n):
        radius = 0.062 + i * 0.0003 if varied_geometry else 0.068
        kinds.append(f"""kind Ball{i} @Local (pose: xf@Local>World) {{
  visible(ro, rd) = ray_sphere(ro, rd, [0, 0, 0]@Local, {radius:.6f}m)
  trace(p) = sphere(p - [0, 0, 0]@Local, {radius:.6f}m)
  material(p, n) = standard_pbr(rgb({i / n + 0.1}), 0.4, 0.1, coords(p), 40.0, {i}.0)
}}""")
    s = f"""use stdlib::geometry::analytic::{{ray_sphere}}
use stdlib::sdf::solid::{{sphere}}
use stdlib::materials::families::{{standard_pbr}}
camera Main {{ eye: [0, 0, 8.9]@World, at: [0, 0, 0]@World, fov: 26deg }}
{chr(10).join(kinds)}
build {{
  build for i in 0..1000 {{
    let p = [(f32(i % 40) - 19.5) * 0.15, (f32(i / 40) - 12.0) * 0.15, 0]@World
"""
    for i in range(n):
        s += f"    if i % {n} == {i} {{ emit Ball{i}(pose: place(p)) }}\n"
    s += "  }\n}\n"
    suffix = "-geometry" if varied_geometry else ""
    path = generated / f"specialized-{n}{suffix}.loft"
    path.write_text(s)
    return path

def profile(label, scene, settings, nonce, frames=100):
    path = out / f"{label}.timing.json"
    run("profile", scene, "--time", "0", "--size", "1280x800", "--frames", str(frames),
        "--loads", "2", "--nonce", str(nonce), "--json", str(path), settings=settings)
    doc = json.loads(path.read_text())
    doc["contended"] = doc["control_p95_ms"] >= 0.15
    path.write_text(json.dumps(doc, indent=2) + "\n")
    return doc

def q95(data):
    return sorted(data)[(len(data) * 95 + 99) // 100 - 1]

def times(doc, pass_name):
    if pass_name == "Frame":
        return doc["gpu_frame_ms"]
    idx, = (i for i, name in enumerate(doc["passes"]) if name.startswith(pass_name))
    return [stamp[idx] for stamp in doc["gpu_pass_ms"]]

def intervals(runs, route):
    valid = [(times(a, route), times(b, route)) for a, b in runs if not (a["contended"] or b["contended"])]
    if not valid:
        return {"paired_runs": 0}
    rng = random.Random(77692)
    difference, growth = [], []
    for _ in range(1200):
        a, b = [], []
        for x, y in rng.choices(valid, k=len(valid)):
            for j in rng.choices(range(0, len(x), 10), k=len(x) // 10):
                a.extend(x[j:j+10])
                b.extend(y[j:j+10])
        qa, qb = q95(a), q95(b)
        difference.append(qb - qa)
        if qa > 0:
            growth.append(qb / qa - 1)
    interval = lambda samples: [sorted(samples)[29], sorted(samples)[1169]] if len(samples) == 1200 else None
    a = q95([t for x, _ in valid for t in x])
    b = q95([t for _, y in valid for t in y])
    return {"paired_runs": len(valid), "a_p95_ms": a, "b_p95_ms": b, "delta_ms": b - a,
            "delta_95_interval_ms": interval(difference), "growth_95_interval": interval(growth)}

cases = {
    "family10": (source, (("materials", 10),)),
    "family100": (source, (("materials", 100),)),
    "family1000": (source, ()),
    "shuffled1000": (source, (("shuffle", 1),)),
    "pbr-ink200": (source, (("materials", 200), ("families", 2))),
    "all-six": (source, (("families", 6),)),
    "hand1000": (hand, ()),
    "atlas": (scenes / "atlas.loft", ()),
    "atlas-shuffled": (scenes / "atlas.loft", (("shuffle", 1),)),
    "bounces": (scenes / "bounces.loft", ()),
    "specialized10": (specialize(10), ()),
}
for count in [100, 1000, 33]:
    specialized = specialize(count, varied_geometry=count == 33)
    with (out / f"specialized-{count}.log").open("w") as log:
        result = run("check", specialized, stdout=log, check=False)
    assert result.returncode == 1 and "E520" in (out / f"specialized-{count}.log").read_text()

plans = {label: plan(label, scene, settings) for label, (scene, settings) in cases.items()}
for label in ["family10", "family100", "family1000"]:
    assert len(plans[label]["shade"]) == 4
assert len(plans["pbr-ink200"]["shade"]) <= 3
for label in ["family1000", "pbr-ink200", "atlas", "atlas-shuffled", "bounces"]:
    scene, settings = cases[label]
    run("build", scene, "-o", str(out / f"{label}.loftb"), settings=settings)

pairs = {}
# Alternate the order, and use a fresh shader key for each cold load.
seed = int.from_bytes(os.urandom(3), "little") + 1000
for i, (a, b) in enumerate([("family10", "family1000"), ("hand1000", "family1000"),
                            ("family1000", "shuffled1000")]):
    runs = []
    for j in range(4):
        result = {}
        for k, label in enumerate([a, b] if j % 2 == 0 else [b, a]):
            scene, settings = cases[label]
            result[label] = profile(f"pair{i}-{j}-{label}", scene, settings, seed + i*16 + j*2 + k)
        runs.append((result[a], result[b]))
    pairs[f"{a}/{b}"] = {route: intervals(runs, route) for route in ["Classify", "Shade", "Frame"]}

manifest = json.loads(Path("engines/gates.json").read_text())
capsules = next(scene["budget"] for scene in manifest["scenes"] if scene["name"] == "capsules-1")
sample = json.loads((out / "pair0-0-family1000.timing.json").read_text())
observed = {"modules": plans["family1000"]["module_count"], "pipelines": sample["pipelines"]}
budget_controls = []
for name in ["modules", "pipelines"]:
    caps = {**observed, name: capsules[name]}
    failed = [key for key in caps if observed[key] > caps[key]]
    assert failed == [name], (observed, caps)
    budget_controls.append({"observed": observed, "caps": caps, "exceeded": failed})
(out / "budget-controls.json").write_text(json.dumps(budget_controls, indent=2) + "\n")

for i, label in enumerate(["family100", "specialized10", "pbr-ink200", "atlas", "atlas-shuffled"]):
    profile(label, *cases[label], seed + 100 + i)

for label in ["family10", "family100", "family1000", "shuffled1000", "pbr-ink200",
              "all-six", "hand1000", "atlas", "atlas-shuffled", "bounces"]:
    scene, settings = cases[label]
    png = generated / f"{label}.png"
    run("render", scene, "--out", str(png), "--size", "1280x800", settings=settings)
    (out / f"{label}.png.sha256").write_text(hashlib.sha256(png.read_bytes()).hexdigest() + "\n")
    subprocess.run(["ffmpeg", "-v", "error", "-y", "-i", str(png), "-quality", "95",
                    str(out / f"{label}.webp")], check=True)
(out / "comparisons.json").write_text(json.dumps(pairs, indent=2) + "\n")
