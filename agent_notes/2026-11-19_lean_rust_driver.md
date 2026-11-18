# Lean Rust: the program driver (19 Nov 2026)

Plan: `docs/proposals/2026-11-19_lean_rust_loft.md` §1.9, §1.10, §3.5, §8.1, §8.2 (the integrator owns
`loft-core/src/lib.rs`, the addon's `drive`, `src/cli/main.ts` routing and `LOFT_ENGINE`). Branch `feat/lr-driver`
from `d9d208a`. Code commit `a343168`; every number below was taken on it (clean tree, M4, load1 12–19 on 10 CPUs,
so every timed run is marked `loaded`).

## What

- **`loft_core::run(job, limits, compiler)`** (lib.rs, about 190 lines with the module list). One exhaustive dispatch
  over `Program`:
  - **Scene:** decode → plan → emit → bundle.
    - Plan: one resource per `Kernel` decl (a table is `Rows { entries, lanes }`, author rows are a `Curve`), then
      `kernel::plan`, `schedule` and `layout`.
    - Emit: each distinct program root becomes `loft_root{k}(free variables in name order)`, appended as a graph
      function. All roots go into one module, validated by Naga as f32 (a refusal is the build's) and as f16 (reported
      as `mixed: Valid | Refused(msg)`; the host refuses a mixed load of such a module itself).
    - Bundle: `Bundle` V1 with resources, the module, params, inputs and sections; then its bytes and sha256.
    - The report carries counts, `unfilled` tables, `draws` and the wall time of each phase.
  - **Solve, Evaluate, Check:** `Internal::Unbuilt`, because the wire has no layout for their decls.
- **loft-node** `drive` calls it and writes the bundle to the caller's path. The ABI is now 3:
  `run(job, limits, out: Uint8Array)`. `build.rs` stamps the commit into `compiler`; outside git the stamp is the
  named value `not-a-git-checkout`.
- **loft-ir** `Shape::Rows.lanes` is now `u16`, because tables reach 632 lanes. The kernels test had been casting
  them to `u8`, which wrapped 632 to 120. Counts are unchanged.
- **CLI:** `LOFT_ENGINE=rust loft build X --out D` writes `D/scene.loft`. It prints what the bundle lacks.
  `LOFT_ENGINE=shadow` is a `usage` error for build. serve's compile child is pinned to `LOFT_ENGINE=ts`, because
  both executors still read the TS artefact directory. This was the 3 serve.rust failures.
- **Tools:**
  - `bun tools/parity/run.ts rust`: every corpus entry through the driver, compared with how R0's build ended. Output
    goes to `out/parity/rust/rust.json`.
  - `measure.ts` now scores the driver's measured phases. A driver refusal is recorded, and the scene then gets a
    lower bound instead of a projection.
  - `src/record/score.ts` records an import throw as `refused at import: <class>` plus its first line.
    heron_leap_refused gives `SolveError`, so the corpus is 125/125.

## Lane checks

| Lane check | Result |
|---|---|
| 2.1 const fills within 1 ulp (`--run-ignored all`) | **FAIL, unchanged**: 79/160 fills (822/17,900 values). The structural check passes: 4,020 tables, 10/10 Steps, 215 vs 446 dispatches. The gate contradicts §3.4 (f32 tape against f64-then-round TS), so the owner must re-score it. On real jobs it cannot run: a `Kernel` decl on the wire has no fill body (`unfilled` in every report) |
| 2.2 Naga on every corpus module | **PARTIAL.** 27 entries build and are Naga-valid as f32. 213 are refused. Each refusal comes from library raw WGSL that reads a global only the TS host declares: `loft_detail` 204 (surface.ts `grid_cells`, the LOD table), `view_dir` 2, `loft_cast` 2, `loft_passtex_*` 3, `loft_compute_*` 2, `soft_box` 1 |
| 2.2 `lf` alias both ways | 21/27 valid as f16. 6 refused, each for a literal beyond f16's 65,504 (`-1e9`), which emit writes as `lf(...)` |
| 2.2 three permutations give one bundle hash | **PASS** on owl, heron_leap, rope and beetle. The runs were source, `--perturb 7`, `--perturb 99` and a repeat: one hash each. owl's bytecode job hash also equals its source hash (`score.ts --compare`) |
| 2.2 shared modules ≤ TS chunks | **Not comparable**: a bundle has one module of roots and no draw modules |
| 2.3 same verdicts, heron | **Cannot run**: the wire refuses Check (tag 6) and Solve (tag 7) decls, so no Check or Solve job exists, and the driver answers `Unbuilt`. Solve is also being ported on `feat/lr-solve`. The 25 bridged checks are still 25/25 |
| 3.1 / 3.2 / 3.3 parity against R0 | **Cannot run**: 0 of 295 bundles have a draw. The recorder writes no Draw, Pass, Medium or Camera decls; §3.2 pins no layout for them; and 3.1's TS half (library hit, place and paint functions) is not written. `loft-draw` still reads TS artefacts through loft-loader |
| 4.2 §8.1 scored | **FAIL** on all three scenes (below) |
| Corpus ends like R0 | 81/295 match: 27 built, 52 library files refused as `load`, the heron_leap_refused solve and serpent_clip's TypeError. The other 214 differ: 211 are R0 builds that the Naga refusals above turn away, and 3 are R0's CPU-table refusals (open, storm, stormfront) that are now refused by `loft_detail` |

Also: `binding_check.ts` passes under Bun 1.3.10 and Node 24.3.0 (crossing 0.57–0.62 ms on a 1M-node job), and
`loft-core` passes 17/17 tests (1 ignored, red as above).

## §8.1 (`bun tools/parity/measure.ts`, 3 alternating runs, `out/parity/measure/a343168.json`)

Each value is a median in ms of CPU. "Rust" is decode, plan, emit (with Naga f32), f16 and bundle.

| | start | link | job | Rust | rest (priced) | projected | gate | today's process, RSS |
|---|---:|---:|---:|---:|---:|---:|---:|---|
| Owl | 61 | 60 | 29 | 58 (emit 33, f16 24) | 4.7 | **219, FAIL** | 80 | 336 ms, 161 MiB (gate 101) |
| Titan | 63 | 394 | 383 | refused (`loft_detail`) | 22 | **≥ 870, FAIL** (lower bound) | 300 | 1,908 ms, 534 MiB (gate 187) |
| heron | 64 | 43 | 29 | 143 (emit 83, f16 57) | 7.1 | **2,258, FAIL** (design 1,958 is today's TS solve) | 200 | 2,478 ms, 1,144 MiB (gate 136) |

- Counts are unchanged from lane 4.2. Titan U is 1.26× the model and R 1.16× (a COUNT MISS), still under the 0.75M
  kill condition.
- The TS compile (owl 123, Titan 1,020, heron 203) is outside the projection: the cutover removes it.
- What fails before any Rust phase:
  - **Owl** fails on the TS side alone (start, link and job: 150 > 80).
  - **Titan** fails the same way (840 > 300).
  - **heron** fails because its solve still runs in TS at import.
- **New:** Naga validation is unpriced in §8.1 and is most of emit. For owl that is 33 + 24 ms against the model's
  emit + share of 3.8 ms. The bundle is 341 KB of WGSL for 5,906 nodes. Decode alone is 0.1 ms (model 1.1), but the
  driver does no interning or inlining yet.

## What blocks the cutover

1. **Draws.** The wire has no Draw, Pass, Medium or Camera layout, and the TS library is not yet rewritten as
   hit, place and paint functions (lane 3.1's TS half). Without both, no bundle can be drawn and no parity check can
   run.
2. **Fills.** `Kernel` decls carry no fill body, so tables are zero (lane 2.1 on real jobs).
3. **TS-host globals in library raw WGSL.** `loft_detail` alone refuses 204 entries, Titan among them. The LOD plan
   (lane 3.2 "left") and the pass-texture and compute names need Rust bindings or a library rewrite.
4. **Check and Solve decls** have no layout, so the wave-2 analyses cutover cannot route to Rust. Solve is in flight.
5. **loft-draw** is still on loft-loader, not loft-gpu.
6. **§8.1** fails on start (source library), link and job write before any Rust phase.
