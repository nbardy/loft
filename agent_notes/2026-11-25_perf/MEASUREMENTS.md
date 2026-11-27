# Scores on Apple M4 Metal

All timing pairs are nearest-rank GPU p50 / p95 milliseconds from 120 timed frames after 30 warmups. Q means both native control brackets passed 1.10 ms. C means at least one exceeded it. The selected run is the first quiet attempt, or the first completed attempt if every one was flagged. The archive retains all attempts. Pixel counts compare corresponding frames decoded as RGBA8 at 60 Hz.

| View | Raster | Original | Retained | Matching frames, max channel difference |
| --- | --- | --- | --- | --- |
| Ball, sky | 960x720 | 4.08 / 5.66 C | 2.68 / 3.55 C | 120/120, max 0 |
| Ball, sun | 960x720 | 2.27 / 2.53 C | 0.94 / 1.85 C | 120/120, max 0 |
| Starter otter | 960x720 | 15.84 / 33.66 C | 3.92 / 4.79 C | 120/120, max 0 |
| Walrus | 960x720 | 26.53 / 99.26 C | 1.86 / 2.15 C | 120/120, max 0 |
| Heron | 960x720 | 37.01 / 41.19 C | 1.00 / 1.25 C | 120/120, max 0 |
| Final otter | 960x720 | 100.02 / 148.47 C | 1.86 / 2.08 Q | 120/120, max 0 |
| Walrus | 1280x800 | 38.40 / 55.66 C | 2.50 / 2.67 C | 119/120, max 24 |
| Heron | 1280x800 | 94.81 / 114.52 C | 5.00 / 5.87 C | 120/120, max 0 |
| Final otter | 1280x800 | 189.83 / 221.20 C | 2.84 / 10.83 C | 120/120, max 0 |

## Fresh visibility predictions and work targets

The original column uses the isolated calibration. The family column normalizes on one independently timed, rotated calibration view. The shared-rate column fits two multipliers to the independent calibration views. The target view contributes counts to each forecast and no fit coefficients. The reported error is for family normalization. The last two columns use the equal-raster calibration when available. They add the measured non-Vis cost to one analytic query per visible analytic pixel and one or eight field evaluations per visible traced pixel. These are optimistic work targets for a new view.

| View | Cal Vis | Original | Family | Shared rates | Measured Vis | Family error | 1 query/frame ms | 8 queries/frame ms |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| Ball, sky | 0.96 | 1.24 | 2.24 | 1.50 | 1.19 | +87.8% | 2.99 | 3.19 |
| Starter otter | 10.71 | 23.52 | 22.47 | 24.75 | 14.23 | +57.9% | 1.70 | 1.78 |
| Walrus | 21.22 | 47.74 | 47.86 | 50.35 | 24.41 | +96.1% | 2.32 | 3.31 |
| Heron | 26.17 | 31.72 | 53.87 | 41.94 | 35.69 | +50.9% | 1.44 | 1.67 |
| Final otter | 64.27 | 158.20 | 140.12 | 153.15 | 96.54 | +45.1% | 3.59 | 3.93 |

## Paired same-size camera holdout

A second calibration uses a rotated camera and a full 960×720 raster. Each forecast is written to disk after calibration and before the original view runs. Direct counters at both views transfer variable cost. The archive contains the frozen predictions and the subsequent measurements.

| View | Rotated Vis | Frozen forecast | Shared-rate forecast | Original Vis | Error | Cal control | Original control |
| --- | --- | --- | --- | --- | --- | --- | --- |
| Ball, sky | 1.08 | 1.25 | 0.87 | 1.11 | +12.9% | C | C |
| Starter otter | 13.39 | 13.80 | 14.03 | 13.12 | +5.1% | C | C |
| Walrus | 22.68 | 25.09 | 28.52 | 19.65 | +27.7% | Q | C |
| Heron | 26.19 | 26.44 | 23.76 | 32.75 | -19.3% | Q | C |
| Final otter | 79.42 | 84.87 | 86.71 | 73.18 | +16.0% | C | Q |

## Counted work at 960×720


| View | Proxy fragments M | Geom. hits M | Hit steps M | Miss steps M | 192-step exits | Shaded pixels | Source root operations B |
| --- | --- | --- | --- | --- | --- | --- | --- |
| Ball, sky | 1.290 | 0.707 | 2.516 | 4.040 | 0 | 529,924 | 0.013 |
| Starter otter | 9.722 | 0.566 | 3.849 | 48.954 | 4,367 | 501,082 | 0.739 |
| Walrus | 35.027 | 1.278 | 12.160 | 53.337 | 135 | 503,090 | 1.326 |
| Heron | 2.428 | 0.526 | 0.590 | 61.676 | 0 | 455,706 | 1.196 |
| Final otter | 116.712 | 0.859 | 9.761 | 127.447 | 8,221 | 548,493 | 4.849 |

## Additive frame budgets

Each row is one actual frame at the median rank in the first original profile. Later quiet retries and the camera holdout are reported separately. Visibility is allocated using the relative calibration rates and normalized once to that frame’s Vis timestamp. The clear/store estimate comes from pooled equal-size calibration targets. The remaining passes are GPU timestamps on that frame. The source counters measure work directly. The allocated shares of milliseconds are estimates. Rounding can change a displayed sum by 0.02 ms.

| View | March miss | March hit | Analytic miss | Analytic hit | Vis clear | Cull | Shadow | Classify | Shade | Present | Frame |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| Ball, sky | 0.66 | 0.41 | 0.02 | 0.07 | 0.04 | 0.01 | 0.00 | 0.14 | 2.40 | 0.31 | 4.08 |
| Starter otter | 10.42 | 0.82 | 2.90 | 0.04 | 0.04 | 0.01 | 0.00 | 0.08 | 1.42 | 0.10 | 15.84 |
| Walrus | 15.36 | 3.99 | 4.96 | 0.06 | 0.04 | 0.01 | 0.00 | 0.04 | 1.99 | 0.09 | 26.53 |
| Heron | 35.12 | 0.40 | 0.05 | 0.09 | 0.04 | 0.01 | 0.00 | 0.11 | 1.02 | 0.17 | 37.01 |
| Final otter | 65.13 | 4.38 | 26.95 | 0.05 | 0.04 | 0.01 | 0.00 | 0.09 | 2.62 | 0.74 | 100.02 |

## Fresh-view ablations

These runs use the original executable and recompute visibility on every frame. The analytic guard and field guard are distinct source forks. Bounds2 combines the head guard with a torso guard. Image evidence for each fork is in the last two columns.

| View | Fork | Initial original frame | Same-slot original frame | Fork frame | Vis saved vs initial | Vis saved vs same slot | Matching frames, max difference |
| --- | --- | --- | --- | --- | --- | --- | --- |
| Starter otter | Analytic sphere | 15.84 / 33.66 C | — | 14.56 / 24.15 C | 1.20 | — | 3/3, max 0 |
| Starter otter | Field box | 15.84 / 33.66 C | 16.05 / 27.34 C | 8.04 / 14.21 C | 8.72 | 9.09 | 120/120, max 0 |
| Walrus | Analytic sphere | 26.53 / 99.26 C | — | 26.05 / 31.13 C | 0.52 | — | 3/3, max 0 |
| Walrus | Field box | 26.53 / 99.26 C | 27.18 / 43.68 C | 20.64 / 23.70 C | 6.44 | 7.17 | 120/120, max 0 |
| Heron | Field box | 37.01 / 41.19 C | 44.46 / 48.84 C | 8.71 / 10.33 C | 30.73 | 37.84 | 120/120, max 0 |
| Final otter | Analytic sphere | 100.02 / 148.47 C | — | 97.77 / 116.42 C | 1.56 | — | 3/3, max 0 |
| Final otter | Field box | 100.02 / 148.47 C | 124.64 / 151.24 C | 54.11 / 61.15 C | 44.36 | 69.15 | 120/120, max 0 |
| Final otter | Head + torso | 100.02 / 148.47 C | — | 81.67 / 94.54 C | 20.99 | — | 120/120, max 0 |

The all-ray counter also replayed the original march with the exact guard intervals at the original camera. Its work estimate applies the original kind throughput to the measured reduction in evaluations.

| View | Field | Original steps M | Guarded steps M | Geometric hit delta | Estimated Vis saved ms |
| --- | --- | --- | --- | --- | --- |
| Starter otter | Otter | 52.80 | 20.72 | 0 | 6.83 |
| Walrus | Cap | 21.31 | 5.46 | 0 | 2.41 |
| Walrus | Hide | 33.81 | 15.89 | 0 | 5.58 |
| Heron | HeronHead | 59.29 | 1.38 | 0 | 32.36 |
| Final otter | Head4 | 71.40 | 9.32 | 0 | 21.65 |
| Final otter | Torso | 51.32 | 11.04 | 0 | 30.80 |

The final otter also has two adjacent fresh-view triplets within a single GPU reservation. The second takes the reverse order. All use the original binary at 960×720. The measured reduction in evaluations had predicted 21.65 ms for the head plus 30.80 ms for the torso at the original calibration rates.

| Run order | Final otter | Vis p50 | Frame p50 | Frame p95 | Control |
| --- | --- | --- | --- | --- | --- |
| 1 | Original | 60.08 | 61.99 | 91.96 | C |
| 1 | Head | 49.00 | 50.99 | 70.42 | C |
| 1 | Head and torso | 31.57 | 33.58 | 47.01 | C |
| 2 | Head and torso | 30.03 | 31.84 | 39.14 | C |
| 2 | Head | 49.17 | 51.07 | 87.85 | C |
| 2 | Original | 78.91 | 81.42 | 90.20 | C |

## Ball lighting control

Two integrations retain ambient occlusion and two make it dead. The sun-only AO control multiplies the direct term by AO to keep its five field samples live. These controls change lighting by design. They are excluded from all picture-preserving speedups.

| Integrator | Median shade ms | Attempts | Selected control |
| --- | --- | --- | --- |
| sky | 2.38 | 3 | C |
| sky_no_ao | 2.16 | 3 | C |
| sun_ao | 0.99 | 3 | C |
| sun | 0.65 | 3 | C |

## Work available for further engine changes

These conditional savings overlap. All are relative to original fresh Vis. The bounds column removes 90% of miss evaluations in the measured giant fields. The strand column charges four proxy fragments per true hit plus 36 work units per row. The pruning column halves the arithmetic in fields with at least eight expanded source root operations. Those efficiencies are design targets, not measured gains.

| View | Certified inner bound | Tighter thin proxies | Half composite arithmetic |
| --- | --- | --- | --- |
| Ball, sky | 0.00 | 0.00 | 0.00 |
| Starter otter | 9.38 | 2.87 | 5.62 |
| Walrus | 11.05 | 4.87 | 7.78 |
| Heron | 29.75 | 0.00 | 16.86 |
| Final otter | 55.39 | 26.01 | 34.18 |

## Paired deferred-timestamp check

These are the ring half of native immediate/ring A/B runs in a separate GPU-slot window. The same mode applies to both members of each pair.

| View | Raster | Original ring | Retained ring |
| --- | --- | --- | --- |
| Walrus | 960x720 | 30.68 / 32.72 Q | 4.35 / 4.84 C |
| Heron | 960x720 | 40.13 / 41.64 Q | 2.51 / 4.21 Q |
| Final otter | 960x720 | 114.12 / 131.77 Q | 5.04 / 11.10 Q |
| Walrus | 1280x800 | 48.61 / 55.67 C | 7.48 / 10.09 C |
| Heron | 1280x800 | 58.09 / 101.99 Q | 3.11 / 3.62 C |
| Final otter | 1280x800 | 212.96 / 233.16 C | 6.54 / 7.18 C |

## Native contention brackets across retries

These are all immediate observations from the four standard attempts and the immediate half of each A/B run, followed by the ring observations. Each count is quiet / total for that scene, build and raster. The held-out camera and experimental bounds studies have their own controls.

| View | Raster | Original immediate | Original ring | Retained immediate | Retained ring |
| --- | --- | --- | --- | --- | --- |
| Walrus | 960x720 | 0/5 | 1/1 | 0/6 | 0/2 |
| Heron | 960x720 | 0/5 | 1/1 | 0/6 | 1/2 |
| Final otter | 960x720 | 0/5 | 1/1 | 1/5 | 1/1 |
| Walrus | 1280x800 | 0/6 | 0/2 | 0/6 | 0/2 |
| Heron | 1280x800 | 0/5 | 1/1 | 0/6 | 0/2 |
| Final otter | 1280x800 | 0/6 | 0/2 | 0/6 | 0/2 |

The archive contains the actual brackets, 120 pass and frame observations per profile, per-kind counters, per-frame image digests and the independent calibration records. [Download raw measurements](measurements.tar.gz).
