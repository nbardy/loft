"""Print the Rust source ratchet from scc's by-file Code counts."""
import csv
import io
import subprocess
import sys
import tomllib
from pathlib import Path

members = tomllib.loads(Path("Cargo.toml").read_text())["workspace"]["members"]
counts = {Path(member).name: 0 for member in members if Path(member).parts[0] == "crates"}
version = subprocess.check_output(["scc", "--version"], text=True).strip()
rows = subprocess.check_output(
    ["scc", "--include-ext", "rs", "--exclude-dir", "tests", "--by-file", "--format", "csv", "crates"],
    text=True,
)
shared = native = 0
for row in csv.DictReader(io.StringIO(rows)):
    if row["Language"] != "Rust":
        continue
    path = Path(row["Provider"])
    code = int(row["Code"])
    if path.parts[1] == "shared":
        shared += code
    else:
        counts[path.parts[1]] += code
    if path.is_relative_to("crates/loft-cli/src/native") or path == Path("crates/loft-cli/src/native.rs"):
        native += code
total = sum(counts.values()) + shared
tree = subprocess.check_output(
    ["cargo", "tree", "--locked", "-p", "loft-std", "--all-features", "--target", "all",
     "--prefix", "none", "--format", "{p}"], text=True,
)
wgpu = len({package for package in tree.splitlines() if package.startswith("wgpu ")})
print(version)
for crate, count in sorted(counts.items()):
    print(f"{crate:20} {count:6,d}")
print(f"{'shared':20} {shared:6,d}")
ratchets = [
    ("Rust core", total, 18_000),
    ("loft-runner", counts["loft-runner"], 2_200),
    ("loft-cli/native", native, 450),
    ("loft-std wgpu deps", wgpu, 0),
]
for name, value, ceiling in ratchets:
    print(f"{name:20} {value:6,d} / {ceiling:,}" + (" FAIL" if value > ceiling else ""))
sys.exit(int(any(value > ceiling for _, value, ceiling in ratchets)))
