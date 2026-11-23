# perf-load · cold and warm load

Base `34bd927` (main). Apple M4 Metal, 1280x800. All timings through `loft load SCENE --set count=N --nonce N`
(extended in this lane to print the Load gate's cold/warm pair, front-end stage laps and each pipeline's creation time).
The machine was shared with other lanes: `uptime` load average was 2.5 to 22 during measurement, and the same binary
varied 1.0 to 2.0 s cold on Hermit between runs. Compare only adjacent A/B pairs.

## Where the cold load goes (Hermit-1, main)

| Part | Quiet (load ~3-7) | Loaded (load 14-20) |
| --- | ---: | ---: |
| parse, check, schedule, scans, emit, naga | 0.02 s | 0.03 s |
| interval derivation (`derive`, 11 shapes in 5 kinds) | 0.21 s | 0.46 s |
| WGSL module creation + validation (`modules_ms`) | 0.01 s | 0.03 s |
| 14 Metal pipelines, created concurrently | 0.8-0.9 s | 1.7-3.0 s |

* Parse/check/emit and Naga are not the problem (about 30 ms together). "Double Naga validation" is worth at most
  10-35 ms (`modules_ms`); not pursued.
* Pipelines are already created concurrently (`create_all`, one thread per recipe). The cost per pipeline in the
  parallel run (1.6-3.0 s each, even the trivial `cull`) is queueing in Metal's compiler service, not shader size.
  Serialising them (one at a time) summed to 4.4-11.5 s; four at a time gave the same wall time as all 14 at once
  (5.0 vs 4.1-5.4 s under load). Throughput saturates near four jobs, so the cold pipeline stage is
  total compile work divided by about four, and more concurrency buys nothing.
* Serial per-pipeline cost (load ~10): the five `shade` kernels 5.3 s (57 %), five `vs fs` 2.8 s (30 %),
  cull/classify/present 1.1 s, bake 0.24 s. Even `cull` costs 0.29 s alone, so each pipeline has a fixed
  Metal cost near 0.3 s.
* Relaxing wgpu runtime checks (`force_loop_bounding`, `bounds_checks`, `int_div_checks`, all) did not move compile time
  beyond run-to-run noise (4.5 to 11.5 s for identical serial runs). Not adopted; it would also remove safety.
* The slowest-pipeline figures of 3.4-4.1 s in OPEN.md were queueing under a loaded machine; alone Hermit's biggest
  pipeline is 0.8-1.8 s.
* No Metal pipeline cache exists in wgpu 30 (`PIPELINE_CACHE` is Vulkan only; see `ray_query.rs`). Metal relies on the
  OS shader cache, which is why a repeated source loads in 0.03 s (warm). The nonce in each cold sample changes the
  source on purpose.

## Predictions, made before the change

1. Port c1a: Hermit 14 to 13 pipelines, one fewer compute dispatch at load. The bake pipeline costs 0.24 s of 9.3 s
   serial work, so about -3 % of the cold pipeline stage (-0.03 to -0.1 s). Frames byte-identical.
2. Derive all (kind, shape) jobs from every kind in one dynamically-claimed pool instead of one chunked pool per kind.
   Hermit has five kinds with 1-3 shapes each, so the per-kind pools used 1-3 of 10 cores. Predict `derive` 0.21 to
   about 0.08 s quiet (the longest single derivation), 0.46 to about 0.2 s loaded. Blobs-20 and Capsules-20 already
   fill the cores; predict no change.
3. Cold load for Hermit therefore falls by about 0.15-0.25 s. It will not reach 1.5 s on a loaded machine because the
   pipeline stage alone is 1.7 s there.

## Change

* `crates/loft-emit/src/emit/surface.rs`, `emit.rs`, `crates/loft-passes/src/bundle.rs`, `crates/loft-native/src/{lib,shadow}.rs`,
  `crates/loft-cli/examples/phase4_gate.rs`: reimplemented c1a (f9bd52c) on main's bundle ABI. A parametric surface
  whose authored point does not read its row (the existing eligibility proof) evaluates `q_surface(row, uv)` in the
  new `vs_grid` entry. The `bake_surface` kernel, its transient points table, the (2,5)/(2,6) bindings, the load
  initializer and a wave are removed. The index buffer and indirect-args layout (five words) are unchanged. Archived
  modules that only have `vs_indexed` take the plain `vs` path, as c1a did.
* `crates/loft-passes/src/pipeline/stage0.rs`: `derive_kind` split into `plan_kind` (sequential: shape keys, step
  scale, cache domain, mesh bounds), `derive_all` (one pool of all (kind, shape) jobs, atomic claiming, one `Eval` per
  worker) and the unchanged `pack_kind`. Diagnostics order: plan-time errors of all kinds now precede derive-time errors
  of any kind; within the derive stage the first failing job in kind, shape order reports.
* `crates/loft-cli/src/main.rs`: `loft load` prints the cold/warm pair with a fresh derivation cache, the front-end
  stage laps and each pipeline's creation time (this was the missing instrument; `pipeline_ms` was only in gate.json).
  Not an owned file; instrument only.

## Measured (see commands below)

Frames: stills at t = 0, 4, 9 s of blobs-1/20, capsules-1/20, hermit-1/20 are byte-identical PNGs, base vs lane
(18/18). `coral_shrine` differs from itself between two runs of the unchanged base binary (41/137/149 pixels above 2/255)
and the lane differs from base by 73/125/84, inside that noise; it has no surface kind so it takes none of the changed code.

Cold/warm A/B (Hermit, adjacent pairs, `loft_base2` is main with the new `loft load`; machine load 6-8, rising):

| Scene | main cold s | lane cold s | main warm s | lane warm s | main front s | lane front s |
| --- | --- | --- | --- | --- | --- | --- |
| hermit-1 (3 pairs) | 1.13 / 1.96 / 1.89 | 0.92 / 1.82 / 1.79 | .035 / .064 / .058 | .034 / .057 / .063 | .23 / .46 / .42 | .10 / .19 / .18 |
| hermit-20 | 1.04 / 2.05 / 1.98 | 1.17 / 1.72 / 1.93 | .035 / .067 / .076 | .052 / .065 / .065 | .23 / .40 / .39 | .10 / .19 / .21 |

Pipelines: Hermit 14 to 13 at both counts.

Blobs-20 and Capsules-20 derive (5 pairs, load 5-11): blobs-20 524 ms mean lane vs 682 ms main, capsules-20 308 vs 531 ms
(both dominated by noise; no regression, as predicted).

## Verdict

* Prediction 1 hit (14 to 13, frames identical). Prediction 2 hit on the quiet pair (derive 210 to 82 ms, front 0.23 to
  0.10 s) and on the loaded pairs (0.45 to 0.19 s). Scenes with one kind unchanged within noise.
* Prediction 3 hit: Hermit cold falls 0.1-0.25 s per pair, but the pipeline stage (0.8 s quiet, 1.7 s loaded) now sets the
  cold load. On a quiet machine Hermit cold is about 0.9-1.2 s (under the 1.5 s cap); on the loaded machine it is 1.7-1.9 s.
  Blobs-20 (cap 0.5 s) and Capsules-20 (cap 0.5 s) miss their caps by derivation (0.2-0.5 s for 20 and 140 shapes)
  plus a 0.5-0.9 s Metal floor for five pipelines; the five-pipeline Metal floor alone is above the cap on this machine.
* Not done: overlapping derivation with pipeline creation needs a two-phase `compile` (modules and schedule first, row
  data after). Pipelines depend only on modules and layouts, so about 0.1-0.2 s of front end could hide under compile.
  Deriving inside a single shape (interval subdivision across threads) would be needed for blobs-20.
* Gate verdicts for Load (cold caps) cannot be called reliably unless the machine is idle; measure with `tools/quiet_gpu.sh`
  and no other lane running Cargo.
