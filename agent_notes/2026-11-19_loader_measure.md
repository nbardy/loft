# The old Rust loader (loft-view) on TS artefacts: frame, load, modules, pipelines (2026-11-19)

Question: how fast is `rust/crates/loft-loader` (`loft-view`) when it draws artefact folders the TS engine compiled,
against the TS engine on Dawn and the new path (loft-core bundle on loft-draw / loft-gpu)?

Machine: Apple M4, Metal, load average 3.4 to 4.7 during the runs. Base: main at 5d94531, branch
`scratch/loader-measure`. One run per scene and path; numbers are single measurements, not distributions.

## Method

- Artefacts: `bun src/cli/main.ts build <scene> --out out/loader_measure/<name>` (the TS engine's `buildTs`, which
  runs `lower` and `src/host/share.ts`).
- loft-view: `tools/dev.sh build loft-loader --fast` (profile `lab`), then
  `rust/target/lab/loft-view <dir> --view <R0 first camera> --time 1.37 --size 960x540 --frames 16`. This matches
  tools/parity's `loft-draw profile` call: no `--wait`, so frames plan from the last bounds read, as a game would. GPU
  frame = pose pass + render passes from timestamps. Load = read + pipelines + resources.
- Instrumentation added: `loft-view`'s quantile line now prints p95 using the same nearest-rank rule as loft-draw's
  `quantiles`. Module and pipeline counts are the manifest's (`modules`, `renderPipelines`, `computePipelines`); the
  loader creates exactly those.
- R0's first cameras (out/parity/R0/entries): lineup `lineup`, Titan `game`, Owl `game`, heron `side`.
- TS on Dawn: `bun src/cli/main.ts profile <scene> --view <cam> --time 1.37 --size 960x540 --frames 16`. Its "normal
  frame" is the **least** of 16, not p50/p95. It does not report load on its own, so the column gives whole-process wall
  time, which includes the TS compile (5.8 s of the lineup's).

## Results

| Scene | Path | GPU p50 / p95 ms | Load | Modules | Pipelines (render / compute) |
|---|---|---:|---:|---:|---:|
| five-crab lineup | **loft-view** | **3.01 / 3.55** | 32.9 s cold, 3.6 s warm | 305 | 422 / 24 |
| | TS on Dawn | 4.33 (least) | 36.3 s wall | 305 | (same artefact) |
| | loft-draw (task brief) | 97.2 / 109.1 | 23 s | 5,538 | 653 / 4,905 |
| | loft-draw, rerun here at 5d94531 | 30.9 / 32.1 | 96.4 s | 5,539 | 654 / 4,905 |
| Titan (reefback) | **loft-view** | **1.16 / 4.68** | 4.73 s cold, 0.23 s warm | 78 | 89 / 19 |
| | TS on Dawn | 4.13 (least) | 1.46 s wall | 78 | |
| | loft-draw (draws note) | 6.79 / 7.20 | 35.1 s | 990 | 116 / 889 |
| Owl | **loft-view** | **0.43 / 0.43** | 0.74 s | 16 | 17 / 6 |
| | TS on Dawn | 0.39 (least) | 0.92 s wall | 16 | |
| | loft-draw (draws note) | 2.69 / 6.01 | 0.31 s | 91 | 19 / 73 |
| heron | **loft-view** | **0.45 / 0.50** | 0.94 s | 17 | 19 / 6 |
| | TS on Dawn | 0.33 (least) | 2.09 s wall | 17 | |
| | loft-draw (draws note) | 1.69 / 8.08 | 0.54 s | 159 | 27 / 133 |

The loft-draw rows other than the rerun come from agent_notes/2026-11-19_lean_rust_draws.md ("Costs").

Notes on the numbers:

- **Load is dominated by the Metal compiler and its OS cache.** The first loft-view load of each scene was cold, at
  about 5 s of user CPU against 33 s of wall time for the lineup, so the time is spent in the system's Metal compiler.
  A second process's `--loads 2` measured 3.59 s and 3.69 s for the lineup and 0.227 s and 0.222 s for Titan. Load
  figures from different paths are comparable only at the same cache state. The 23 s and 54.9 s figures do not record
  their cache state.
- Titan's p95 of 4.68 ms comes from the first two frames: the minimum is 1.11 ms and the median 1.16 ms. Its first
  frames use the bounds read from the picture frame.
- On the lineup, loft-draw split its 30.9 ms frame as a 13.7 ms kernel pass plus 17.3 ms of render passes. loft-view
  split its 3.01 ms as a 0.83 ms pose pass plus 2.17 ms of render passes. The rerun's 30.9 ms is not the task brief's
  97.2 ms; the bundle was rebuilt at 5d94531, and this note does not establish why the two differ.

## Why the old loader is fast

The GPU frame time belongs to the artefact, not to the executor. loft-view runs the same lowered artefact Dawn runs,
and lands within 30% of Dawn on every scene; on the lineup it is 3.0 ms against 4.3 ms. loft-draw's bundle is shaped
differently in two ways:

1. **Pose tables are batched by dependency level.** `src/host/lower.ts` merges every table at one dependency level
   into one compute pipeline ("pose level 0: loft_frame_0, crab0__titan_routine_aimL, ..."). The lineup's 1,188 map and
   232 scan tables become **24 compute pipelines and 24 dispatches** in one pass, which costs 0.83 ms.
   loft-gpu's `dispatch_all` (rust/crates/loft-gpu/src/lib.rs:953) runs one pipeline and one dispatch per kernel,
   **4,884 kernels**, which costs 13.7 ms. This 16x gap in the kernel pass comes from per-dispatch overhead and
   barriers, not arithmetic.
2. **Draw shaders are shared and level of detail is live.** `src/host/share.ts` merges draw shaders that differ only in
   their literals: 658 draws use 305 modules and 422 render pipelines. The loader's bounds readback then picks coarser
   grids, giving 398k triangles at this view. loft-draw builds one pipeline per draw (654), and `loft_detail` is "zero
   until the level-of-detail plan is ported" (loft-gpu lib.rs:217). Its render passes take 17.3 ms against 2.2 ms. The
   level-of-detail part of this is **CONJECTURE**: loft-draw's triangle count was not measured.

Load follows from the same two choices: 305 modules against 5,539, and 446 pipelines against 5,559. With a warm Metal
cache, loft-view loads the lineup in 3.6 s.

## Consequence for the plan

To make loft-draw fast, the next lane should give the loft-core bundle the TS lowering's shape. That means one kernel
per dependency level, with tables fused within a level, plus shared draw templates and the level-of-detail plan. A
faster executor would not close the gap. loft-loader is evidence that wgpu on Metal can execute this shape at or below
Dawn's cost.
