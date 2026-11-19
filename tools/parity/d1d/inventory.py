#!/usr/bin/env python3
"""Join the frozen Phase 5 identities with independent R0 and scored evidence."""

import argparse
from collections import Counter, defaultdict, deque
import hashlib
import json
from pathlib import Path, PurePosixPath
import posixpath
import re
import subprocess


def read(path):
    return json.loads(path.read_bytes())


def git(root, *args):
    return subprocess.check_output(["git", "-C", root, *args])


def sha(data):
    return hashlib.sha256(data).hexdigest()


def unique(pairs, label):
    result = {}
    for key, value in pairs:
        if key in result:
            raise ValueError(f"duplicate {label}: {key}")
        result[key] = value
    return result


def require(condition, message):
    if not condition:
        raise ValueError(message)


def seed_sources(creative, config):
    revision = git(creative, "rev-parse", config["commit"]).decode().strip()
    result = []
    for dataset in config["sets"]:
        contents = git(creative, "show", f'{revision}:{dataset["path"]}')
        require(sha(contents) == dataset["sha256"], f'changed seed manifest {dataset["path"]}')
        lines = contents.splitlines()
        require(len(lines) == dataset["rows"], f'wrong seed row count in {dataset["path"]}')
        for line_no, line in enumerate(lines, 1):
            entry = json.loads(line)
            key = f'{dataset["set"]}/{entry["id"]}'
            result.append((key, {
                "revision": revision,
                "path": dataset["path"],
                "line": line_no,
                "row_sha256": sha(line),
                "file_sha256": dataset["sha256"],
            }))
    return unique(result, "seed source")


def archive_id(entry):
    origin = entry["origin"]
    match origin["kind"]:
        case "example":
            return entry["path"]
        case "seed":
            return f'{origin["set"]}/{origin["seed"]}'
        case kind:
            raise ValueError(f"unknown R0 origin {kind}")


def source_imports(sources):
    # The edge is evidence of a source import. Its presence alone cannot prove
    # that the consumer executes the imported behavior at any particular shot.
    import_path = re.compile(rb'''\b(?:from\s+|import\s*\(?\s*)["'](\.[^"'\n]+)["']''')
    consumers = defaultdict(set)
    for source, contents in sources.items():
        parent = str(PurePosixPath(source).parent)
        for match in import_path.finditer(contents):
            path = posixpath.normpath(posixpath.join(parent, match[1].decode()))
            for resolved in (path, path + ".ts", path + "/index.ts"):
                if resolved in sources:
                    consumers[resolved].add(source)
                    break
    return consumers


def rendered_consumers(identity, imports, drawn):
    visited = {identity}
    pending = deque([(identity,)])
    paths = []
    while pending:
        path = pending.popleft()
        for consumer in sorted(imports[path[-1]] - visited):
            visited.add(consumer)
            chain = (*path, consumer)
            if consumer in drawn:
                paths.append(chain)
            pending.append(chain)
    return paths


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--legacy", type=Path, required=True, help="Loft checkout holding R0 and i5-r2")
    parser.add_argument("--creative", type=Path, required=True)
    parser.add_argument("--run", type=Path, help="include current CPU, bead and scratch legacy measurements")
    parser.add_argument("--out", type=Path, required=True, help="new lane evidence JSON")
    args = parser.parse_args()
    repo = Path(__file__).resolve().parents[3]
    frozen = read(repo / "corpus/manifest.json")
    config = read(repo / frozen["seeds_manifest"])
    example_revision = frozen["examples_rev"]
    archive = args.legacy / "out/parity/R0"
    index = read(archive / "manifest.json")
    require(index["r0"] == example_revision, "R0 is from a different source revision")
    i5 = args.legacy / "out/meta/i5-r2"
    scored = read(i5 / "gate.json")
    old = unique(((row["id"], row) for row in read(i5 / "corpus-inventory.json")), "i5 identity")
    entries = unique(((archive_id(row["entry"]), (file, row))
                      for file in sorted((archive / "entries").glob("*.json"))
                      for row in (read(file),)), "R0 identity")
    identities = unique(((key, (batch["lane"], batch["batch"]))
                         for batch in frozen["batches"] for key in batch["scenes"]), "frozen identity")
    require(len(identities) == 295 and identities.keys() == old.keys() == entries.keys(),
            "frozen, i5 and R0 identity sets differ")
    seeds = seed_sources(args.creative, config["seeds"])
    require(seeds.keys() == {key for key in identities if not key.startswith("examples/")},
            "seed set differs from the frozen corpus")
    examples = {key: git(repo, "show", f"{example_revision}:{key}")
                for key in identities if key.startswith("examples/")}
    imports = source_imports(examples)
    queue = i5 / scored["owner_approval"]["file"]
    look_ids = set(re.findall(r'^\| `([^`]+)` \|', queue.read_text(), re.MULTILINE))
    require(look_ids == {key for key, row in old.items() if row["candidate"]}, "owner queue identities differ")

    pins_dir = Path("agent_notes/2026-11-19_design/render_impl/R0g")
    pin_names = git(repo, "ls-tree", "--name-only", "-r", "origin/main", str(pins_dir)).decode().splitlines()
    bead = read(i5 / "bead.json")
    pins = {Path(name).stem: {
        "path": name,
        "sha256": sha(git(repo, "show", f"origin/main:{name}")),
        "comparison": bead[Path(name).stem],
    } for name in pin_names}
    require(len(pins) == 9 and pins.keys() == bead.keys(), "the bead geometry/depth pin set differs")

    def measured_frames(record):
        return [*record["frames"], *((record["dawn"] or {}).get("frames", []))]

    drawn = {key for key, (_, row) in entries.items()
             if any(frame["kind"] == "drawn" for frame in measured_frames(row))}
    current_cpu = {}
    scratch = {}
    current_bead = {}
    current_bead_clip = None
    current_bead_repack = None
    if args.run:
        current_cpu = unique(((row["scene"], row)
                              for line in (args.run / "candidate-cpu.jsonl").read_text().splitlines()
                              for row in (json.loads(line),)), "current CPU candidate")
        expected = {row["candidate"] for row in old.values() if row["candidate"]}
        require(current_cpu.keys() == expected, "current CPU output covers a different candidate set")
        bead_cpu = read(args.run / "bead-cpu.jsonl")
        require(bead_cpu["scene"] == old["examples/bead.ts"]["candidate"], "wrong bead CPU candidate")
        current_cpu[bead_cpu["scene"]] = bead_cpu
        current_bead_clip = read(args.run / "bead-final-clip.jsonl")
        current_bead_repack = (args.run / "bead-repack.log").read_text().strip()
        scratch = unique(((record["identity"], record)
                          for file in (args.run / "legacy-observations").rglob("observations.json")
                          for record in (read(file),)), "scratch observation identity")
        require(scratch.keys() <= identities.keys(), "scratch observation outside frozen corpus")
        for identity, record in scratch.items():
            require(record["revision"] == example_revision and record["source_sha256"] == sha(examples[identity]),
                    f"wrong scratch provenance for {identity}")
            archived = {frame["sha256"] for frame in measured_frames(entries[identity][1]) if frame["kind"] == "drawn"}
            for frame in record["observations"]:
                require(frame["original_beauty_sha256"] in archived, f"unregistered scratch shot for {identity}")
                # The run directory may have moved from a retired lane workspace.
                file = Path(frame["derived_file"]).name
                image = args.run / "legacy-observations" / identity / file
                require(sha(image.read_bytes()) == frame["png_sha256"], f"changed scratch frame {image}")
                frame["derived_file"] = str(image.relative_to(args.run))
        current_bead = read(args.run / "bead-final/score.json")
        require(current_bead.keys() == pins.keys(), "current bead score covers a different frame set")
        for frame, score in current_bead.items():
            require(score.get("reference_sha256") == pins[frame]["sha256"], f"changed reference {frame}")
            require(sha((args.run / "bead-final" / f"{frame}.png").read_bytes()) == score["candidate_sha256"],
                    f"changed candidate observation {frame}")
    rows = []
    candidate_totals = Counter()
    lanes = Counter()
    beauty_digests = {}
    for identity, (lane, batch) in identities.items():
        baseline = old[identity]
        require((baseline["lane"], baseline["batch"]) == (lane, batch), f"wrong i5 batch for {identity}")
        file, legacy = entries[identity]
        lanes[lane] += 1
        candidate = baseline["candidate"]
        if candidate is not None:
            candidate_totals[lane] += 1
        source = seeds.get(identity) if identity in seeds else {
            "revision": example_revision, "path": identity, "sha256": sha(examples[identity]),
        }
        source["legacy_entry"] = str(file)
        if identity in examples and identity not in drawn:
            source["rendered_consumer_import_chains"] = rendered_consumers(identity, imports, drawn)
        legacy_frames = []
        for frame in measured_frames(legacy):
            if frame["kind"] == "drawn":
                path = frame["file"]
                if path not in beauty_digests:
                    beauty_digests[path] = sha((archive / path).read_bytes())
                require(beauty_digests[path] == frame["sha256"], f"changed legacy beauty frame {path}")
            legacy_frames.append(frame)
        rows.append({
            "id": identity,
            "lane": lane,
            "batch": batch,
            "source": source,
            "legacy_build": legacy["build"],
            "legacy_checks": legacy["checks"],
            "archived_beauty": legacy_frames,
            "candidate_at_i5": candidate,
            "missing_reason_at_i5": baseline.get("missing_reason"),
            "independent_geometry_depth": pins if identity == "examples/bead.ts" else {},
            "scratch_legacy_observations": scratch.get(identity),
            "current_candidate_sha256": sha((repo / candidate).read_bytes()) if args.run and candidate else None,
            "current_cpu": current_cpu.get(candidate),
            "current_pinned_comparison": current_bead if identity == "examples/bead.ts" else {},
            "current_timed_clip": current_bead_clip if identity == "examples/bead.ts" else None,
            "current_bundle_refusal": current_bead_repack if identity == "examples/bead.ts" else None,
            "current_look": "owner approval needed" if args.run and identity == "examples/bead.ts" else None,
            "cpu_at_i5": baseline.get("cpu"),
            "timed_clip_at_i5": baseline.get("speed"),
            "repack_at_i5": baseline.get("bundle"),
            "look_at_i5": "owner queue" if identity in look_ids else "no submitted pair",
            "certified_at_i5": baseline["certified"],
        })
    certified = sum(row["certified_at_i5"] for row in rows)
    candidates = sum(candidate_totals.values())
    report = {
        "example_revision": example_revision,
        "r0": {"directory": str(archive), **index},
        "i5_scored_revision": scored["scored_sha"],
        "i5_owner_queue": str(queue),
        "denominator": len(rows),
        "release_minimum": scored["phase5"]["corpus"]["minimum"],
        "certified_at_i5": certified,
        "candidates_at_i5": candidates,
        "current_full_certificates": certified,
        "current_geometry_qualified": int(bool(current_bead) and all(score["passed"] for score in current_bead.values())),
        "scratch_legacy_identities": len(scratch),
        "scratch_legacy_frames": sum(len(r["observations"]) for r in scratch.values()),
        "lanes": {lane: {"candidates_at_i5": candidate_totals[lane], "total": count}
                  for lane, count in lanes.items()},
        "additional_candidates_required_if_all_current_certify": scored["phase5"]["corpus"]["minimum"] - candidates,
        "archived_beauty_digests_verified": len(beauty_digests),
        "rows": rows,
    }
    args.out.parent.mkdir(parents=True, exist_ok=True)
    with args.out.open("x") as output:
        json.dump(report, output, indent=2)
        output.write("\n")
    print(f"{certified}/{len(rows)} certified at i5, {candidates} candidates, {len(beauty_digests)} archived beauty digests verified")


if __name__ == "__main__":
    main()
