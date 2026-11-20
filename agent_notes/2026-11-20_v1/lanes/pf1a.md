# PF1a · Retain the sun map

## Prediction before implementation

The first submitted frame records the original Shadow pass before Shade. Once the caster render stages are proved independent of per-frame time and sample count, a bit-identical sun uniform and unchanged caster resources permit omitting the entire pass. Changing the sun projection records one full redraw before its next reader and the next unchanged frame reuses it. A caster whose shader or mutable tape cannot be proved invariant continues to draw every frame. Camera row order and orbit can vary independently of the sun's identity caster list.

At 1280 × 800 on the M4, after 30 warmups, I predict 17 → 13 Hermit-20 commands over 300 orbit frames and 12 → 9 close Shrine commands over 100 fixed t=0 frames. PF1’s A/B implies about 0.56/0.46 ms median savings against 0.45/0.35 ms targets. The first frame still pays one full shadow pass, expected roughly 0.65/0.46 ms of attributed GPU time. Candidate p95 targets are 5.60/4.00 ms. PF1’s respective p95 savings were +0.428/−0.062 ms, so a Shrine tail improvement is uncertain. Use one Metal device and binary with natural-order ABBA timing, a triangle control in each arm, and absolute p95 as a separate verdict.

I expect zero decoded beauty, camera depth and ID differences at 0, 4 and 9 seconds, with Shrine comparing identical ascending and descending camera orders independently. Shadow depth should match the always-draw control both before and after invalidation, differ between the two light projections, and match the redrawn map on immediate reuse. The pass should have no timestamp span or GPU commands when skipped.

## Dependency proof

The native Shadow owner analyzes Naga IR reachable from the vertex entry selected for the draw and the same fragment entry the bundle uses. It accepts reads of the load-owned caster tables, identity lists, LUTs and indexed grid. Membership, geometry and pose, bounds, index data and texture allocation are fixed for that load. The sun projection, inverse, direction and other invariant uniform bytes are compared by bits. Changes to the live parameter buffer invalidate the map if any caster stage reads it. Time and the sample-count word are excluded from the comparison only when their absence from caster reads is proved. Unknown resources and whole value reads that could include those words select every-frame recording.

Editable caster modules select every-frame recording from the start. A publication can change tape or row data, bounds and projection, or the promoted shader. A publication for a separate noncaster module leaves the original caster resources valid. The submission mutex spans frame writes, the cache decision and queue submission. The first draw records in its original place before Shade. Later draws retain the same pipelines, caster and index ordering, depth store and receiver sampling. Timestamp queries are packed for the executed passes and expanded to source pass order with a zero Shadow pair on reuse.

## Speed

Apple M4 Metal on macOS 15.5, 1 ns timestamp period, 1280 × 800 and one sample. Each arm had 30 warmup frames, then 300 natural-order Hermit-20 orbit frames at 60 Hz or 100 natural-order close Shrine frames at fixed t=0. A forces the native main schedule to record Shadow every frame. B uses the production reuse decision. Both scenes were resident on the same device and queue and built from the same program in the same lab binary. The baseline was `5527cba`. Main advanced to `bca209d` during the lane with documentation changes only.

Frame p50 and p95 use the gate's rounded rank. The gain is the mean of the two order-specific A minus B differences. Pass quantiles use consecutive pass ends as in the gate. Raw start-to-end spans are in the score files and are larger where passes overlap. Present ended last in all timed frames. All 24 triangle controls were below the gate's 0.350 ms contention threshold. The desktop AGX utilization sampled immediately before the runs was 39%, 42% and 39%. Both build slots and the GPU lock were held during timing.

Both median savings targets held in all runs. Hermit held the 5.600 ms p95 cap in all six candidate arms. Shrine exceeded 4.000 ms in both first-run arms and held it in all four retry arms. The first run captured Hermit and ascending Shrine before the harness's second cull rewrite failed. The second run used the original cull for each capture order. The third run confirmed timing with the same binary as the second. Every timed candidate Shadow slot had a zero pair and issued zero draws. The two exact Hermit command budgets in the gate manifest move from 17 to 13.

### Run1

**Hermit-20** · ABBA · main minus candidate p50/p95 0.871 / 0.860 ms.

| GPU ms unless specified | A1 | B2 | B3 | A4 |
| --- | ---: | ---: | ---: | ---: |
| Triangle control | 0.096 | 0.031 | 0.028 | 0.028 |
| Draws and dispatches | 17 | 13 | 13 | 17 |
| Whole frame p50 / p95 | 4.438 / 5.940 | 3.758 / 5.269 | 3.940 / 5.372 | 5.002 / 6.421 |
| Cull p50 / p95 | 0.004 / 0.010 | 0.004 / 0.005 | 0.004 / 0.010 | 0.004 / 0.011 |
| Vis p50 / p95 | 1.408 / 1.679 | 1.395 / 1.659 | 1.413 / 1.726 | 1.489 / 2.255 |
| Shadow p50 / p95 | 0.641 / 1.120 | 0.000 / 0.000 | 0.000 / 0.000 | 0.688 / 1.424 |
| Classify p50 / p95 | 0.002 / 0.292 | 0.057 / 0.272 | 0.056 / 1.083 | 0.000 / 1.123 |
| Shade p50 / p95 | 2.085 / 3.398 | 2.066 / 3.412 | 2.147 / 3.536 | 2.366 / 3.758 |
| Present p50 / p95 | 0.153 / 0.514 | 0.147 / 0.425 | 0.151 / 0.546 | 0.159 / 0.554 |

The first candidate frame recorded 17 commands in 5.481 ms. The Shadow timestamp span was 1.903 ms and the attributed cost was 1.006 ms.

**Close Shrine** · ABBA · main minus candidate p50/p95 0.413 / 0.479 ms.

| GPU ms unless specified | A1 | B2 | B3 | A4 |
| --- | ---: | ---: | ---: | ---: |
| Triangle control | 0.028 | 0.028 | 0.030 | 0.028 |
| Draws and dispatches | 12 | 9 | 9 | 12 |
| Whole frame p50 / p95 | 3.369 / 5.110 | 2.914 / 4.690 | 2.917 / 4.462 | 3.288 / 4.999 |
| Cull p50 / p95 | 0.009 / 0.014 | 0.009 / 0.014 | 0.009 / 0.013 | 0.009 / 0.010 |
| Vis p50 / p95 | 2.168 / 2.923 | 2.294 / 2.867 | 2.271 / 3.100 | 2.245 / 3.060 |
| Shadow p50 / p95 | 0.475 / 1.483 | 0.000 / 0.000 | 0.000 / 0.000 | 0.479 / 0.724 |
| Classify p50 / p95 | 0.000 / 1.081 | 0.053 / 1.534 | 0.053 / 1.568 | 0.000 / 0.520 |
| Shade p50 / p95 | 0.387 / 1.526 | 0.285 / 0.465 | 0.286 / 0.505 | 0.377 / 1.163 |
| Present p50 / p95 | 0.178 / 0.459 | 0.180 / 0.654 | 0.182 / 0.510 | 0.178 / 0.244 |

The first candidate frame recorded 12 commands in 4.782 ms. The Shadow timestamp span was 1.941 ms and the attributed cost was 0.421 ms.

[Hermit ticks and commands](../../../out/meta/pf1a/run1/hermit-timing.json) · [Shrine ticks and commands](../../../out/meta/pf1a/run1/coral_shrine-timing.json) · [Raw pass spans and pair deltas](../../../out/meta/pf1a/run1/score.json) · [Host and triangle log](../../../out/meta/pf1a/run1.log)

### Run2

**Hermit-20** · BAAB · main minus candidate p50/p95 0.600 / 0.650 ms.

| GPU ms unless specified | B1 | A2 | A3 | B4 |
| --- | ---: | ---: | ---: | ---: |
| Triangle control | 0.099 | 0.029 | 0.032 | 0.029 |
| Draws and dispatches | 13 | 17 | 17 | 13 |
| Whole frame p50 / p95 | 3.697 / 4.678 | 4.291 / 5.340 | 4.295 / 5.321 | 3.690 / 4.684 |
| Cull p50 / p95 | 0.004 / 0.007 | 0.004 / 0.006 | 0.004 / 0.008 | 0.004 / 0.005 |
| Vis p50 / p95 | 1.381 / 1.622 | 1.398 / 1.639 | 1.402 / 1.592 | 1.383 / 1.604 |
| Shadow p50 / p95 | 0.000 / 0.000 | 0.634 / 0.999 | 0.637 / 0.888 | 0.000 / 0.000 |
| Classify p50 / p95 | 0.057 / 0.336 | 0.001 / 0.567 | 0.001 / 0.620 | 0.057 / 0.287 |
| Shade p50 / p95 | 2.065 / 2.804 | 2.076 / 2.940 | 2.077 / 2.920 | 2.067 / 2.843 |
| Present p50 / p95 | 0.148 / 0.429 | 0.147 / 0.338 | 0.147 / 0.400 | 0.146 / 0.479 |

The first candidate frame recorded 17 commands in 22.559 ms. The Shadow timestamp span was 9.489 ms and the attributed cost was 3.561 ms.

**Close Shrine** · BAAB · main minus candidate p50/p95 0.423 / 0.485 ms.

| GPU ms unless specified | B1 | A2 | A3 | B4 |
| --- | ---: | ---: | ---: | ---: |
| Triangle control | 0.030 | 0.029 | 0.031 | 0.030 |
| Draws and dispatches | 9 | 12 | 12 | 9 |
| Whole frame p50 / p95 | 2.786 / 3.468 | 3.134 / 3.869 | 3.246 / 4.238 | 2.748 / 3.669 |
| Cull p50 / p95 | 0.008 / 0.011 | 0.008 / 0.011 | 0.009 / 0.010 | 0.008 / 0.010 |
| Vis p50 / p95 | 2.133 / 2.629 | 2.113 / 2.504 | 2.198 / 2.862 | 2.173 / 2.555 |
| Shadow p50 / p95 | 0.000 / 0.000 | 0.479 / 0.799 | 0.473 / 0.762 | 0.000 / 0.000 |
| Classify p50 / p95 | 0.051 / 0.306 | 0.000 / 0.541 | 0.000 / 0.653 | 0.051 / 0.708 |
| Shade p50 / p95 | 0.267 / 0.752 | 0.415 / 0.848 | 0.386 / 0.795 | 0.269 / 0.620 |
| Present p50 / p95 | 0.172 / 0.587 | 0.172 / 0.285 | 0.175 / 0.340 | 0.175 / 0.260 |

The first candidate frame recorded 12 commands in 4.065 ms. The Shadow timestamp span was 1.613 ms and the attributed cost was 0.487 ms.

[Hermit ticks and commands](../../../out/meta/pf1a/run2/hermit-timing.json) · [Shrine ticks and commands](../../../out/meta/pf1a/run2/coral_shrine-timing.json) · [Raw pass spans and pair deltas](../../../out/meta/pf1a/run2/score.json) · [Host and triangle log](../../../out/meta/pf1a/run2.log)

### Run3

**Hermit-20** · ABBA · main minus candidate p50/p95 0.709 / 0.839 ms.

| GPU ms unless specified | A1 | B2 | B3 | A4 |
| --- | ---: | ---: | ---: | ---: |
| Triangle control | 0.100 | 0.028 | 0.031 | 0.030 |
| Draws and dispatches | 17 | 13 | 13 | 17 |
| Whole frame p50 / p95 | 4.444 / 5.424 | 3.747 / 4.346 | 3.755 / 4.425 | 4.476 / 5.024 |
| Cull p50 / p95 | 0.004 / 0.010 | 0.004 / 0.005 | 0.004 / 0.008 | 0.004 / 0.005 |
| Vis p50 / p95 | 1.407 / 1.672 | 1.385 / 1.589 | 1.388 / 1.598 | 1.396 / 1.647 |
| Shadow p50 / p95 | 0.638 / 1.183 | 0.000 / 0.000 | 0.000 / 0.000 | 0.635 / 0.948 |
| Classify p50 / p95 | 0.000 / 0.291 | 0.057 / 0.319 | 0.056 / 0.362 | 0.000 / 0.498 |
| Shade p50 / p95 | 2.089 / 3.214 | 2.074 / 2.732 | 2.073 / 2.716 | 2.081 / 2.810 |
| Present p50 / p95 | 0.148 / 0.438 | 0.147 / 0.458 | 0.147 / 0.497 | 0.146 / 0.371 |

The first candidate frame recorded 17 commands in 4.636 ms. The Shadow timestamp span was 1.740 ms and the attributed cost was 0.641 ms.

**Close Shrine** · ABBA · main minus candidate p50/p95 0.442 / 0.525 ms.

| GPU ms unless specified | A1 | B2 | B3 | A4 |
| --- | ---: | ---: | ---: | ---: |
| Triangle control | 0.029 | 0.031 | 0.031 | 0.028 |
| Draws and dispatches | 12 | 9 | 9 | 12 |
| Whole frame p50 / p95 | 3.047 / 3.673 | 2.622 / 3.155 | 2.608 / 3.211 | 3.066 / 3.744 |
| Cull p50 / p95 | 0.008 / 0.009 | 0.008 / 0.011 | 0.008 / 0.012 | 0.008 / 0.009 |
| Vis p50 / p95 | 2.101 / 2.299 | 2.093 / 2.334 | 2.092 / 2.525 | 2.099 / 2.504 |
| Shadow p50 / p95 | 0.455 / 0.670 | 0.000 / 0.000 | 0.000 / 0.000 | 0.458 / 0.852 |
| Classify p50 / p95 | 0.000 / 0.101 | 0.050 / 0.391 | 0.050 / 0.218 | 0.000 / 0.126 |
| Shade p50 / p95 | 0.376 / 0.781 | 0.264 / 0.449 | 0.264 / 0.315 | 0.371 / 0.842 |
| Present p50 / p95 | 0.171 / 0.210 | 0.172 / 0.392 | 0.171 / 0.377 | 0.171 / 0.274 |

The first candidate frame recorded 12 commands in 3.529 ms. The Shadow timestamp span was 2.387 ms and the attributed cost was 0.637 ms.

[Hermit ticks and commands](../../../out/meta/pf1a/run3/hermit-timing.json) · [Shrine ticks and commands](../../../out/meta/pf1a/run3/coral_shrine-timing.json) · [Raw pass spans and pair deltas](../../../out/meta/pf1a/run3/score.json) · [Host and triangle log](../../../out/meta/pf1a/run3.log)

## Decoded comparisons and redraw

The instrument read RGBA8 beauty, raw D32 camera and shadow textures, and the first word of each RG32Uint Vis texel for the packed kind/row ID. Camera depth was stored for readback in the capture runs. Speed ran with the default depth discard. Comparisons below are decoded pixel counts. Both Shrine arms used the same camera row order within each pair.

| A/B pair | Seconds | Beauty differences | Camera depth differences | ID differences | Shadow depth differences |
| --- | --- | --- | --- | --- | --- |
| Hermit natural | 0 / 4 / 9 | 0 / 0 / 0 | 0 / 0 / 0 | 0 / 0 / 0 | 0 / 0 / 0 |
| Shrine ascending | 0 / 4 / 9 | 0 / 0 / 0 | 0 / 0 / 0 | 0 / 0 / 0 | 0 / 0 / 0 |
| Shrine descending | 0 / 4 / 9 | 0 / 0 / 0 | 0 / 0 / 0 | 0 / 0 / 0 | 0 / 0 / 0 |

The nine candidate beauty frames also match PF1's archived control frames under the corresponding row orders. [Decoded counts and hashes](../../../out/meta/pf1a/run2/score.json) · [PF1 archive comparisons](../../../out/meta/pf1a/run2/pf1-archive-beauty.json)

At Hermit's t=9 camera, replacing the light direction with `[-0.39, 0.83, 0.39]` and recomputing the projection caused **13 → 17 → 13** commands. The redraw changed 219,448 of 1,048,576 shadow-depth texels. The redraw and the immediate reuse both matched the always-draw control in every shadow texel and in decoded beauty, camera depth and ID. Separate caster-shader probes recorded **17 → 13 → 17 → 13** when a read live parameter changed and **17 → 17 → 17** when the caster read time. The first frames of the original Hermit and Shrine sequences drew 17 and 12 commands respectively, including Shadow before Shade. [Hermit redraw ticks](../../../out/meta/pf1a/run2/hermit/stamps.json) · [Parameter and time probes](../../../out/meta/pf1a/run2/dependencies.json) · [Ascending Shrine ticks](../../../out/meta/pf1a/run2/shrine-ascending/stamps.json) · [Descending Shrine ticks](../../../out/meta/pf1a/run2/shrine-descending/stamps.json)

## Checks and reproduction

`CARGO_BUILD_JOBS=2 tools/dev.sh check loft-cli` and the lab workspace build passed. Static checks passed, including 281/281 expected outcomes in both grammar recognizers. The locked native suite passed **153/154** across 25 executables, including the new load-time shader proof test. The existing runner Tier 0 promotion test measured **0.519 ms** overhead p95 against **0.500 ms**, with zero of 140,000 pixels over 1/255. The workspace doctest command passed. [Native suite](../../../out/meta/pf1a/native-tests.log) · [Doctests](../../../out/meta/pf1a/doctests.log) · [Static checks](../../../out/meta/pf1a/static.log) · [Compile check](../../../out/meta/pf1a/check-final.log)

The two probe sources and the capture patch are in the primary checkout's `out/meta/pf1a/instrument/`. For the corrected probe, copy `retry/pf1a/` to `crates/loft-cli/examples/pf1a/`, apply `capture.patch`, then run the following from the workspace root. The capture patch is diagnostic and the timed branch uses the production texture usages and load/store operations.

```sh
CARGO_BUILD_JOBS=2 tools/dev.sh build loft-cli --fast --example pf1a
CARGO_BUILD_JOBS=2 LOFT_PF1A_BAAB=1 tools/gpu_lock.sh bash \
  "$HOME/git/loft/out/meta/pf1a/run-probe.sh" /absolute/evidence/output
```

[Scoring script](../../../out/meta/pf1a/score.py) · [GPU score wrapper](../../../out/meta/pf1a/run-probe.sh) · [Corrected probe binary hash](../../../out/meta/pf1a/instrument/retry/sha256.txt)
