"""Run every test executable emitted by a single Cargo workspace build."""
import json
from pathlib import Path
import subprocess
import sys


def main(build):
    messages = [json.loads(line) for line in build.read_text().splitlines()]
    if not messages or messages[-1] != {"reason": "build-finished", "success": True}:
        sys.exit(f"incomplete Cargo build in {build}")
    tests = {Path(m["executable"]): Path(m["manifest_path"]).parent for m in messages
             if m.get("executable") and m["profile"]["test"]}
    if not tests:
        sys.exit("no native tests in the build")
    failed = []
    for executable, crate in sorted(tests.items()):
        print(f"\nRunning {executable.name} ({crate.name})", flush=True)
        # Match Cargo's test working directory and serialize tests sharing the GPU.
        if subprocess.run([executable, "--test-threads=1"], cwd=crate).returncode:
            failed.append(executable.name)
    if failed:
        sys.exit("failed test executables: " + ", ".join(failed))
    print(f"Passed all {len(tests)} native test executables")


if __name__ == "__main__":
    main(Path(sys.argv[1]))
