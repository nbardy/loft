# Visibility on a moving camera

Each native profile warms 30 frames, then times 120 frames on the Apple M4 with Metal at 60 Hz. All 120 must have a positive Vis timestamp. The five source fixtures preserve the pose, target, FOV, camera height and horizontal viewing distance. The camera starts on +Z and moves through 29.75 degrees. This changes which parts of the character fill the screen. The interval between timed frames is 1/60 second.

The selection rule takes the first run with both load-control brackets at or below 1.10 ms. If both attempts exceed the threshold, the table shows the first attempt and marks it C. Q marks a quiet run. Times are nearest-rank GPU frame p50 / p95 in milliseconds. The unmodified renderer in this comparison is the fixed-view-retention build at the start of this lane. [Orbit fixtures and native commands](fresh.py) · [Predictions recorded before timing](fresh_predictions.md)

| Scene | Raster | Original prediction | Original measurement | Guard prediction |
|---|---|---:|---:|---:|
| Ball | 960×720 | 4.1 / 5.1 | 2.29 / 7.93 C | 4.1 / 5.1 |
| Starter otter | 960×720 | 15.4 / 19.3 | 15.20 / 21.45 Q | 8.8 / 11.0 |
| Walrus | 960×720 | 27.2 / 34.0 | 50.62 / 61.54 Q | 19.0 / 23.8 |
| Heron | 960×720 | 27.8 / 34.8 | 42.76 / 64.05 C | 4.0 / 5.0 |
| Final otter | 960×720 | 88.3 / 110.4 | 115.43 / 161.61 C | 42.2 / 52.8 |
| Walrus | 1280×800 | 40.3 / 50.3 | 77.95 / 90.00 C | 28.1 / 35.2 |
| Heron | 1280×800 | 41.1 / 51.5 | 98.40 / 108.27 C | 5.9 / 7.4 |
| Final otter | 1280×800 | 130.7 / 163.4 | 206.75 / 226.72 C | 62.5 / 78.1 |

The previous same-size holdout predicted Vis within 30% on five scenes. Transferring its frame forecast to this larger change in azimuth underestimated all three original character medians. On the quiet walrus median frame, Vis cost 41.20 ms, classify 0.42, shade 8.64, present 0.35 and cull 0.01. The original fixed-camera budget had allocated 24.41 ms to Vis and 1.99 to shade. [Fixed-camera calibration, pass budgets and counted work](MEASUREMENTS.md)

## What the visibility proof covers

The primary guard retains the triangles that generate each interpolated ray, the march entry, the original Lipschitz constant and the full sequence of samples until the guarded exit. Its local interval covers points that can satisfy the distance-dependent hit test. A zero-set enclosure alone can exclude positive samples accepted by the original tracer. The plateau regression exercises this case against both a fresh and a cached compilation.

The shader checks that the local eye lies within 32 metres of the proof centre, that the direction has sufficient length, and that the original march segment stays inside the proof cube. Rays outside this domain use the original interval. Shadow rays keep their original sampling and projection. The compiler groups rows by the same geometric inputs as the original prover. Each retained cell contributes all six faces to the hull. Cells left at the work limit stay in the enclosure.

The direct-proxy experiment changes both the raster box and march entry. It records the old and new bounds, its own immediate timestamps and lossless frames at the same times as the original clips. Picture comparisons use corresponding decoded RGBA8 bytes. The per-frame counts include any difference in alpha. Diff images amplify the absolute channel error by 16. [Proxy instrument](../../crates/loft-cli/examples/fresh_proxy.rs) · [Timing and pixel scorer](fresh_score.py)

### Moving-camera pass budget before changes

Each row is the selected actual median-ranked frame, in GPU milliseconds. Across all 960×720 character scenes the original shadow map was retained. The camera changes on every timed frame and primary visibility runs on all 120.

| Scene | Raster | Cull | Vis | Shadow | Classify | Shade | Present | Frame |
|---|---|---:|---:|---:|---:|---:|---:|---:|
| Ball | 960×720 | 0.01 | 0.73 | 0.00 | 0.09 | 1.33 | 0.13 | 2.29 |
| Starter otter | 960×720 | 0.01 | 13.08 | 0.00 | 0.06 | 1.56 | 0.50 | 15.20 |
| Walrus | 960×720 | 0.01 | 41.20 | 0.00 | 0.42 | 8.64 | 0.35 | 50.62 |
| Heron | 960×720 | 0.01 | 41.07 | 0.00 | 0.06 | 1.44 | 0.19 | 42.76 |
| Final otter | 960×720 | 0.02 | 112.20 | 0.00 | 0.06 | 2.97 | 0.18 | 115.43 |
| Walrus | 1280×800 | 0.01 | 65.89 | 0.00 | 0.32 | 9.00 | 2.72 | 77.95 |
| Heron | 1280×800 | 0.05 | 94.00 | 0.00 | 0.33 | 3.42 | 0.59 | 98.40 |
| Final otter | 1280×800 | 0.03 | 198.71 | 0.00 | 0.29 | 4.93 | 2.78 | 206.75 |

The rate model's 7° held-out camera and the measured performance of hand guards supply the prewritten predictions. The actual orbit starts on +Z, bringing the walrus shade bill alone to 8.64 ms at the median. At 960×720, 16.7 ms minus the observed non-Vis work leaves 7.28 ms for walrus visibility, 15.01 ms for heron and 13.47 ms for final otter on these median frames. Each p95 comparison uses its own actual frame.
