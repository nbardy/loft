"""Check the frozen Phase 5 partition against its pinned example revision and seed sets."""
from collections import Counter
import hashlib
import json
from pathlib import Path
import subprocess


def check():
    root = Path(__file__).resolve().parents[1]
    manifest = json.loads((root / "corpus/manifest.json").read_bytes())
    sources = json.loads((root / manifest["seeds_manifest"]).read_bytes())
    ids = []
    for i, batch in enumerate(manifest["batches"]):
        expected = (i + 1, ("l5c", "l5d", "l5e", "l5f")[i // 5], 10 if i == 19 else 15)
        if (batch["batch"], batch["lane"], len(batch["scenes"])) != expected:
            raise ValueError(f"incorrect corpus partition in batch {i + 1}")
        ids.extend(batch["scenes"])
    if len(ids) != 295 or len(set(ids)) != len(ids):
        raise ValueError("expected 295 distinct corpus entries")
    tree = subprocess.check_output(
        ["git", "ls-tree", "-r", "--name-only", manifest["examples_rev"], "examples"],
        cwd=root, text=True,
    )
    examples = sorted(p for p in tree.splitlines() if p.endswith(".ts"))
    if len(examples) != 125 or ids[:125] != examples:
        raise ValueError("corpus examples differ from the pinned Git tree")
    counts = Counter(identity.split("/", 1)[0] for identity in ids[125:])
    if counts != {s["set"]: s["rows"] for s in sources["seeds"]["sets"]}:
        raise ValueError("corpus seed counts differ from the source manifest")
    fixtures = root / "tests/characters"
    for name, revision, source, blob in json.loads((fixtures / "sources.json").read_bytes()):
        data = (fixtures / "before" / name).read_bytes()
        digest = hashlib.sha1(f"blob {len(data)}\0".encode() + data).hexdigest()
        if digest != blob or not (fixtures / name).is_file():
            raise ValueError(f"{name} differs from its character fixture source {revision}:{source}")
    print("4 character authoring fixture sources verified")
    print("295 distinct Phase 5 identities in 20 batches")


if __name__ == "__main__":
    check()
