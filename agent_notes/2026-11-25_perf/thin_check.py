"""Inspect emitted segment proxies and a fork whose capsule is a constant hit."""
import hashlib
import json
import shutil
import subprocess
from fresh import checkpoint, notes, out, root, scenes

folder = out / "thin"
binary = folder / "loft"
def shaders(scene):
    data = subprocess.check_output([binary, "show", scene, "--json"])
    return {m["name"].split("_")[-1]:m["wgsl"] for m in json.loads(data)["modules"]}

expected = {"ball":set(), "base":{"Whisker"}, "walrus":{"Rod"}, "otter":{"Strand","Bit"}, "heron":set()}
summary = {}
for name, path in scenes.items():
    modules = shaders(out/"source/fixtures"/(path+".loft"))
    got = {k for k,v in modules.items() if "fn loft_thin_position(" in v}
    for k in got:
        (folder / f"{name}-{k}.wgsl").write_text(modules[k])
    summary[name] = sorted(got)
    print(name, sorted(got), flush=True)
    assert got == expected[name], (name, got, expected[name])
fork = folder / "fork"
fork.mkdir(exist_ok=True)
(fork / "Cargo.toml").touch()
shutil.copytree(root / "stdlib", fork / "stdlib", dirs_exist_ok=True)
lib = fork / "stdlib/geometry/analytic.loft"
source = lib.read_text()
original = "return t0 + t"
assert source.count(original) == 1
lib.write_text(source.replace(original, "return 0.1"))
shutil.copytree(out/"source/fixtures/walrus", fork/"walrus", dirs_exist_ok=True)
summary["fork"] = [k for k,v in shaders(fork/"walrus/tq.loft").items() if "fn loft_thin_position(" in v]
assert not summary["fork"]
summary["binary_sha256"] = hashlib.sha256(binary.read_bytes()).hexdigest()
checkpoint("thin", "compiler.json", json.dumps(summary, indent=2).encode(), "Verify thin-row eligibility and changed-solver fallback")
