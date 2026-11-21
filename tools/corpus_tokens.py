# /// script
# dependencies = ["tiktoken==0.14.0"]
# ///
"""Report the TOK1 live-corpus population and optional AD inline controls."""
import argparse
import csv
import math
from pathlib import Path
import statistics
import sys

import tiktoken


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("baseline", type=Path)
    parser.add_argument("candidate", type=Path)
    parser.add_argument("--inline", type=Path)
    args = parser.parse_args()
    encode = tiktoken.get_encoding("o200k_base").encode_ordinary
    counts = []
    table = csv.writer(sys.stdout)
    table.writerow(("scene", "before", "after", "inline"))
    for group in ("l5c", "l5d", "l5e", "seeds"):
        for path in sorted((args.candidate / "corpus" / group).rglob("*.loft")):
            relative = path.relative_to(args.candidate)
            before = len(encode((args.baseline / relative).read_text()))
            after = len(encode(path.read_text()))
            control = args.inline / path.name if args.inline else None
            inline = len(encode(control.read_text())) if control and control.exists() else after
            counts.append((before, after, inline))
            table.writerow((relative, before, after, inline))
    for label, column in zip(("before", "after", "inline"), zip(*counts, strict=True), strict=True):
        ordered = sorted(column)
        print(f"{label}: n={len(ordered)} median={statistics.median(ordered):g} "
              f"p95={ordered[math.ceil(0.95 * len(ordered)) - 1]} total={sum(ordered)}", file=sys.stderr)
    print(f"AD helper net corpus savings: {sum(c - b for _, b, c in counts)} tokens", file=sys.stderr)


if __name__ == "__main__":
    main()
