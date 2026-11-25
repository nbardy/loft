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

### Compiler coverage and cost

The recovered draft contributed the second LUT interval, the outward float evaluator, the exit-only WGSL change, hot-edit upload, orbit fixtures and direct-proxy probe. This continuation corrected translation recognition in checked maps, float endpoint widening, interpolation coverage and the work-limit traversal, then added six exterior slab searches for coarse survivors. The six largest character fields are now bounded automatically.

| Field | Original longest side (m) | Certified longest side (m) | Extra range/point evaluations with the original certificate cached |
|---|---:|---:|---:|
| Starter Otter | 128.028 | 0.518 | 9,588 |
| Walrus Hide | 512.104 | 0.918 | 26,847 |
| Walrus Cap | 192.040 | 0.307 | 7,027 |
| HeronHead | 128.028 | 0.283 | 3,965 |
| Final Head4 | 128.028 | 0.268 | 6,884 |
| Final Torso | 128.028 | 0.408 | 9,008 |

The initial traversal uses a 31.25 mm leaf width and approximately 24,000 evaluations per distinct shape. On exhaustion each of six faces gets up to 2,048 extra outward interval evaluations. An inconclusive slab stays in the result. These limits bound compilation time and may leave a face farther than one cell from the surface. With the original proof cached and the guard cold, whole-scene check times were 10.49 s for walrus, 2.57 s for heron, 6.60 s for final otter, 1.50 s for starter otter and 0.065 s for ball. Repeated checks reuse the on-disk certificate. The guard uses 40 bytes per traced row plus one word per drawn kind. [Per-kind compiler readings](orbit/certified-derivation.txt) · [All interval coordinates](orbit/certified-bounds.json.gz)

All 5,042 original serialized instance and kind records match the guarded program byte-for-byte. A second compile had zero bound evaluations in all five scenes. The shifted-sphere live edit changed 256,034 pixels and had zero pixels over 1/255 against a fresh load at 1280×800. Twelve focused tests passed, including a case where real arithmetic places a surface inside 5 mm while f32 cancellation makes the original raster boundary hittable. The static check matched 360/365 grammar expectations. Both recognizers rejected five pinned source fixtures also present before this change. The grammar, its checker and those five inputs have identical Git contents at the original and revised revisions. Corpus and dependency-pin checks passed. [Compiler and executable hashes](orbit/compiler-parity.json) · [Focused tests](orbit/focused-tests.txt) · [Live edit](orbit/live-edit.txt) · [Static check](orbit/static.txt)

The outward check encloses individual f32 operations in the evaluator's expression order and both common interpolation forms. WGSL also permits implementations to reassociate expressions. The mathematical interval therefore describes the evaluated expression, and the matching-frame count separately tests the shipped Metal program. General equivalence under arbitrary driver reassociation remains unverified. [WGSL floating-point evaluation](https://www.w3.org/TR/WGSL/#floating-point-evaluation)

At orbit frames 0, 60 and 119 each giant outer box admits all 691,200 ideal pixel-centre rays. HeronHead's guard admits 80,961–94,072 and Cap admits 84,137–94,796, both below the predicted quarter-screen ceiling. Hide still admits 493,664–539,498 rays because the body fills the forward view. Shortening a surviving miss's exit can still save work. At the midpoint Head4 admits 63,631 rays, Torso 159,436 and the starter head 200,089. These CPU counts use the original camera and mathematical box intersections. GPU pass times include rasterization, interpolation and divergence. [Projection counts at the three poses](orbit/projected-counts.json)

### Authoring a moving character

- Use the stock exact visibility query for a primitive when its geometry matches.
- Group smooth parts that blend together. Give spatially separate parts independent bounds.
- Put surface detail that does not affect the silhouette in material.
- Read the derived L in `loft check`. Inspect expensive kinds with `sdf_steps` and `overdraw`.
- Compare fresh GPU pass times with a moving camera at the delivery resolution. Read both load-control brackets and compare corresponding beauty frames when changing geometry.

### Adjacent moving-camera results for change 1

The two builds ran consecutively for each input. All 120 measured primary visibility passes executed in every run. Each entry is GPU p50 / p95 in ms. The table above preserves the earlier operating window. The paired character baselines were up to 2.6 times faster than the earlier medians even though the binary and input were identical.

| Scene | Raster | Paired original | With guard | Guard median frame, Cull / Vis / Shadow / Classify / Shade / Present |
|---|---|---:|---:|---|
| Ball | 960×720 | 4.36 / 6.13 C | 2.49 / 4.45 Q | 0.01 / 0.88 / 0 / 0.08 / 1.35 / 0.19 |
| Starter otter | 960×720 | 9.90 / 10.76 C | 7.14 / 9.61 Q | 0.01 / 5.18 / 0 / 0.06 / 1.56 / 0.33 |
| Walrus | 960×720 | 21.59 / 23.32 C | 11.48 / 13.16 C | 0.01 / 9.80 / 0 / 0.04 / 1.57 / 0.06 |
| Heron | 960×720 | 25.95 / 31.20 C | 7.98 / 9.30 C | 0.02 / 4.00 / 0 / 0.13 / 3.42 / 0.42 |
| Final otter | 960×720 | 57.82 / 62.82 C | 26.14 / 33.85 C | 0.01 / 24.41 / 0 / 0.03 / 1.62 / 0.06 |
| Walrus | 1280×800 | 26.12 / 26.97 C | 13.13 / 14.52 C | 0.01 / 11.09 / 0 / 0.05 / 1.89 / 0.10 |
| Heron | 1280×800 | 38.42 / 40.90 C | 10.23 / 12.40 C | 0.02 / 4.82 / 0 / 0.19 / 4.70 / 0.50 |
| Final otter | 1280×800 | 78.36 / 85.80 C | 31.64 / 35.85 C | 0.01 / 29.27 / 0 / 0.05 / 2.21 / 0.10 |

The observed 960 guard p95 was 4.3 ms above forecast on heron and 10.6 / 19.0 ms below forecast on walrus / final otter. Operating-window differences affect even the ball control. On the larger final otter the guarded p95 still needs 19.15 ms removed to reach budget. The actual p95-ranked pass costs and the large run-to-run control shifts belong in the final budget. [Original timings](orbit/paired-original) · [Guard timings](orbit/revised)

The paired p95-ranked final otter frames spent 32.02 ms and 33.49 ms in Vis at the two sizes. Keeping the other passes at their observed costs leaves 14.87 ms and 14.34 ms for primary visibility. Even the small raster needs a further 54% reduction in that pass on the sampled tail frame. The guarded heron median still paid 3.42 / 4.70 ms in shade. The original calibrated model counted relatively little shading at the original camera and operating point.

The proof attempts every eligible traced kind. A query with a clock dependency, live geometry parameter, scheduled law, foreign row access, bake, raw helper or an unsupported operation uses its original interval. Authored Lipschitz estimates and the rotation, ellipsoid and round-cone rules are unchanged. Traced guards are derived once per distinct eligible shape and loaded with its rows. The camera can still move independently.
