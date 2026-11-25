#!/bin/bash
# List spark pairs for voting, side by side.
# Usage: out/spark_pairs/list_pairs.sh [--ids] [--pair <id>]
# Default prints one ballot block per pair: id, criteria, A vs B + prompts.
set -u
ROOT="out/spark_pairs"
VOTES="$ROOT/votes.json"

if [ ! -f "$VOTES" ]; then
  echo "list_pairs: $VOTES not found; run: python3 $ROOT/align_pairs.py" >&2
  exit 1
fi

if [ "${1:-}" = "--ids" ]; then
  python3 -c "import json; print('\n'.join(p['id'] for p in json.load(open('$VOTES'))['pairs']))"
  exit 0
fi

if [ "${1:-}" = "--pair" ]; then
  ID="${2:?list_pairs: --pair needs an id}"
  python3 - "$ID" <<'EOF'
import json, sys
votes = json.load(open('out/spark_pairs/votes.json'))
pid = sys.argv[1]
for p in votes['pairs']:
    if p['id'] == pid:
        print(f"=== {p['id']} ===")
        print(f"criteria: {p['criteria']}")
        print(f"A: {p['a']['render']}")
        print(f"   prompt: {p['a']['prompt']}")
        print(f"B: {p['b']['render']}")
        print(f"   prompt: {p['b']['prompt']}")
        print(f"created_by: {p['created_by']}")
        break
else:
    sys.exit(f"list_pairs: unknown pair id {pid!r}")
EOF
  exit 0
fi

python3 - <<'EOF'
import json
votes = json.load(open('out/spark_pairs/votes.json'))
pairs = votes['pairs']
print(f"{len(pairs)} pairs ready for voting\n")
for p in pairs:
    print(f"=== {p['id']} ===")
    print(f"criteria: {p['criteria']}")
    print(f"A: {p['a']['render']}")
    print(f"   prompt: {p['a']['prompt']}")
    print(f"B: {p['b']['render']}")
    print(f"   prompt: {p['b']['prompt']}")
    print(f"created_by: {p['created_by']}")
    print(f"vote: record '<id>: a|b' beside the pair")
    print()
EOF
