# Lean Rust: validate once, f16 on request, lane 2.1 re-scored (19 Nov 2026)

Plan: `docs/proposals/2026-11-19_lean_rust_loft.md` (§8.1, §8.2 lane 2.1). Before: `agent_notes/2026-11-19_lean_rust_driver.md`
§8.1 table (a343168). Branch `feat/lr-speed` from `2726371`. Code commit `628ae5e`, and every number below was taken on it
(M4, load1 15–23 on 10 CPUs, so every run is `loaded`).

## The theory, as stated before the run

1. Naga validated each module twice: once in emit (Owl: 33 ms of emit, plus 24 ms on the f16 variant), and again when
   the host created the module. Make emit's Naga a lab/check step only, and Owl's Rust phase drops from 58 ms to about 5 ms.
2. The f16 check buys nothing unless mixed was asked for. Run it only under `--precision mixed`: −24 ms on Owl, −57 ms on heron.

## What changed

- **`loft_core::run(job, limits, Build { naga, precision }, compiler)`**
  - `Naga::Load` is the default build. Emit runs no Naga, neither the per-raw parse nor the module validation, so the
    host's module creation is the one validation. That is loft-gpu `validate` and then wgpu: loft-gpu still runs its
    own Naga at load before any GPU object exists, which is Keel's hot-reload step and is not this lane's to drop.
  - `Naga::Emit` runs `emit::check`: each raw function alone, then the module as f32, refusing with
    `Scene { Wgsl }` at the author's line. `loft build --naga` (LOFT_ENGINE=rust), `tools/parity/run.ts rust` (with
    `--naga --precision mixed`, so its corpus numbers stay comparable with the driver note's) and draw modules
    (`draw::Lowered::module`) ask for it.
  - `emit::module` no longer validates.
  - `Precision::Mixed` adds the f16 check. Under F32, `Scene.mixed` is `NotAsked` and the `f16` phase is 0.
  - **Cost of the default:** a bad module under `Load` is refused at load, without the author's line. `loft check`
    has no Rust Scene path yet (Check jobs are `Unbuilt`). At the cutover it should build with `Naga::Emit`.
- **loft-node ABI 4:** `run(job, limits, out, build)`. `build` is 2 bytes, `[naga, precision]`, classified once in
  `build_of`, and any other byte is a `Usage` refusal. `src/core/native.ts` takes `Build` (default
  `{ naga: "load", precision: "f32" }`).
- **Lane 2.1 check (4):** "within 1 ulp" is replaced by `Tol::f32_rel(1e-4)`, a typed constructor in
  `loft-core/tests/it/main.rs`. A fill holds when |got − want| ≤ 1e-4 · max(|want|, 1). The constructor is a
  `const fn` that refuses rel < 1e-4 at compile time. Picture parity is the check's other half and needs bundles that
  draw (lane 3.1). The test is un-ignored as `const_fills_within_f32_tol_of_ts`, and the plan's 2.1 row says so.
  `analyses.rs` still writes its margin tolerance by hand (`1e-4 * |m| + f32::EPSILON`) and can move onto `Tol`.

## Scores (`bun tools/parity/measure.ts`, 3 alternating runs, `out/parity/measure/628ae5e.json`)

Rust = decode + plan + emit + f16 + bundle, median ms.

| | before (a343168) | after (628ae5e) | emit after | predicted |
|---|---:|---:|---:|---:|
| Owl | 58 (emit 33, f16 24) | **10.5** | 9.2 | ~5 |
| heron | 143 (emit 83, f16 57) | **27.0** | 23.2 | ≤ 86 (f16 −57 stated; emit share not stated) |
| Titan | refused (`loft_detail`) | **142.8** (builds now) | 129.9 | — |

- **Partly missed.** Removing both Naga passes took off what was predicted: 47.5 ms on Owl and 116 ms on heron. The
  miss is in the remainder. Emission without Naga is 9.2 ms on Owl, against the §8.1 model's 1.8 ms
  (2 statements/U at 55 ns), so 5 ms assumed emit was already near the model. Emit's text building is the next lever:
  Owl writes 341 KB of WGSL for 5,906 nodes, and the driver does no interning or inlining yet.
- **Titan builds now.** The `loft_detail` refusal was Naga's at emit. Under `Load` it moves to load time, where the
  host refuses it. Its 130 ms of emit is the first Rust number for Titan. 211 corpus entries behave the same way, and
  `run.ts rust` keeps `--naga`, so the corpus comparison is unchanged.
- §8.1 still fails on all three scenes, and before any Rust phase: start + link + job is already 178 ms for Owl
  against a gate of 80.

## Lane 2.1

`tools/dev.sh test loft-core --release -E 'test(const_fills)'`: **PASS**. 0 of 160 compared fills (17,900 values) are
outside `Tol::f32_rel(1e-4)`, and the worst is 9.24e-6 of max(1, |want|). The structural check is unchanged: 4,020
tables, 10 Steps, and 215 Rust frame dispatches against 446 for TS.

## loft-draw and loft-loader: not moved, and why

loft-draw cannot move onto loft-gpu's Bundle V1 loader yet without breaking every picture path on main:

- **Nothing writes a drawable Bundle V1.** The TS build writes artefact directories, and the Rust driver's bundles
  have 0 draws (the wire has no Draw, Pass, Medium or Camera layout).
- **The serve protocol's camera has no meaning in a bundle.** A serve request carries an explicit camera
  (eye/target/fovY). A bundle has only `cameras: (name, root)`, and loft-gpu reads no camera: a host camera is
  "inputs", and their names are not pinned.
- **loft-gpu lacks what `loft-draw profile` uses:** `Bounds::Latest`, `FrameStats` and the pose-time readings.

A Bundle variant now would only draw empty frames. It moves when lanes 3.1 and 3.2 pin draws and cameras.

What still depends on loft-loader:

- **Code:**
  - `rust/crates/loft-draw` uses `Loft`, `read_dir`, `Bounds`, `Camera`, `Precision`, `PipelineCache`, `Timestamps`,
    `FrameStats`, `OUTPUT_FORMAT` and `Error` across main, still, serve and profile.
  - Through loft-draw: `src/cli/serve.ts` (the draw child of rust serve, render, frames and clip), `src/cli/main.ts`
    `profileOnDraw`, and `tools/parity/run.ts` (the R0 reference pictures). `run.ts` also lists
    `rust/crates/loft-loader` in `PICTURE` and in the loftDraw source hash.
  - loft-loader's own `loft-view` binary, used by `tools/rust_compare.ts`.
- **Mentions only:**
  - `tools/check.sh` (a crate list and a comment)
  - `tools/lib/gate_flags.sh` (a comment)
  - the `rust/Cargo.toml` comment
  - `loft-gpu/src/cache.rs` (a provenance comment)
  - `src/cli/serve.ts:196` (a comment)

## Checks

- **loft-core:** focused emit, kernels and draws tests pass. In `check.sh quick`, 18/18 nextest pass.
- **binding_check.ts** (probe build, Bun): no fails.
- **check.sh quick:** static all ok. The Bun subset passes, except that the 3 `serve.rust` tests failed in a fresh
  lane because `rust/target/lab/loft-draw` had not been built; after `tools/dev.sh build loft-draw --fast` they pass
  3/3.
