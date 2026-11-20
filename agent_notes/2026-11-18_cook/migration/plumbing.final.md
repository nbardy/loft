Three seed ports reproduced all four saved subject-frame hashes. The fourth recorded a budget failure. The offline trainer check passed for all five task types.

```json
{
  "creative_commit": "fbb92b7ba44113dbd62f7ae1f05cd01532bc46e9",
  "vendor_commit": "e7c3c5a7abfa948b185a2cf7151c0b7f81d160a2",
  "branch": "loft/migrate",
  "worktree": "clean",
  "smoke_results": {
    "crab-improve-v1": {
      "train-0/0": "budget failure at frame 0, 75.29 ms against the 50 ms limit",
      "train-14/2": "4 of 4 hashes matched"
    },
    "crab-actions-v1": {
      "r38retry-before/before-2/0": "4 of 4 hashes matched",
      "r38retry-before/before-0/1": "4 of 4 hashes matched"
    }
  },
  "files_changed": {
    "configs/loft": ["all-five-check.jsonc", "all-five-check.tasks.jsonl", "first-run.jsonc"],
    "training/loft": ["README.md", "config.py", "data.py", "library.py", "port_seed.py", "render.py", "taskgen.py", "test_taskgen.py", "tinker.py"],
    "training/loft/host": ["allowed-imports.json", "bundle.ts", "confine.py", "fixtures/lit.ts", "import-policy.json"],
    "training/loft_envs/seed_programs": ["PORTING.md", "crab-actions-v1/build.py"],
    "vendor": ["loft"]
  }
}
```

Exact smoke commands:

```sh
UV_NO_SYNC=1 uv run python -m training.loft.port_seed --set crab-improve-v1 --ids train-0/0,train-14/2 --code-dir /tmp/creative-loft-port-smoke-no9nl218/code --out /tmp/creative-loft-port-smoke-no9nl218/ported-seeds.jsonl --ported-by loft-migration-codemod
UV_NO_SYNC=1 uv run python -m training.loft.port_seed --set crab-actions-v1 --ids r38retry-before/before-2/0,r38retry-before/before-0/1 --code-dir /tmp/creative-loft-port-smoke-no9nl218/code --out /tmp/creative-loft-port-smoke-no9nl218/ported-seeds.jsonl --ported-by loft-migration-codemod
```

The import allowlist check, focused tests, and frozen replay passed. A 16-frame WGSL and `draw.points` canary peaked at **31.90 ms** per subject frame. Both the CPU overrun and Loft exit 7 reached the scored-zero path. The GPU check uses serve’s per-frame wall time including readback as a conservative bound. Renders were verified on macOS.

For the porting agent, provide reviewed `<safe-id>.ts` files and keep the ledger beside its `.renders` directory. The `first-run` draft still needs the reviewed seed ledger and task rows for the accepted ports. [Porting guide](/Users/nicholasbardy/git/_wt/creative-loft-migrate/training/loft_envs/seed_programs/PORTING.md)