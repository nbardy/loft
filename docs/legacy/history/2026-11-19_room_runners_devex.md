# Room Runners (Keel) developer experience, and Keel on Loft (19 Nov 2026)

Read-only study of `~/git/room-runners-arena-lib` (Keel, Rust) and `~/git/room-runners-arena-lib-lean-scope`. Nothing
was built. Two trees matter: `main` (56b025f7) and the active branch `keel-complete` (b82d8b8c, 818 commits ahead,
checked out at `~/.claude/worktrees/room-runners-arena-lib-af23afd9/keel-complete`). Where they differ, this note
follows `keel-complete`. Paths below are relative to that repo unless they start with `~`.

The lean-scope folder is not a repository. It is the plan folder for the October deslop of the old TypeScript game:
measure, then inventory, then a target with line budgets, then a board, then waves (`README.md`, `target*.md`,
`inventory/`, `lanes/`). Its method is the same as the `deslop` skill's. It holds nothing about Rust or Loft.

## Part 1. How Keel works, mechanism by mechanism

Every mechanism exists because of a dated incident on one shared 10-core M4 with many agents. The comment beside each
mechanism names the incident and its numbers. Loft should keep that habit as well as the mechanisms.

### 1.1 One target per worktree. Sharing goes through sccache and a registry-only seed.

Keel does **not** share a mutable target directory between worktrees. That is the most important finding.

- **`keel/scripts/cache-env.sh`**, sourced by every wrapper, sorts the output directory into one kind: `primary`
  (`keel/target`), `experiment` (`target/agent-parallel`, `target/agent-cranelift`), `owned` (another subdirectory,
  allowed only with `KEEL_TARGET_OWNER=<task>`, and the owner deletes it), or refused (`unowned`, `outside`). It
  resolves symlinks and `..` in pure shell first; this replaced a python3 spawn that cost about 30 ms per call.
  Incident, 2026-10-30: agents exported a new `CARGO_TARGET_DIR` for each task. About 100 folders held 64 GB, and each
  was a private full rebuild that nothing later reused. `scripts/test-cache-env.sh` guards these rules in the static
  gate.
- **Why sharing a target is unsafe.** Cargo's unit hashes for workspace members do not depend on the checkout path.
  When a whole target was cloned into a second checkout whose sources kept their mtimes, Cargo reported every member
  Fresh. The binary then printed the *origin's* `env!("CARGO_MANIFEST_DIR")`. Keel bakes manifest paths in for shader
  reload, so a shared target silently runs the wrong tree's shaders. `scripts/test-seed.sh` reproduces this as a
  control.
- **Cross-worktree reuse is sccache.** `cache-env.sh` sets `RUSTC_WRAPPER=sccache` and `SCCACHE_CACHE_SIZE=6G`, and
  incremental compilation stays off (`CARGO_INCREMENTAL=0`, `build.incremental=false`) because sccache refuses
  incremental units. `CARGO_INCREMENTAL=1` is an explicit opt-in for a long personal edit loop and drops the wrapper.
  This machine already sets `rustc-wrapper = sccache`, `jobs = 3` and `RUST_TEST_THREADS=4` in `~/.cargo/config.toml`,
  so Loft's Rust crates get sccache today without doing anything.
- **Seed (`scripts/seed.sh`, `scripts/lib/seed.py`).** It warms the gate, then APFS-clones (`cp -c`; reflink on
  Linux) *only* registry and git dependency units into `~/Library/Caches/keel-seed/<rustc release>/target`. Units are
  whitelisted by `.fingerprint/<package>-<hash>`. There is one snapshot per toolchain, swapped atomically. A new lane
  starts with third-party crates already built and compiles its own ~70 crates once. Republish after a toolchain or
  `Cargo.lock` change; a stale seed only costs more compiles. `scripts/check-seeded-lane.sh LANE` proves it on real
  crates.
- **Sweep (`scripts/sweep.sh [--yes]`).** It runs `warm.sh --gate` under
  `CARGO_LOG=cargo::core::compiler::fingerprint=debug` and deletes only `deps/` hash groups that the trace does not
  name. Measured: 4,426 paths removed, then 0 of 1,752 gate units recompiled. The earlier rule, "keep the newest two
  by mtime", evicted live variants, so the next gate rebuilt 167 crates in ~114 s. A warm build rewrites no file, so
  no age rule can tell a live build from a dead one.
- **Workspace-hack (`crates/keel_hack`, cargo-hakari 0.9.39).** Without it, `dev.sh test game_room` (`-p`) resolved
  different serde and postcard features than the `--workspace` gate, and right after a warm gate compiled 24 units
  in 50.7 s. With the hack it compiles 0 units in 1.4 s. The hack is a dev-only edge: members depend on it under
  `cfg(keel_hack)`, which only `cache-env.sh` sets, so the production `cargo build --release -p host_server` does not
  link the GPU union. That server tree had been 174 crates including wgpu; `check-server-deps.sh` keeps it out. wgpu
  is traversal-excluded from the hack, which took `dev.sh check keel_math` from 148 crates down to 89.
  `check-feature-unification.sh` proves that `-p X` and `--workspace` agree for every member, and it must be run
  after bumping wgpu or winit.
- **Gate flags in one file (`scripts/lib/gate_flags.sh`).** Clippy's `-- -D warnings` and `--all-targets` are part of
  each unit's fingerprint. When `warm.sh` lacked `-D warnings`, every switch between warm and gate re-linted 175
  units.
- **One integration-test binary per crate (`tests/it/main.rs`).** Seventy separate test executables each linked the
  engine: 309 MB of binaries, and about 15 s more per `--no-run` after a `keel_math` edit. A new test file is now a
  `mod` line. `check-test-layout.sh` enforces this.
- **Profiles.** `[profile.lab]` inherits release with `lto = false` and `codegen-units = 16`. A one-line edit to
  `host_server` cost 64–71 CPU-s in release and 9–10 CPU-s in lab, and lab is used for every optimized loop
  (`dev.sh build X --fast`). Dependencies build at `opt-level 3` in dev; tests build at `opt-level 1` with no
  debuginfo. Two changes were rejected on measurement: sim `opt-level 3` in the test profile (+30 CPU-s per rebuild,
  tests ~1 CPU-s) and `build-override opt 3` (equal medians over 10 interleaved pairs).
- **Toolchain pin.** `rust-toolchain.toml` pins `1.99.1` with the minimal profile. `cache-env.sh` puts the rustup
  proxy first on PATH. Incident, 2026-11-01: Homebrew's cargo 1.99 came first on PATH, and its newer Clippy failed
  two crates that the pinned toolchain passes. The nightly parallel frontend and Cranelift are opt-in lanes with
  their own target directories. They are not the default because parallel was 14% *slower* at load 14, and
  Cranelift code ran ~26–30% slower and aborts on panic (`reports/build-perf/2026-10-30/toolchains.md`).

### 1.2 Machine-wide compile locks

`keel/scripts/lib/build_gate.sh`, described in `DEV_LOOP.md` under "Compile rarely, compile small, queue":

- **Incident (2026-11-01).** Many agents compiled at once and the load reached 60–200. A small release rebuild took
  8 min wall for 54 s of CPU. Builds that run one after another finish sooner than builds sharing cores.
- **Mechanism.** A small perl process takes `flock(2)` on `~/Library/Caches/keel-build-gate/slot.0` and holds it
  while the caller's PID lives. The kernel drops the lock on exit, including after `kill -9`, so a stale lock can
  never exist. The lock survives `exec cargo`. The holder never passes its file descriptor to cargo, because the
  sccache server that cargo starts would inherit it and hold the lock forever. Nested scripts see `KEEL_GATE_HELD`
  and do not wait again. A waiter prints the holder and waits; it never fails. `build_gate.sh status` shows the
  holder; `build_gate.sh run cargo ...` wraps a direct cargo call; `KEEL_BUILD_GATE=off` turns it off on a machine
  you own.
- **Two compile slots (on keel-complete, 2026-11-03).** HEAVY (`slot.0`) is for check.sh, warm, build/run and web
  builds, at full `-j` (`CARGO_BUILD_JOBS=ncpu`, overriding the global `jobs = 3`). LIGHT (`slot.light`) is for a
  single-crate `dev.sh check|lint|test`, at 4 jobs and reniced +10. At most two compiles run per machine. The LIGHT
  slot exists because a 10 s crate check had queued behind another lane's 10–20 minute gate.
- **SIM slot (`slot.sim`).** `keel_cli tidewar` takes it itself with `std::fs::File::lock`, so only one simulation
  batch plays per machine. **The locks are for compiling only.** Incident, 2026-11-02: a 70-minute balance batch held
  the compile lock and stalled every lane. Long runs now start after the build, outside the lock, under `nice -n 15`.
- **History worth copying.** This replaced a 2-slot FIFO ticket queue plus a per-rustc `RUSTC_WRAPPER` shim and a
  planner, about 800 lines with tests. One lock at full `-j` gives the same protection with nothing to tune.
  `test-build-gate.sh` covers exclusion, `kill -9`, nesting and stale exports, and it is the first step of the full
  gate.

### 1.3 Cambium lanes

- `.cambium.toml` sets `require_cow = true` and `policy = "skip"` for `target` and `keel/target`, so Cambium never
  seeds build trees.
- `keel/scripts/lane.sh new NAME BRANCH` runs
  `git cambium add -b BRANCH -name NAME -ephemeral ~/git/lanes/room-runners/NAME <this checkout's HEAD>`. The explicit
  commit matters: without it Cambium bases the lane on the main checkout's HEAD. On 2026-10-30 a lane made from
  keel-complete came up on main without `keel/`. The script then clones the kit submodule locally (no network, 0.4 s),
  because Cambium does not populate submodules, and applies the seed.
- `lane.sh done NAME` refuses any lane that is not under the lane root. Every Cambium workspace reports
  `ephemeral: true`, including the primary checkout, so containment is the real guard. It also refuses a dirty lane
  or unpushed kit work, sweeps lane-end compiler state, and removes the kit clone without `git submodule deinit`
  (deinit unregistered the kit in *every* worktree on 2026-10-30). Then it runs `cambium remove`.
- Loft already uses Cambium worktrees (`cambium list` shows `loft-rust-core`, `loft-migration` and `loft-api-l8` under
  `~/git/_wt/`, all `apfs-clone`), but it has no `.cambium.toml` and no lane script.

### 1.4 Verification tooling

- **`dev.sh check|lint|test|build|run CRATE`.** It uses the pinned toolchain, `--locked`, `keel/target`, the compile
  lock, and gate flags for `lint`, so a crate lint shares units with the gate. `dev.sh plan` lists changed crates and
  their reverse dependencies. `dev.sh plan --proof` prints which proof the diff needs and the exact commands, from
  `scripts/lib/proof_table.json`, where the first matching path kind wins. An unmapped path is reported, never
  ignored.
- **`check.sh static | quick | full`.**
  - `static` runs every check that compiles nothing, concurrently, with buffered output and per-job seconds.
    `cargo fmt --all` was sharded into 5 `-p` groups: 12.0 s became 3.1 s at load 150. The serial version took
    22–28 s and the parallel one 4.6–8.5 s at load 145–193.
  - `quick` classifies the change set once (`scripts/lib/quick_plan.py`): the paths changed since
    `merge-base HEAD $KEEL_BASE`, plus staged, unstaged and untracked files. Changed members and their transitive
    reverse dependencies get clippy and tests. A change to a workspace-wide input (Cargo.toml or lock, `.cargo`,
    content, flag scripts) falls back to the full gate and says why. A missing base ref is an error, never a silent
    empty or whole-tree selection. `cargo metadata` is cached by a sha256 of its inputs (0.015 s against 1.3–1.8 s).
  - `full` runs once per merged batch, by the integrator.
- **Tests run through cargo-nextest (decision #109, `.config/nextest.toml`).** Four tests run at once. Test groups
  `gpu` and `simulation` have `max-threads = 1`. A test is reported SLOW every 30 s and killed as a TIMEOUT failure
  after 5 minutes. There is one retry, used only to label a failure FAIL or FLAKY, and `flaky-result = "fail"`.
  `known-flaky` is empty. junit output goes to `target/nextest/default/junit.xml`, where slow tests can be found.
- **Staged browser audits (`keel/scripts/audit-url.mjs`).**
  - Named stages run serially, one owned child process each. The output is `report.json`, rewritten after every
    stage, plus `contact-sheet.html`.
  - At the start and end of every stage the runner records `os.loadavg()[0]` and the CPU count
    (`runs[].stages[].load.start|end`), and the contact sheet marks a stage `(loaded)`.
  - `--wait-quiet [LOAD]` waits, for at most `--wait-quiet-max` (600 s), until the 1-minute load drops below LOAD (by
    default the CPU count), then starts anyway and records `startedLoaded`. It never changes an assertion or a
    timeout. Why: `versus` failed two of three runs with 20 s Playwright timeouts at load 120–140, and nothing in the
    report said so.
  - `--repeat N` writes each run to `run-K` and keeps going after a failed run, so an intermittent failure is
    measured rather than hidden. `--scene NAME` starts a server on an audit scene (`content/audit/NAME.json`), so a
    stage begins at the moment it checks instead of playing a match in real time to reach it.
- **Proof by kind of change (decision #99, in `AGENTS.md` under Verification).**
  - Gameplay changes are proved with crate tests plus headless `keel_cli tidewar` batches, where a 45-minute match
    takes seconds.
  - Visual or client changes need a matched native/WASM pair and only the audit stage that covers the change, on
    real GPU pixels.
  - A deploy needs one connect, play and reconnect run.
  - No test is rerun to make it pass, and no assertion is weakened for speed.
- **Opt-in pre-commit:** `git config core.hooksPath keel/scripts/hooks` runs `check.sh static` (~4 s warm).
- **Landing (`keel/scripts/land.sh`).** `commit "MSG" PATH...` commits only the named paths. `head` and `branch`
  merge into `origin/main` with `git merge-tree --write-tree` and `commit-tree`, without a checkout, so a busy tree is
  never touched. It never force-pushes. `test-land.sh` tests it against a throwaway bare remote.
- **Timing seams for tests.** `keel_session::sessions::Timing` has a `PRODUCTION` constant and
  `const scaled_down(n)`. A test injects a scaled `Timing`, and separate tests pin the `PRODUCTION` values. One
  speed-swarm item (S4) took the liveness test from 21 s to 1.4 s and `keel_preview remote::` from 34.7 s to 8.0 s
  without weakening a deadline. Mutants verified each new test.

### 1.5 How agents are told to work (`AGENTS.md`, `DEV_LOOP.md`, `key_learnings/`)

- **"This codebase is yours" (decision #100).** Agents improve the structure, fix the nearby problem, record design
  calls as numbered files in `notes/design/decisions/`, and commit and merge checked lanes. Production, spending,
  other repos and evidence stay with the owner. No assertion is ever weakened.
- **"Write a lot, compile rarely."** Reason from the source and write in the journal what you expect before
  compiling. Compile the smallest slice. Fix everything one run reports before compiling again. Run one compile at a
  time per agent. Simulations and audits run outside the lock, niced, and never beside your own compile.
- **Append-only journal.** `agent_notes/YYYY-MM-DD_topic.md` records what was read or run, what was observed, what
  failed and what was decided. `key_learnings/*.md` holds curated, compressed lessons, and agents read them at the
  start of a session.
- **Workers (2026-11-17).** The lead delegates to the cheapest capable model and gives each worker a brief in a file,
  a journal path, a narrow scope and the files it must not touch. There is one compiling worker per worktree, and the
  lead reviews every diff. Codex "Sol" workers handle maths through `keel/scripts/codex-worker.sh DIR`.
- **The speed swarm (`agent_notes/2026-11-01_speed-swarm.md`)** is the model for tooling work. It had eight items
  (S1–S8), each measured before and after on the loaded machine, using CPU-seconds when wall time was useless. Then
  an integrator ran the full gate (1185 pass, 0 fail), two adversarial reviewers checked the batch, and a fixer pass
  followed. Several of its own items failed first (clippy on a probe example; a test-only sweep in the production
  core loop), and those failures are recorded.

## Part 2. What Loft should adopt

Loft today: Bun plus TypeScript, `pnpm test`; one Bun test process per user across worktrees through an `O_EXLOCK`
lock (`tests/setup.ts`, exit 75); worktrees under `~/git/_wt/`. It has two separate Cargo packages,
`rust/loft-loader` and `rust/loft-core` (the latter on `feat/rust-core`), each with its own `Cargo.lock` and
`target/`. No toolchain pin, no workspace and no wrapper script.

| # | Adopt | Concrete form in Loft | Pass / fail |
|---|---|---|---|
| A1 | One Cargo workspace | `rust/Cargo.toml` with members `loft-loader`, `loft-core`; one `Cargo.lock`, one `rust/target` per worktree | `cargo metadata` lists both; one lockfile; wgpu resolves once |
| A2 | Pinned toolchain | `rust/rust-toolchain.toml` (`1.99.1`, minimal, rustfmt+clippy, `wasm32-unknown-unknown`), same as Keel | `rustup show` in `rust/` names 1.99.1; the wrapper refuses a non-rustup cargo |
| A3 | One wrapper, `rust/dev.sh check\|lint\|test\|build CRATE` | `--locked`, the pinned toolchain, `rust/target`, gate flags from one file, `--fast` = a `lab` profile | a crate lint and the gate share units: the second run compiles 0 units |
| A4 | Target directory admission | port `keel_output_kind`: `rust/target` only; any other path needs `LOFT_TARGET_OWNER`; outside the worktree is refused | `CARGO_TARGET_DIR=/tmp/x rust/dev.sh check loft-core` exits 2 with the reason |
| A5 | Share the **same** machine lock as Keel | source Keel's `build_gate.sh` protocol: the same `~/Library/Caches/keel-build-gate/slot.0` / `slot.light` files and holder semantics (copy the ~100-line script; do not invent a second lock) | while a Keel `check.sh` holds HEAVY, a Loft `dev.sh build` prints the holder and waits |
| A6 | sccache, incremental off | already global; keep `CARGO_INCREMENTAL=0` in the wrapper, opt-in `=1` drops the wrapper | `sccache --show-stats` hits climb on a second worktree's first build |
| A7 | No shared targets; registry-only seed only if needed | Loft's dependency tree is small (wgpu, serde, thiserror, png): measure the first build in a fresh lane before porting `seed.py` | port the seed only if a fresh lane's dependency build exceeds ~60 s CPU |
| A8 | `.cambium.toml` | `require_cow = true`; `skip` for `rust/target`, `out/`, `node_modules` handled as Cambium does now | a new lane has no `rust/target` and a cloned `node_modules` |
| A9 | Lane script | `tools/lane.sh new\|done\|check` with an explicit base commit, containment under `~/git/_wt/`, and a refusal when dirty | `done` on the primary checkout refuses |
| A10 | Static gate, concurrent | `tools/check.sh static`: `tsc --noEmit` (through `rtk proxy`), `cargo fmt --check`, `check_loader_closure.ts`, the Naga validation of built modules (loader README "proposed format changes" 1), the lockfile pin check (B6), each buffered, with per-job seconds | wall time printed below the serial sum; any failed job fails the gate |
| A11 | Quick plan | map changed paths to tests: `src/**` to the suite subset, `rust/<crate>` to that crate plus its reverse dependencies, `docs/**` to nothing, with a named full fallback for `package.json`, `bunfig.toml` and `Cargo.lock` | an unmapped path is printed, never dropped |
| A12 | Test runner discipline | nextest for Rust (`gpu` group with `max-threads = 1`, 5 min TIMEOUT, `flaky-result = "fail"`); for Bun keep the user lock, and record pass and fail per test in a junit-like file | a hang is a TIMEOUT, never a stall; a pass on retry is reported FLAKY |
| A13 | Load recorded in every measured report | `loft profile`, `tools/pose_time.ts`, `tools/tracer_bench.ts` and serve benches write `{load1, cpus}` at start and end; `--wait-quiet` before timing runs | every timing JSON has `load`; a timing taken loaded is marked |
| A14 | Long jobs outside the lock, niced | GPU renders, training-serve probes and corpus runs start after the build, `nice -n 15` | `build_gate.sh status` is free while a corpus run plays |
| A15 | Journal and learnings | Loft has `agent_notes/`; add `key_learnings/` with a README that agents read first, and the expectation line written before each compile | — |
| A16 | One integration test binary per Rust crate | `tests/it/main.rs` from the start | `ls target/debug/deps/*it-*` shows one per crate |

Not worth copying: the nightly parallel and Cranelift lanes (measured slower or riskier under load); hakari, until
Loft has more than a few members, because its job is to keep `-p` and `--workspace` in agreement and two crates
hardly diverge; `land.sh`, unless Loft moves to remote-merge landing.

## Part 3. Keel and Loft today

### 3.1 What exists

- **Keel uses no Loft code.** Nothing under `keel/` depends on `loft-loader` or reads an artefact. The word "loft" in
  Keel means three other things:
  1. The lab experiments `keel/lab/loft_sea`, `loft_blend` and `loft_spacetime` (2026-11-10). These are the Rust
     proofs of sections 11.1, 11.2 and 11.4 of `notes/design/2026-11-10_creature_pipeline_options.md`: a curved scene
     graph, junctions blended in rho, and an attack defined as one space-time object. They render through
     `keel_present::scene3d` and `keel_render`, live in the separate lab workspace, and no game crate depends on them.
  2. Sections 10–12 of that design note, which named and specified Loft. Its main points: TypeScript is the authoring
     host; the compiler and runtime are Rust; the artefact is a serialized graph; the shipped game loads graphs and
     never runs authoring code; the sim owns gameplay shapes and Loft derives the look from them.
  3. Ordinary hull and boss "lofts" in art crates, such as `art_boss_wreckers_lamp::art::loft`.
- **Two recent design notes put Loft out of scope for now.** `notes/design/2026-11-17_tidewar_class_weapons.md:249`
  says LOFT "is not needed". `notes/design/2026-11-18_tidewar_class_weapon_art.md:158` says "nothing moved from LOFT".
  `agent_notes/2026-11-13_loft-literal-index-report.md` on main is about the training team's wrapper, not about Keel.
- **How Keel draws a boss today.** Art crates (`keel/games/art/art_boss_*`, `art_boat_*`) implement
  `showroom::Asset`. They are GPU-free (`keel_present` only, and `check-gpu-free-art.sh` resolves the real dependency
  graph) and emit a CPU `Scene3D` every frame (about 1 ms per boss, MEASURED in the design note). `game_client`
  (`boss_art.rs`, `fleet_art.rs`) calls them, and `keel_render`'s `MeshPass` draws the result.
- **Bosses on the Loft side.** Loft ported five Keel bosses (branches `boss/{lamp,thundermaw,titan,tithe,wyrm}` and
  `pass2/*`, `examples/bosses/`). `docs/proposals/2026-11-18_next.md` says what Loft is for: "creatures and bosses for
  the game (Keel)".
- **`rust/loft-loader`.** It is on wgpu `30.0.1`, which matches Keel's lock (`wgpu 30.0.1`, `naga 30.0.1`). It takes a
  device and queue it does not own, has typed errors, a `PipelineCache` (Vulkan), and `loft-view` for pictures and
  timing. It is 2,933 lines including the binary. The Metal proof measured an 8–14% GPU slowdown against Dawn and
  64/294 → 0/407 modules refused by Naga after f32 rate folding.

### 3.2 What is missing for "Keel runs on Loft"

The loader's README already names the first five rows ("What the game needs next"). The rest come from Keel's side.

| Gap | Keel today | Loft today | Needed |
|---|---|---|---|
| Frame ownership | `keel_render` owns the frame: surface format (rgba8unorm or bgra8 sRGB variants), its own clears | every frame ends in Loft's own `rgba8unorm` texture, cleared, with alpha that is not coverage | `encode_draws` into the caller's colour attachment, with `LoadOp::Load` and no output clears, in the caller's format; or Loft's shaders accept an sRGB flag so values are not encoded twice |
| Depth | `Depth32Float`, **sample_count 1** (`keel_render/src/scene3d.rs:1125`) | `depth32float` **4x MSAA**, its own near/far (distance ÷ 100, × 100) | Loft draws with the game's depth attachment: same sample count (or Keel moves to 4x), the game's projection |
| Camera | Keel's camera produces view and projection matrices; the fog/overview far plane is shared between rasterized and raymarched depth | `set_camera` takes eye, target and fovY | `set_view_projection(view, proj, near)`; the LOD plan reads `near` |
| Surroundings | Keel owns sea, sky, fog and terrain | a boss artefact includes its sea, sky and post pass | `loft build --subject-only`, or layer-tagged draws the loader can skip |
| Many instances | 10+ boats and up to 5 boss types on screen | one `Loft` = one full set of passes | a "draws" part of the manifest that the game schedules inside its own passes; instancing per artefact |
| Browser (WASM) | `host_web` is wasm32 with `wgpu` features `webgpu, wgsl`; no blocking allowed | `pollster::block_on` at load (`src/lib.rs:127`); features `metal, vulkan, dx12`; `read_dir` uses the filesystem | an async load path; a `from_bytes(manifest, chunks, blobs)` reader (fetch or `include_bytes!`); features chosen per target as Keel does; `cargo check --target wasm32-unknown-unknown` in the gate |
| Lints | workspace `unsafe_code = "deny"`, `float_arithmetic = "deny"` except presentation/render crates | `PipelineCache::open` is `unsafe` | keep the loader outside Keel's workspace as a path or git dependency, or expose a safe cache API so Keel's lints hold |
| GPU containment | wgpu only in keel_gpu, keel_render, game_client_gpu, hosts and tools; the server never links wgpu (`check-server-deps.sh`) | — | loft-loader is reached only through `keel_render` (or a new `keel_loft` GPU crate); `check-no-web-deps`, `check-server-deps` and `cargo tree -e normal -i wgpu` stay green |
| Feature union | native features `metal, vulkan, gles, wgsl` | adds `dx12, parking_lot, std` | match Keel's set or accept the union; run `check-feature-unification.sh` |
| Asset pipeline | content is compiled in (`include_str!` of `content/*.json`); the WASM bundle is built by `build-web.sh` | `loft build <scene> --out <dir>` needs Bun and the TypeScript compiler | Keel's gate must not need Bun: commit built artefacts (or fetch them by content hash) with the Loft commit and format version in `manifest.json`; a `keel/scripts/loft-sync.sh` that rebuilds them from a pinned Loft SHA is a dev step, not a gate step |
| Hot reload | debug `keel_preview` reloads three WGSL files, validated under an error scope before swap | params, inputs and curves can change with no compile (`set_param`, `set_curves`); a rebuilt artefact is a new directory | the preview watches an artefact directory and calls `Loft::load` on change, keeping the old one on error (the same pattern as WGSL reload); param edits go through `set_param` live |
| Determinism and netcode | sim crates: no floats, no HashMap iteration, `DetRng`; server-authoritative; the client renders interpolated snapshots; `PROTOCOL` generation | GPU floats; a frame is a function of (scene, camera, time, size, params, inputs, …) | Loft is presentation only: inputs come from sim state (`set_input("strike.at", …)`, time from the authoritative tick). Hit shapes stay in the sim (`game_bships`); a Loft space-time attack is built *from* the sim's timing and hit shape, never the reverse (design note 12.1). No Loft output feeds back into the sim. Nothing in Loft changes the wire |
| Checks and budgets | `audit-performance.mjs` with GPU timestamps per pass; the browser `performance` stage; `keel_gallery` byte identity on the real GPU | `loft profile`, `loft-view --frames`, `set_timestamps` | a per-boss GPU budget measured with Keel's own timer (`__keelGpu`) at the game camera and DPR 1.5; a `keel_gallery` still per Loft boss under the `gpu` test group |
| wgpu pin | `wgpu = "30"`, locked at 30.0.1 | `"30.0.1"` | one check (static, in both repos) that both lockfiles name the same `wgpu`/`naga` version; bump together |
| Toolchain and edition | 1.99.1, edition 2024 | none pinned, edition 2021 | pin 1.99.1 in Loft (A2); edition 2021 is fine as a dependency |

### 3.3 Integration plan

Each step names its check. The order follows the rule Keel already lives by: the sim owns gameplay, the renderer
only observes, GPU code stays in GPU crates.

0. **Decide where the bridge lives (Keel decision file).** Recommended: a new GPU crate `keel/crates/keel_loft` that
   depends on `loft-loader` by git (pinned rev) or by path from `~/git/loft/rust`, and is used only by `keel_render`
   and the `keel_preview`/`keel_gallery` tools. *Pass:* `cargo tree -e normal -i loft-loader` lists only GPU crates
   and tools; `check-server-deps.sh` passes; `host_server` links no wgpu.
1. **Loft side: a frame that does not own the frame.** Add to `loft-loader` an `encode_into(encoder, color, depth,
   view_proj, near, layers)` that loads instead of clearing, takes the caller's format and depth, and skips
   surroundings. Add `loft build --subject-only`. *Pass:* `loft-view` draws a boss over a solid background through
   the new path; pixels match today's subject region within the 8/255, 0.05% rule of `gpu_native.md`; a check fails
   if any pass clears an attachment it does not own.
2. **Loft side: a WASM-capable loader.** `from_bytes` reader, async `load`, per-target wgpu features matching Keel's.
   *Pass:* `cargo check -p loft-loader --target wasm32-unknown-unknown`; the native path is unchanged
   (`loft-view` hash equal).
3. **Shared pin check.** A script in each repo compares `wgpu`/`naga` versions in Keel's `keel/Cargo.lock` and
   Loft's `rust/Cargo.lock`. *Pass:* both static gates run it; a deliberate bump on one side fails the other.
4. **Keel side: one boss through Loft in the native preview, beside the CPU `Scene3D` version.** Start with the
   Wrecker's Lamp, which already exists on both sides. Artefact committed under `keel/content/loft/lamp/` with
   `loft_commit` and `format_version` in its manifest. Inputs from sim state only (boss phase, tick time, target).
   *Pass:* `keel_preview --world bships` shows it at the game camera with correct occlusion against hulls and sea
   (a still where a hull crosses the boss, judged by depth); the frame still runs every Keel shader; one
   `keel_gallery` still per state under the `gpu` nextest group, byte-identical across two runs on one device.
5. **Budget.** Measure GPU time per boss with Keel's timestamp path (`audit-performance.mjs`, matched raster extent,
   load recorded with `--wait-quiet`). Prediction to score: 2–5 ms per boss at 1280x800 (`gpu_native.md`), against
   Keel's 16.7 ms frame. *Pass:* a written budget (for example ≤ 2 ms p95 per on-screen boss at the default quality)
   that the stage asserts; *fail:* the step stops and the CPU `Scene3D` path stays.
6. **Hot reload.** The preview watches the artefact directory, loads the new one under an error scope and keeps the
   old one on refusal; `set_param` edits apply live. *Pass:* an invalid artefact leaves rendering and input live
   (Keel's WGSL-reload test pattern); a valid one swaps within one frame of the load.
7. **Browser.** `host_web` draws the same boss from bytes fetched with the WASM bundle. *Pass:* the browser `objective`
   stage on the matched pair (`objective-lamp` scene), real GPU pixels, `contact-sheet.html` reviewed.
8. **Many instances.** Split the manifest's draws so the game schedules all Loft subjects inside its own scene pass.
   *Pass:* five bosses on screen cost less than five times one boss (GPU timestamps); no extra render pass per
   instance.
9. **Retire the CPU path per boss** only after 4–8 pass for that boss, as its own commit. The art crate's hit and
   telegraph geometry stays in the sim (`game_bships`), never in the artefact.

Keel's own design notes say the current weapons work does not need Loft, so steps 0–3 are Loft-side work. They can
start now, and nothing on Keel's side waits for them.

## Part 4. Keel architecture worth copying into a lean Rust Loft

- **Sum types at every seam, one handler per variant.** `Offer { Accepted, Full(Box<CoreInput>), Closed }`;
  `TickPace { Paced, Unpaced }` with the test-only variant behind `#[cfg(test)]`; `RoomTiming { World, Fixed }`;
  `NoSpell` as an uninhabited enum, so "this asset casts nothing" is a type, not an empty-list check. Speed-swarm S8
  removed all ten `_ =>` arms in game_client. Lint rule: no new catch-all arms on enums the workspace owns.
- **The crate graph enforces the layering, with a check for each rule.** The sim crates do not compile wgpu, which
  took them from 148 to 89 crates; art crates depend only on `showroom`; the server tree has no GPU; and a lab
  workspace with its own lockfile and target keeps experiments from re-linting the game (27 of 63 members had been
  lab crates). Each rule is a script that resolves the real `cargo tree`, not a substring search. For Loft: `loft-ir`
  (graph, no GPU, no floats in identity and hashing) → `loft-core` (CPU twin, proofs, WGSL emit) → `loft-loader` (the
  only wgpu crate) → `loft-view` (binary). A check proves `loft-core` never reaches wgpu, mirroring
  `check_loader_closure.ts` in the other direction.
- **Lints as policy.** `float_arithmetic = "deny"` at the workspace level, relaxed per crate for presentation. Loft's
  equivalent is to deny floats in the crates that compute hashes, names and artefact bytes, where byte equality is
  the contract (the proposal's naming lane). `unsafe_code = "deny"` with a single named exception (the pipeline
  cache).
- **Production constants with typed scaling for tests** (`Timing::PRODUCTION`, `scaled_down`): tests get fast
  deadlines without a second code path, and the production values are pinned by their own tests.
- **One protocol generation constant** (`game_room::protocol::PROTOCOL`), bumped with any incompatible serialized
  layout, with peers kept paired. Loft's `format_version` already plays this role; record it, with the compiler
  commit, in every artefact, and have the loader refuse a mismatch by name.
- **"The renderer consumes frame output, never writes simulation state."** This is the contract Loft needs from
  Keel, and Loft should state it in its own loader docs: inputs in, pixels out, nothing back.
- **Comments name the incident.** Every rule carries its date, its numbers and the alternative that was rejected,
  in the code next to it. That is why a later agent can delete a rule safely.
