# Scores on Apple M4 Metal

All timing pairs are nearest-rank GPU p50 / p95 milliseconds from 120 timed frames after 30 warmups. Q means both native control brackets passed 1.10 ms. C means at least one exceeded it. The selected run is the first quiet attempt, or the first completed attempt if every one was flagged. The archive retains all attempts. Pixel counts compare corresponding frames decoded as RGBA8 at 60 Hz.

| View | Raster | Original | Retained | Matching frames, max channel difference |
| --- | --- | --- | --- | --- |
| Ball, sky | 960x720 | 4.08 / 5.66 C | 2.68 / 3.55 C | 120/120, max 0 |
| Ball, sun | 960x720 | 2.27 / 2.53 C | 0.94 / 1.85 C | 120/120, max 0 |
| Starter otter | 960x720 | 15.84 / 33.66 C | 3.92 / 4.79 C | 120/120, max 0 |
| Walrus | 960x720 | 26.53 / 99.26 C | 1.86 / 2.15 C | 120/120, max 0 |
| Heron | 960x720 | 37.01 / 41.19 C | 1.00 / 1.25 C | 120/120, max 0 |
| Final otter | 960x720 | 100.02 / 148.47 C | 1.99 / 2.09 C | — |
| Walrus | 1280x800 | 38.40 / 55.66 C | 2.50 / 2.67 C | — |
| Heron | 1280x800 | 94.81 / 114.52 C | 5.00 / 5.87 C | — |
| Final otter | 1280x800 | 189.83 / 221.20 C | 2.84 / 10.83 C | — |

## Fresh visibility predictions and work targets

The original column uses the isolated calibration. The family column normalizes on one independently timed, rotated calibration view. The shared-rate column fits two multipliers to the independent calibration views. The target view contributes counts to each forecast and no fit coefficients. The reported error is for family normalization. The last two columns add the measured non-Vis cost to one analytic query per visible analytic pixel and one or eight field evaluations per visible traced pixel. These are optimistic work targets for a new view.

| View | Cal Vis | Original | Family | Shared rates | Measured Vis | Family error | 1 query/frame ms | 8 queries/frame ms |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| Ball, sky | 0.96 | 1.24 | 2.24 | 1.50 | 1.19 | +87.8% | 3.05 | 3.43 |
| Starter otter | 10.71 | 23.52 | 22.47 | 24.75 | 14.23 | +57.9% | 1.73 | 1.86 |
| Walrus | 21.22 | 47.74 | 47.86 | 50.35 | 24.41 | +96.1% | 2.46 | 4.36 |
| Heron | 26.17 | 31.72 | 53.87 | 41.94 | 35.69 | +50.9% | 1.54 | 2.00 |
| Final otter | 64.27 | 158.20 | 140.12 | 153.15 | 96.54 | +45.1% | 3.64 | 4.20 |

## Paired same-size camera holdout

A second calibration uses a rotated camera and a full 960×720 raster. Each forecast is written to disk after calibration and before the original view runs. Direct counters at both views transfer variable cost. The archive contains the frozen predictions and the subsequent measurements.

| View | Rotated Vis | Frozen forecast | Shared-rate forecast | Original Vis | Error | Cal control | Original control |
| --- | --- | --- | --- | --- | --- | --- | --- |

## Counted work at 960×720


| View | Proxy fragments M | Geom. hits M | Hit steps M | Miss steps M | 192-step exits | Shaded pixels | Source root operations B |
| --- | --- | --- | --- | --- | --- | --- | --- |
| Ball, sky | 1.290 | 0.707 | 2.516 | 4.040 | 0 | 529,924 | 0.013 |
| Starter otter | 9.722 | 0.566 | 3.849 | 48.954 | 4,367 | 501,082 | 0.739 |
| Walrus | 35.027 | 1.278 | 12.160 | 53.337 | 135 | 503,090 | 1.326 |
| Heron | 2.428 | 0.526 | 0.590 | 61.676 | 0 | 455,706 | 1.196 |
| Final otter | 116.712 | 0.859 | 9.761 | 127.447 | 8,221 | 548,493 | 4.849 |

## Additive frame budgets

Each row is one actual frame at the native median rank. Visibility is allocated using the relative calibration rates and normalized once to that frame’s Vis timestamp. The clear/store estimate comes from pooled equal-size calibration targets. The remaining passes are GPU timestamps on that frame. The source counters measure work directly. The allocated shares of milliseconds are estimates. Rounding can change a displayed sum by 0.02 ms.

| View | March miss | March hit | Analytic miss | Analytic hit | Vis clear | Cull | Shadow | Classify | Shade | Present | Frame |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| Ball, sky | 0.66 | 0.41 | 0.02 | 0.07 | 0.04 | 0.01 | 0.00 | 0.14 | 2.40 | 0.31 | 4.08 |
| Starter otter | 10.42 | 0.82 | 2.90 | 0.04 | 0.04 | 0.01 | 0.00 | 0.08 | 1.42 | 0.10 | 15.84 |
| Walrus | 15.36 | 3.99 | 4.96 | 0.06 | 0.04 | 0.01 | 0.00 | 0.04 | 1.99 | 0.09 | 26.53 |
| Heron | 35.12 | 0.40 | 0.05 | 0.09 | 0.04 | 0.01 | 0.00 | 0.11 | 1.02 | 0.17 | 37.01 |
| Final otter | 65.13 | 4.38 | 26.95 | 0.05 | 0.04 | 0.01 | 0.00 | 0.09 | 2.62 | 0.74 | 100.02 |

## Fresh-view ablations

These runs use the original executable and recompute visibility on every frame. The analytic guard and field guard are distinct source forks. Bounds2 combines the head guard with a torso guard. Image evidence for each fork is in the last two columns.

| View | Fork | Original frame | Fork frame | Vis ms saved | Matching frames, max difference |
| --- | --- | --- | --- | --- | --- |
| Starter otter | Analytic sphere | 15.84 / 33.66 C | 14.56 / 24.15 C | 1.20 | 3/3, max 0 |
| Starter otter | Field box | 15.84 / 33.66 C | 8.04 / 14.21 C | 8.72 | 3/3, max 0 |
| Walrus | Analytic sphere | 26.53 / 99.26 C | 26.05 / 31.13 C | 0.52 | 3/3, max 0 |
| Final otter | Analytic sphere | 100.02 / 148.47 C | 97.77 / 116.42 C | 1.56 | 3/3, max 0 |

## Work available for further engine changes

These conditional savings overlap. All are relative to original fresh Vis. The bounds column removes 90% of miss evaluations in the measured giant fields. The strand column charges four proxy fragments per true hit plus 36 work units per row. The pruning column halves the arithmetic in fields with at least eight expanded source root operations. Those efficiencies are design targets, not measured gains.

| View | Certified inner bound | Tighter thin proxies | Half composite arithmetic |
| --- | --- | --- | --- |
| Ball, sky | 0.00 | 0.00 | 0.00 |
| Starter otter | 9.38 | 2.87 | 5.62 |
| Walrus | 11.05 | 4.87 | 7.78 |
| Heron | 29.75 | 0.00 | 16.86 |
| Final otter | 55.39 | 26.01 | 34.18 |

The archive contains the actual brackets, 120 pass and frame observations per profile, per-kind counters, per-frame image digests and the independent calibration records. [Download raw measurements](measurements.tar.gz).
