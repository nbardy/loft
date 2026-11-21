# V15 performance

Release build on Apple M4 Metal. Base is unified execution (`5e252da`,
render-equivalent to `origin/main` at `78ef244`). Six alternating A/B pairs
under `tools/quiet_gpu.sh` used 30 warmups and 300 measured frames at 1280 × 800.
Triangle p95 ranged from 0.098 to 0.110 ms.

| Median batch GPU p95 (ms) | Base | Candidate | Paired difference, ≥95% CI (ms) |
| --- | ---: | ---: | ---: |
| Hermit-20 orbit | 3.666 | 3.569 | [−0.109, −0.066] |
| Shrine at t=0, distance 6.4 | 1.807 | 1.728 | [−0.333, +0.355] |
| 1,000-crab motion dispatch | 0.01898 | 0.02767 | [+0.00846, +0.00879] |

Hermit Shade median dropped from 2.015 to 1.924 ms. Shade samples the dirty
sky cube once per tile and keeps the 12-patch BSDF quadrature. A sky edit
adds one bake dispatch. The 2.5 ms target remains open.
Shrine tests the union of each static spatial cluster before its rows.
The interval crosses zero, so the 0.25–0.45 ms saving remains unverified.

CPU/Metal state and verdicts match on 481/481 ticks each for Crab Scramble,
Heron Hop and Boss Hit 1. The 149-line motion barrier passed the WGSL/MSL
leak guard. The 1,000-crab candidate interval [0.02742, 0.02783] ms meets 0.06 ms.

Deterministic pictures passed 19/19 with exact ID/depth and at most seven
one-code pixel changes per frame. Stochastic pictures passed 4/4
(41.73–42.07 dB, variance 4.49e−5), and the gain mutant failed.
Release motion and bake tests passed 17/17. Both grammars accepted 4/4 edited scenes.

[Raw measurements and captures](../../../out/meta/v15_perf/verified/final/).
