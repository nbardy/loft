# perf-instrument: a Speed verdict that survives a shared Mac

## Why contention went undetected (read from source, then confirmed by measurement)

1. `tools/loft-verify/src/suite.rs` (Speed) took ONE `triangle_control` (32 serial one-triangle frames) just before
   `Loaded::new` plus 330 scene frames, which run for seconds. A control that ends before the batch cannot see a
   burst that arrives inside it.
2. The control itself is blind. A one-triangle frame is shorter than a GPU time slice. Measured: during a load-27 run
   the triangle read 0.075 ms in every block while hermit-1 blocks swung 4.3 to 11.7 ms. The 7-9 ms triangle readings
   in OPEN.md came from separate `loft profile` runs taken before and after the gate, not from the gate's own control.
3. The flag was one point estimate (`control_p95 >= 0.150` of 32 samples), and CONTENDED checks were excluded from
   the failure count.
4. `tools/gates/timing.py` used a separate cold 1-frame preflight and raised (no verdict) when it was loud.

## Theory (revised after the first measurement)

First theory: bracket 30-frame blocks with a triangle control. Refuted by (2): the triangle never moved.
Revised: the control must be as long as the work it vouches for. `Gpu::load_control` is a full-screen ALU loop
(48 iterations of fract(sin)) measuring 0.82-1.05 ms alone at 1280x800 on the M4. Observed: with control 0.9-1.05 ms
hermit-1 blocks were 3.2-3.9 ms; with control 3.8-5 ms the same blocks were 11-13 ms. Quiet bound: control p50 <= 1.10 ms.
A 2048-iteration (50 ms) control did not track the scene either; length matters both ways.

Statistic: 10 blocks of 30 frames per 300-frame orbit, each bracketed before and after by the load control; a block
whose brackets are not both quiet is re-run at the same orbit phase (up to 3 more times). Need >= 7 of 10 quiet blocks,
else the Speed p95 check is Unknown and marked CONTENDED (never a miss). Over quiet blocks: Held if pooled p95 <=
budget; Missed only if the median block p95 also exceeds the budget; otherwise Unknown ("tail only"). p50 and p95
are both reported; lines keep the `p95 ` prefix and the 12-check count.

## Measurements (Apple M4 Metal, 1280x800, 30 warmup, 300 frames, under tools/quiet_gpu.sh; loft-verify gate --speed-gate)

| Run | load avg before / after | result |
|---|---|---|
| A | 20.7 / 22.3 | 8/12 held, 0 contended (8-10 of 10 blocks quiet) |
| B | 23.6 / 6.6 | all 6 p95 CONTENDED, Unknown (0-1 quiet blocks); raw p95 3.9-22.9 ms, never scored as a miss |
| C | 6.5 / 2.8 | all 6 CONTENDED (control 1.6-2.8 ms) |
| D | 2.6 / 3.0 | hermit-1 Held 4.07 ms (7/10 quiet); 5 CONTENDED |
| mutA | 3.3 / 10.1 | 0 contended, all blocks quiet except blobs-1 7/10 |

Scenes quiet in both A and mutA (p95 ms, A vs mutA): blobs-1 1.01 vs 0.86, blobs-20 1.77 vs 1.52, capsules-20 1.25 vs
1.17, hermit-20 6.53 vs 6.05. Verdicts agree (blobs held, hermit-20 Missed, capsules-20 over budget / tail-only
Unknown). The old gate flipped these between 12/12 and 6/12 with zero contended.
Real finding: hermit-20 (6.0-6.5 vs 5.6) and capsules-20 (1.17-1.25 vs 1.1) are over budget on a quiet GPU.
hermit-1 is 6.6 (A, tail-only Unknown) and 4.07 (D, Held); it sits near the budget.

Deliberate slowdown (uncommitted gates.json edit, reverted): capsules-1 count 400 and hermit-1 count 40 -> capsules-1
p95 1.296 ms (was 0.83) Missed, hermit-1 6.97 ms (was 4.07) Missed, both with 10/10 quiet blocks.

Prediction check: (a) "6/6 quiet and within 15% across two runs" held for the four scenes quiet in both runs
(within about 15%); the machine was rarely quiet at all (B, C, D mostly contended). (b) held: contended runs gave
Unknown, never a Miss. (c) held: slowed scenes Missed.

## Not done / caveats
- Quiet bound 1.10 ms is calibrated on this M4 only.
- `loft controls` (controls.rs) keeps its own 0.35 ms triangle rule; the triangle cannot see contention, so that
  instrument has the same blind spot.
- timing.py was not run end to end; only test_scoring.py (passes).
