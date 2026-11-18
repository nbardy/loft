# Key learnings: read first

One line per incident. Most are Keel's (room-runners-arena-lib), from the same shared 10-core M4 that Loft builds on;
Loft copied the mechanism with the incident (plan `docs/proposals/2026-11-19_lean_rust_loft.md` §6). Add a line when a
mechanism here earns or loses its place, with the date and the numbers.

## Building Rust

- Build through `tools/dev.sh` and `tools/check.sh`, never bare `cargo`. Write in your journal what you expect a
  compile to show before you run it; fix everything one run reports before compiling again; one compile per agent.
- 2026-11-01 (Keel): many concurrent compiles drove load to 60-200, and a 54 CPU-second rebuild took 8 min wall. One
  machine-wide lock (`tools/lib/build_gate.sh`, the same `~/Library/Caches/keel-build-gate` files as Keel) queues
  builds instead. HEAVY for builds and gates, LIGHT (4 jobs, niced) for a single-crate check, lint or test.
- 2026-11-02 (Keel): a 70-minute batch held the compile lock and stalled every lane. Long runs (renders, parity,
  benches) start after the build, outside the lock, under `nice -n 15`; `dev.sh run` releases the lock before exec.
- 2026-10-30 (Keel): a fresh `CARGO_TARGET_DIR` per task made ~100 folders and 64 GB of private rebuilds. Only
  `rust/target` is admitted; another folder needs `LOFT_TARGET_OWNER` and its owner deletes it.
- 2026-10-30 (Keel): a target cloned into another checkout reported every crate Fresh and ran the other tree's code
  (unit hashes ignore the checkout path). Never share or copy a target; sccache is the cross-worktree reuse.
- 2026-11-01 (Keel): Homebrew's cargo came first on PATH and its newer Clippy failed crates the pinned 1.99.1 passes.
  `cache-env.sh` puts the rustup proxy first and refuses to run without rustup.
- 2026-10-30 (Keel): a warm script lacking the gate's `-D warnings` re-linted 175 units on every switch. Gate flags
  live only in `tools/lib/gate_flags.sh`; `dev.sh lint` uses them, so a second lint compiles 0 units.
- 2026-10-30 (Keel): seventy integration-test executables linked the engine each (309 MB, ~15 s per edit). One test
  binary per crate: `tests/it/main.rs`, a new test file is a `mod` line; `check.sh static` enforces it.
- 2026-11-01 (Keel): a one-line edit cost 64-71 CPU-s in release and 9-10 in the `lab` profile. Use `--fast` for
  optimized loops; release only for shipped artifacts and performance numbers.

## Lanes and gates

- 2026-10-30 (Keel): without an explicit base commit Cambium based a lane on the primary checkout's HEAD.
  `tools/lane.sh new` always passes one.
- 2026-10-30 (Keel): Cambium reports every workspace ephemeral, the primary checkout included. `lane.sh done` removes
  only a clean lane under `~/git/_wt/`.
- 2026-11-01 (Keel): the serial no-compile checks summed to 15-45 s under load; concurrent, the slowest one is the wall
  time. `check.sh static` prints both.
- Workers run focused checks; the integrator runs `tools/check.sh full` once per wave. Every timing JSON records
  `{load1, cpus}` at start and end, and a run taken under load is marked.
