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

### Acceptance against the original moving clips

The table covers 960 timed images per build. A differing pixel counts once per frame if any RGBA8 channel differs. Maximum channel error is measured on the 0–255 scale. The two pixel counts are the largest count on any frame and the sum over all 120 frames. The linked PNGs amplify channel differences by 16.

| Scene and size | Guard exact / 120 | Guard max channel | Guard changed pixels, peak / total | Direct proxy p50 / p95 ms | Direct max channel | Direct changed pixels, peak / total |
|---|---:|---:|---:|---:|---:|---:|
| ball-960x720 | 120 | 0 | 0 / 0 | 4.31 / 4.47 C | 127 | [158 / 12,131](orbit/proxy/proxy-ball-960x720.png) |
| base-960x720 | 120 | 0 | 0 / 0 | 8.20 / 15.01 Q | 203 | [6,854 / 729,501](orbit/proxy/proxy-base-960x720.png) |
| walrus-960x720 | 84 | 107 | [4 / 48](orbit/revised/revised-walrus-960x720.png) | 9.56 / 10.68 Q | 229 | [46,225 / 5,273,955](orbit/proxy/proxy-walrus-960x720.png) |
| heron-960x720 | 120 | 0 | 0 / 0 | 7.25 / 9.71 C | 252 | [18,224 / 1,840,609](orbit/proxy/proxy-heron-960x720.png) |
| otter-960x720 | 120 | 0 | 0 / 0 | 23.69 / 35.70 Q | 238 | [21,641 / 2,375,389](orbit/proxy/proxy-otter-960x720.png) |
| walrus-1280x800 | 78 | 106 | [3 / 52](orbit/revised/revised-walrus-1280x800.png) | 11.42 / 12.57 Q | 226 | [68,108 / 7,857,169](orbit/proxy/proxy-walrus-1280x800.png) |
| heron-1280x800 | 120 | 0 | 0 / 0 | 8.99 / 9.99 C | 252 | [22,415 / 2,267,752](orbit/proxy/proxy-heron-1280x800.png) |
| otter-1280x800 | 120 | 0 | 0 / 0 | 29.77 / 39.55 Q | 238 | [26,772 / 2,932,950](orbit/proxy/proxy-otter-1280x800.png) |

The direct proxy changed every frame on every fixture. Its 960 median improvement relative to the guard in the other GPU window was 1.92 ms on walrus, 0.73 ms on heron and 2.45 ms on final otter, all below forecast. Its otter p95 increased at both sizes. Changing the raster and first sample gives a visible cost for little measured time on these views.

The guarded heron, both otters and ball preserve the archived reference images. The default walrus comparison fails strict frame equality at both sizes. The row-order and repeated-original controls are reported separately. All timings, per-frame hashes and counts are stored alongside the diff images. [Guard scores](orbit/revised) · [Direct-proxy scores](orbit/proxy)

### Thin rows and prior work

Sigg, Weyrich, Botsch and Gross projected quadrics to bounded screen primitives and solved the surface at each covered pixel. Tarini, Cignoni and Montani used procedural impostors for molecular atoms and bonds. Loft already has per-pixel capsule and rounded-cone solvers. The thin-row experiment keeps the existing 36 cube vertices and their varying, computes two parallel screen support lines for the endpoint balls, and clips the cube to that strip using homogeneous depth. The fragment still supplies the surface depth. This trades additional vertex arithmetic for fewer fragment launches within a single existing draw. [Sigg et al., 2006](https://cgl.ethz.ch/Downloads/Publications/Papers/2006/Sig06/Sig06.pdf) · [Tarini et al., 2006](https://www.rbvi.ucsf.edu/chimera/data/ambient-jul2014/Tarini_FinalVersionElec.pdf)

Knoll and colleagues used interval and affine arithmetic to exclude empty ray intervals. The first change here applies an outward interval check during compilation to the Loft marcher's distance-dependent acceptance band. Zanni’s tile lists and sparse traversal address a different remaining expense, evaluating a whole composite field where only a few operands can matter. [Knoll et al.](https://www.sci.utah.edu/~knolla/cgrtia.pdf) · [Zanni, 2023](https://arxiv.org/abs/2304.09673)

The strip build is recorded at `8188ae85`. A second build at `5ad3bfc7` draws up to eight original rows at a time under a hardware scissor enclosing their projected endpoint balls. It keeps the original clip position and interpolants. Primary rows in these direct draws use authored row order. The ordinary compacted order can choose different rows at a depth tie. The stock visibility schedule and immutable row data are required; a host that loads serialized IR uses the ordinary indirect draw. The scissor projection runs on the native CPU each frame, and every visible eight-row group adds a GPU draw. The shader recognizes the checked direct call and the complete stock solver module by content. [Before-run coverage predictions](orbit/thin-projection.txt) · [Fixed-order replay](thin_replay.py) · [Color and timing protocol](fresh_score.py)

### Native ring timing for change 1

The native AB command times an immediate pass and then another 30 warmups and 120 timed frames using the device’s timestamp ring. Each half has its own pair of load-control brackets. The table uses the first quiet ring attempt, or the first flagged one after retry. Absolute engine time changes materially between these measurement windows.

| Character and size | Original ring p50 / p95 ms | Guard ring p50 / p95 ms | Guard brackets before / after ms |
|---|---:|---:|---:|
| walrus-960x720 | 26.20 / 33.11 Q | 16.30 / 24.27 Q | 0.49 / 0.72 |
| heron-960x720 | 46.59 / 55.70 C | 2.02 / 3.04 C | 1.94 / 0.46 |
| otter-960x720 | 93.31 / 111.30 Q | 46.38 / 57.60 Q | 0.57 / 0.63 |
| walrus-1280x800 | 38.73 / 44.24 Q | 21.77 / 29.74 Q | 0.91 / 0.90 |
| heron-1280x800 | 46.11 / 49.27 Q | 2.66 / 3.77 C | 2.39 / 0.75 |
| otter-1280x800 | 155.69 / 166.07 Q | 68.27 / 87.01 C | 2.81 / 1.02 |

At 960 the second original-only walrus capture differed from the first original on 42 of 120 PNGs. On 20 of those 42, the original repeat exactly reproduced the guarded PNG. This establishes intrinsic baseline variation at the moving camera. It does not assign the remaining pixels to the culler. The fixed-order test evaluates that explanation using the same cull kernel on both builds. [Original repeat metadata](orbit/repeat-original)

Timing and picture comparisons use the stock CLI’s two time generators. The profile multiplies an f32 frame index by an f32 interval; the clip rounds a double-precision frame time once. On this orbit 76/120 profile timestamps are one f32 ULP above the corresponding clip timestamp. Every profile pair uses the same profile times, and every image pair uses the same clip times.

At 1280×800, frame 60 of the walrus differs between the screen-strip and guard builds at 1,140 pixels, with a largest channel error of 133. The strip already fails the picture requirement on this frame. It is an experimental timing result. The hardware-scissor build uses the old clip coordinates throughout. [Strip shader](strip.wgsl) · Eight-row CPU projection in `5ad3bfc7:crates/loft-native/src/scissor.rs`.

### Raster-strip timing and image differences

In the strip build, every 120-frame clip using an eligible solver changed on every frame. Ball and heron have no eligible solver. All timing pairs below used immediate timestamps and both attempts were flagged. Times are GPU p50 / p95 in milliseconds. Each linked image shows the largest channel error across the 120-frame original comparison. The last four numbers are exact frames / largest channel error / peak changed pixels on a frame / total changed pixels.

| Scene and raster | Guard in this pair | Predicted guard + strip | Measured guard + strip | RGBA result |
|---|---:|---:|---:|---|
| ball-960x720 | 2.37 / 4.26 C | 2.5 / 4.5 | 4.02 / 5.17 C | 120 / 0 / 0 / 0 |
| base-960x720 | 7.78 / 9.87 C | 5.8 / 8.1 | 7.09 / 8.74 C | [0 / 176 / 474 / 39,785](orbit/thin/thin-base-960x720.png) |
| walrus-960x720 | 13.05 / 15.74 C | 8.2 / 10.2 | 13.81 / 19.99 C | [0 / 243 / 1,302 / 122,066](orbit/thin/thin-walrus-960x720.png) |
| heron-960x720 | 6.06 / 10.07 C | 8.0 / 9.3 | 6.43 / 9.32 C | 120 / 0 / 0 / 0 |
| otter-960x720 | 39.21 / 49.31 C | 12 / 16 | 15.89 / 18.68 C | [0 / 244 / 851 / 87,694](orbit/thin/thin-otter-960x720.png) |
| walrus-1280x800 | 16.54 / 20.01 C | 9 / 11 | 13.21 / 16.16 C | [0 / 253 / 1,588 / 137,036](orbit/thin/thin-walrus-1280x800.png) |
| heron-1280x800 | 5.59 / 9.55 C | 10.2 / 12.4 | 2.82 / 3.54 C | 120 / 0 / 0 / 0 |
| otter-1280x800 | 48.27 / 65.04 C | 15.5 / 19.5 | 17.67 / 20.31 C | [0 / 244 / 949 / 99,851](orbit/thin/thin-otter-1280x800.png) |
At 960 the strip saved 30.63 ms at the final otter’s p95 and added 4.25 ms at the walrus’s p95 relative to the immediately preceding guard. Predicting total time directly from fragment count overestimated the gain on the walrus. The calibrated per-fragment rate also depends on primitive size, GPU operating state and the work remaining outside that draw.

### Fragment launches and visibility words

At frame 60 of the 960×720 orbit, the instrument clears the same textures and draws one kind in a fixed row order. Each time is the median of seven uncensored draws. The hit counter counts successful per-fragment solves, including overlapping surfaces. The final two columns count pixels where either 32-bit visibility word differs from the original draw.

| Kind | Original launches | Strip launches | Scissor launches | Original hits | Original / strip / scissor ms | Strip changed pixels | Scissor changed pixels |
|---|---:|---:|---:|---:|---|---:|---:|
| [Whisker](orbit/thin/base-Whisker-vis-replay.json.gz) | 6,916,484 | 114,163 | 411,915 | 5,133 | 3.61 / 0.36 / 0.54 | 3,727 | 0 |
| [Rod](orbit/thin/walrus-Rod-vis-replay.json.gz) | 31,202,634 | 1,878,641 | 9,970,921 | 177,414 | 8.66 / 2.20 / 6.59 | 35,833 | 0 |
| [Bit](orbit/thin/otter-Bit-vis-replay.json.gz) | 3,238,049 | 164,730 | 731,529 | 23,616 | 1.72 / 0.29 / 0.48 | 19,078 | 0 |
| [Strand](orbit/thin/otter-Strand-vis-replay.json.gz) | 96,849,778 | 4,230,331 | 2,503,784 | 10,819 | 22.86 / 3.68 / 2.72 | 8,514 | 0 |
All twelve fixed-order scissor replays, covering four kinds at frames 0, 60 and 119, preserved every visibility word. At the middle pose, the coverage model predicted 2.508 million scissored Strand fragments and measured 2.504 million. It predicted 4.244 million for the strip and measured 4.230 million. A correct launch-count model can still miss native p95. Full-frame timing includes the other kinds, pass scheduling and the cost of hundreds of draws.

### Eight-row hardware scissors on the moving camera

Each entry is GPU p50 / p95 in ms. In the left comparison original and guard ran consecutively. In the right comparison the same guard binary ran immediately before the combined build. The forecast column was frozen before the scissor build and applies to the combined build. Original and guard forecasts are in the first table.

| Scene and raster | Original | Guard | Adjacent guard | Predicted with both | Measured with both |
|---|---:|---:|---:|---:|---:|
| ball-960x720 | 4.36 / 6.13 C | 2.49 / 4.45 Q | 4.44 / 8.76 C | 2.5 / 4.5 | 4.47 / 7.02 C |
| base-960x720 | 9.90 / 10.76 C | 7.14 / 9.61 Q | 7.42 / 10.09 C | 5.5 / 8 | 4.68 / 6.71 C |
| walrus-960x720 | 21.59 / 23.32 C | 11.48 / 13.16 C | 11.19 / 12.24 C | 9.5 / 12 | 9.57 / 10.83 C |
| heron-960x720 | 25.95 / 31.20 C | 7.98 / 9.30 C | 8.28 / 10.11 C | 8 / 9.3 | 4.27 / 8.65 C |
| otter-960x720 | 57.82 / 62.82 C | 26.14 / 33.85 C | 31.10 / 37.76 C | 13 / 18 | 13.06 / 15.96 C |
| walrus-1280x800 | 26.12 / 26.97 C | 13.13 / 14.52 C | 14.48 / 16.64 C | 11 / 13.5 | 11.71 / 13.28 Q |
| heron-1280x800 | 38.42 / 40.90 C | 10.23 / 12.40 C | 6.06 / 9.62 C | 10.2 / 12.4 | 5.99 / 9.74 C |
| otter-1280x800 | 78.36 / 85.80 C | 31.64 / 35.85 C | 36.57 / 44.79 Q | 16 / 21 | 14.36 / 16.96 Q |

All six passes in these budgets come from the same actual combined-build p95-ranked frame, in GPU milliseconds. The last column gives the extra time above 16.7 ms at that rank.

| Scene and raster | Cull | Vis | Shadow | Classify | Shade | Present | Frame | Excess |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| ball-960x720 | 0.01 | 1.57 | 0.00 | 0.18 | 4.74 | 0.51 | 7.02 | 0.00 |
| base-960x720 | 0.01 | 5.17 | 0.00 | 0.05 | 1.36 | 0.12 | 6.71 | 0.00 |
| walrus-960x720 | 0.01 | 9.18 | 0.00 | 0.03 | 1.46 | 0.15 | 10.83 | 0.00 |
| heron-960x720 | 0.02 | 5.03 | 0.00 | 0.13 | 3.21 | 0.26 | 8.65 | 0.00 |
| otter-960x720 | 0.01 | 14.14 | 0.00 | 0.04 | 1.62 | 0.15 | 15.96 | 0.00 |
| walrus-1280x800 | 0.01 | 10.54 | 0.00 | 0.06 | 2.58 | 0.11 | 13.28 | 0.00 |
| heron-1280x800 | 0.03 | 5.84 | 0.00 | 0.16 | 3.52 | 0.20 | 9.74 | 0.00 |
| otter-1280x800 | 0.01 | 14.56 | 0.00 | 0.05 | 2.21 | 0.12 | 16.96 | 0.26 |

The picture columns give identical frames out of 120 / largest RGBA8 channel error / largest changed-pixel count per frame / sum of changed pixels over the orbit. Counts include alpha. Diff images use the frame with the largest channel error.

| Scene and raster | Guard vs original | Guard + scissors vs original |
|---|---|---|
| ball-960x720 | 120 / 0 / 0 / 0 | 120 / 0 / 0 / 0 |
| base-960x720 | 120 / 0 / 0 / 0 | 120 / 0 / 0 / 0 |
| walrus-960x720 | [84 / 107 / 4 / 48](orbit/revised/revised-walrus-960x720.png) | [2 / 113 / 7 / 349](orbit/scissor/scissor-walrus-960x720.png) |
| heron-960x720 | 120 / 0 / 0 / 0 | 120 / 0 / 0 / 0 |
| otter-960x720 | 120 / 0 / 0 / 0 | [117 / 48 / 1 / 3](orbit/scissor/scissor-otter-960x720.png) |
| walrus-1280x800 | [78 / 106 / 3 / 52](orbit/revised/revised-walrus-1280x800.png) | [2 / 192 / 8 / 439](orbit/scissor/scissor-walrus-1280x800.png) |
| heron-1280x800 | 120 / 0 / 0 / 0 | 120 / 0 / 0 / 0 |
| otter-1280x800 | 120 / 0 / 0 / 0 | [117 / 59 / 1 / 3](orbit/scissor/scissor-otter-1280x800.png) |

[All adjacent guard profiles](orbit/pre-scissor) · [Combined profiles, per-frame hashes and diffs](orbit/scissor)

The grouped 960 frame p95 forecast missed by −1.17 ms on walrus, −0.65 ms on heron and −2.04 ms on final otter. At 1280 the corresponding errors were −0.22, −2.66 and −4.04 ms. Only the larger walrus and final otter passed both load brackets in the selected immediate character runs. On that quiet final otter pair the guard measured 36.57 / 44.79 ms and the eight-row candidate 14.36 / 16.96 ms. The candidate changed one pixel on each of three frames. The stock walrus changed 349 and 439 pixels in total across the two captures. With original and modified programs using the same deterministic cull, all 480 walrus frame pairs (guard and grouped scissors, both resolutions) were byte-identical, including PNG encoding. [Fixed-order image comparisons](orbit/ordered-original/byte-identical-pngs.json.gz)

At the replay’s midpoint the screen-space box estimate used double-precision projection of the endpoint balls’ enclosing axis-aligned boxes. The native grouped implementation instead used a perspective denominator bound on each endpoint ball plus a two-pixel margin. Their targets and geometric containment agree; their exact launched-fragment counts can differ. The production frame times and RGBA comparisons above use the native projection.

### Preserving the culler's order

At `affc82f8` the native moving-camera path bins rows into disjoint 32×32-pixel hardware scissors. One GPU dispatch projects endpoint hulls in compacted slot order. A second dispatch counts and copies the intersecting rows into each tile. Its 64 lanes own contiguous slices of the original list; a prefix sum assigns each lane's output start. This preserves the exact original draw order in each tile, including holes from the ordered culler. Every draw uses the original triangles, ray interpolation and analytic solver. The indirect first vertex selects a tile's list. The first instance is zero on every adapter. The stock cull and binning share the Cull timestamp. The native profiler reports elapsed time from the previous pass end, so the pass budget adds to the reported frame. [GPU projection and stable gathering](../../crates/loft-passes/src/bundle/segment_tiles.wgsl) · [Native draw preparation and memory limit](../../crates/loft-passes/src/bundle/segment_tiles.rs)

For final otter the tile row lists occupy 10.49 MB at 960×720 and 15.20 MB at 1280×800 (decimal MB across Bit and Strand). There are 690 or 1,000 tiles per kind. Two kinds issue 1,380 or 2,000 indirect draws, including tiles with zero instances. The allocator limits each list to 64 MiB and all lists together to 128 MiB. Larger inputs use the original indirect draw for each kind that does not fit. Fixed-eye, editable, custom-culler and serialized programs retain their existing draw paths. The grouped prototype’s host projection and direct-draw path live in its recorded commit. The current native path fits the conservative screen rectangles entirely on the GPU.

### Contiguous warm-device control

The native host warmed each device with 30 scene frames before the first load control. It then measured a single block covering the full orbit with a deferred timestamp ring, repeating up to twice at the same camera times if either bracket failed. The original and guard programs came from the preserved compiled IR. The combined program was compiled from its source with the endpoint metadata available to the native host. All 120 Vis timestamps in every attempt were positive. Each cell gives the first quiet attempt or the first flagged attempt, in GPU p50 / p95 ms.

| Character and resolution | Original | Guard | Guard + scissors | Combined before / after control ms | Commands per frame, guard → combined |
|---|---:|---:|---:|---:|---|
| walrus-960x720 | 34.53 / 37.18 Q | 23.82 / 28.58 C | 19.82 / 24.27 Q | 0.89 / 0.72 | 21 → 160 |
| heron-960x720 | 40.56 / 48.06 Q | 5.31 / 7.27 Q | 5.05 / 7.14 Q | 0.73 / 1.08 | 23 → 23 |
| otter-960x720 | 127.36 / 138.44 Q | 64.25 / 71.26 Q | 26.51 / 30.77 C | 1.24 / 1.19 | 23 → 497 |
| walrus-1280x800 | 58.63 / 65.79 C | 33.85 / 39.21 C | 26.63 / 34.38 C | 1.20 / 2.81 | 21 → 160 |
| heron-1280x800 | 70.44 / 75.18 C | 3.99 / 8.91 C | 7.00 / 11.56 C | 2.92 / 3.38 | 23 → 23 |
| otter-1280x800 | 163.85 / 176.89 C | 72.91 / 82.50 C | 31.13 / 76.15 Q | 1.03 / 1.03 | 23 → 497 |

The following ordinary host submissions were measured after all GPU timing, with the previous frame drained before starting the CPU clock. Each sample includes native frame preparation, command encoding and queue submission. Values are median / p95 in ms across 30 calls.

| Scene | Guard CPU | Combined CPU | Difference in medians |
|---|---:|---:|---:|
| walrus-960x720 | 0.92 / 1.61 | 1.04 / 1.39 | +0.12 |
| heron-960x720 | 0.87 / 1.64 | 0.83 / 2.35 | -0.04 |
| otter-960x720 | 0.89 / 1.23 | 1.58 / 6.72 | +0.69 |
| walrus-1280x800 | 0.87 / 1.67 | 1.08 / 1.31 | +0.22 |
| heron-1280x800 | 0.89 / 2.49 | 0.93 / 2.34 | +0.04 |
| otter-1280x800 | 0.92 / 1.21 | 1.51 / 1.92 | +0.58 |

The native AB ring uses a separate measurement protocol: an immediate 120-frame pass followed by 30 warmups and a further ring-timed 120-frame pass. Its brackets and its original/guard comparisons are retained independently.

| Character and resolution | Original AB ring | Guard AB ring | Combined AB ring | Combined before / after control ms |
|---|---:|---:|---:|---|
| walrus-960x720 | 26.20 / 33.11 Q | 16.30 / 24.27 Q | 10.15 / 12.16 Q | 0.46 / 0.46 |
| heron-960x720 | 46.59 / 55.70 C | 2.02 / 3.04 C | 1.98 / 2.98 Q | 0.84 / 0.46 |
| otter-960x720 | 93.31 / 111.30 Q | 46.38 / 57.60 Q | 12.15 / 14.70 Q | 0.45 / 0.46 |
| walrus-1280x800 | 38.73 / 44.24 Q | 21.77 / 29.74 Q | 11.10 / 13.22 Q | 0.65 / 0.65 |
| heron-1280x800 | 46.11 / 49.27 Q | 2.66 / 3.77 C | 2.46 / 3.52 C | 1.23 / 0.66 |
| otter-1280x800 | 155.69 / 166.07 Q | 68.27 / 87.01 C | 12.80 / 15.58 Q | 0.66 / 0.65 |

[Every warm-device attempt](orbit/warm-device) · [Combined AB measurements](orbit/scissor-ab) · [Host instrument](../../crates/loft-cli/examples/fresh_blocks.rs)

The grouped host-cost forecast held: its median increment was 0.12–0.22 ms on walrus and 0.58–0.69 ms on final otter. The GPU-speed forecast did not transfer between quiet windows. For example, at 1280×800 the grouped final otter took 15.58 ms p95 in the AB ring and 76.15 ms p95 in the fully bracketed contiguous block. Both passed the same 1.10 ms control threshold, and the latter issued the expected 497 native commands. On walrus at 960 the corresponding quiet p95 values were 12.16 and 24.27 ms. A bracket measures its own time window. These runs do not establish sustained 60 fps across operating conditions. The separate timings remain in the report.

The grid-occupancy prediction held at all five sampled 960 poses. Rod occupied 100–109 of the 690 screen tiles and Strand 73–85. At the midpoint, the lists contained 5,085 Rod references and 6,081 Strand references. A load-time union over the full 360° orbit is much looser. Conservatively enclosing each endpoint's rotated path around the camera target kept 428/690 tiles for Rod and 508/690 for Strand, or 578/1,000 and 718/1,000 at 1280. The latter two Strand fractions met the predicted 75% ceiling at both sizes. All 240 sampled camera angles per kind fit the analytic all-angle bounds. A per-frame or short-angle active set has more room to cut empty indirect submissions. [Per-pose occupancy](orbit/tiles/occupancy.json.gz) · [All-angle occupancy](orbit/tiles/orbit-occupancy.json.gz)

### Field work remaining at the grouped build’s native p95 pose

Each measurement replays one guarded field at the camera time of the selected full-frame p95. The hit and miss columns count field evaluations in millions. The GPU draw column is the median of seven isolated single-kind draws in ms; the last column halves that duration after subtracting the measured empty pass. It is an optimistic scenario for cutting composite evaluation, since each draw also includes mandatory raster and march work. These isolated times came from a separate reservation.

| Scene and size | Field | Frame | Hit steps M | Miss steps M | Draw ms | Half variable draw ms |
|---|---|---:|---:|---:|---:|---:|
| [base-960x720](orbit/residual/base-960x720.json.gz) | Otter | 0 | 3.851 | 6.221 | 3.44 | 1.70 |
| [walrus-960x720](orbit/residual/walrus-960x720.json.gz) | Hide | 85 | 3.571 | 6.182 | 7.43 | 3.70 |
| [walrus-960x720](orbit/residual/walrus-960x720.json.gz) | Cap | 85 | 0.336 | 1.204 | 1.17 | 0.56 |
| [heron-960x720](orbit/residual/heron-960x720.json.gz) | HeronHead | 51 | 0.130 | 0.485 | 1.24 | 0.60 |
| [otter-960x720](orbit/residual/otter-960x720.json.gz) | Head4 | 63 | 1.471 | 3.004 | 4.57 | 2.26 |
| [otter-960x720](orbit/residual/otter-960x720.json.gz) | Torso | 63 | 2.304 | 4.326 | 8.00 | 3.98 |
| [walrus-1280x800](orbit/residual/walrus-1280x800.json.gz) | Hide | 42 | 4.182 | 7.374 | 9.18 | 4.56 |
| [walrus-1280x800](orbit/residual/walrus-1280x800.json.gz) | Cap | 42 | 0.412 | 1.385 | 1.05 | 0.50 |
| [heron-1280x800](orbit/residual/heron-1280x800.json.gz) | HeronHead | 76 | 0.172 | 0.630 | 1.19 | 0.57 |
| [otter-1280x800](orbit/residual/otter-1280x800.json.gz) | Head4 | 82 | 1.868 | 3.881 | 5.25 | 2.61 |
| [otter-1280x800](orbit/residual/otter-1280x800.json.gz) | Torso | 82 | 2.938 | 5.520 | 12.78 | 6.37 |

The predicted 960 limits were under 10 million surviving evaluations on HeronHead and under 60 million on the final head and torso together. The counts were 0.614 million and 11.105 million. Halving all variable work in these measured fields would save approximately 4.26 / 5.06 ms on walrus, 0.60 / 0.57 ms on heron and 6.24 / 8.97 ms on final otter at 960 / 1280. Pruning a subset of operands saves less, and transferring a draw rate across operating windows has already produced large errors.

### Preserving the original depth winner

The production tile candidate at `affc82f8` feeds the ordinary culler’s output to a pair of GPU kernels. The first projects the two endpoint balls of each surviving exact segment. For an endpoint at clip coordinate X/W and radius r, its screen-x extent is bounded by r × (‖∇X‖ + |X/W| ‖∇W‖) / (W − r‖∇W‖), with the corresponding bound for y. An unsafe denominator admits the whole viewport. The rectangle enclosing both projected balls also encloses their convex hull while W is positive. The raster expands the rectangle by two pixels after inflating the radius for numerical error.

The second kernel scans the original compacted slots in each 32×32 screen tile and writes an order-preserving subsequence. The renderer draws the original box triangles under disjoint hardware scissors. Every pixel belongs to exactly one tile and sees the original row order among the surviving candidates. Loft’s analytic intersection, depth comparison and interpolated ray still decide the winner. Equal-depth winners can differ between independent executions of the stock atomic culler. The fixed-order controls compare geometry and shading with that variable held constant. This approach combines the inherited projected-primitive idea with Loft’s existing row ordering, original raster geometry and Vis64 representation. It adds no authored type or query.

The native primary beauty path enables the pass for immutable stock exact-segment calls under the stock cull schedule and an orbiting camera. The other passes keep their ordinary draws. The gather scans each compacted slot twice per tile and reserves four bytes per tile per possible row. Strand’s 3,729 rows reserve 9.82 MiB at 960 and 14.22 MiB at 1280. A 64 MiB per-kind and 128 MiB scene limit sends oversized cases through their original draws. The portable serialized program also has the original draw path. [Binning shader](../../crates/loft-passes/src/bundle/segment_tiles.wgsl) · [Native lowering and storage limits](../../crates/loft-passes/src/bundle/segment_tiles.rs)

The original final-otter capture repeated exactly on all 240 frames across both sizes. The grouped build changed one pixel on three frames at each size. With the same deterministic culler in both builds, all 240 otter frames and all 240 walrus frames matched byte for byte. That control isolates draw-order sensitivity in the earlier grouped experiment. [Original repeat](orbit/repeat-original) · [Ordered grouped-scissor RGBA](orbit/ordered-scissor)

### Stable screen tiles on the moving camera

Each entry is GPU p50 / p95 in ms. In the left comparison original and guard ran consecutively. In the right comparison the same guard binary ran immediately before the combined build. The forecast column was frozen before the tile build and applies to the combined build. Original and guard forecasts are in the first table.

| Scene and raster | Original | Guard | Adjacent guard | Predicted with both | Measured with both |
|---|---:|---:|---:|---:|---:|
| ball-960x720 | 4.36 / 6.13 C | 2.49 / 4.45 Q | 1.15 / 1.23 Q | 2.5 / 4.5 | 1.17 / 2.48 Q |
| base-960x720 | 9.90 / 10.76 C | 7.14 / 9.61 Q | 5.09 / 7.73 Q | 5.5 / 8.5 | 3.25 / 3.61 Q |
| walrus-960x720 | 21.59 / 23.32 C | 11.48 / 13.16 C | 25.92 / 30.53 Q | 11 / 15.5 | 11.74 / 16.40 Q |
| heron-960x720 | 25.95 / 31.20 C | 7.98 / 9.30 C | 3.95 / 8.37 C | 6 / 9 | 2.32 / 4.28 Q |
| otter-960x720 | 57.82 / 62.82 C | 26.14 / 33.85 C | 47.94 / 61.67 Q | 16 / 22 | 25.49 / 29.71 Q |
| walrus-1280x800 | 26.12 / 26.97 C | 13.13 / 14.52 C | 21.88 / 36.08 Q | 14 / 19 | 15.69 / 21.05 Q |
| heron-1280x800 | 38.42 / 40.90 C | 10.23 / 12.40 C | 2.89 / 3.20 Q | 6 / 10 | 3.02 / 6.88 Q |
| otter-1280x800 | 78.36 / 85.80 C | 31.64 / 35.85 C | 52.93 / 63.28 C | 19 / 25 | 14.14 / 18.47 C |

All six passes in these budgets come from the same actual combined-build p95-ranked frame, in GPU milliseconds. The last column gives the extra time above 16.7 ms at that rank.

| Scene and raster | Cull | Vis | Shadow | Classify | Shade | Present | Frame | Excess |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| ball-960x720 | 0.00 | 1.05 | 0.00 | 0.18 | 0.87 | 0.38 | 2.48 | 0.00 |
| base-960x720 | 0.03 | 2.64 | 0.00 | 0.03 | 0.84 | 0.06 | 3.61 | 0.00 |
| walrus-960x720 | 0.08 | 11.38 | 0.00 | 0.09 | 4.60 | 0.24 | 16.40 | 0.00 |
| heron-960x720 | 0.00 | 2.63 | 0.00 | 0.17 | 1.25 | 0.22 | 4.28 | 0.00 |
| otter-960x720 | 0.46 | 22.55 | 0.00 | 0.20 | 6.31 | 0.18 | 29.71 | 13.01 |
| walrus-1280x800 | 0.09 | 14.89 | 0.00 | 0.15 | 5.56 | 0.36 | 21.05 | 4.35 |
| heron-1280x800 | 0.01 | 1.99 | 0.00 | 0.19 | 4.37 | 0.33 | 6.88 | 0.00 |
| otter-1280x800 | 0.24 | 15.52 | 0.00 | 0.06 | 2.43 | 0.21 | 18.47 | 1.77 |

The picture columns give identical frames out of 120 / largest RGBA8 channel error / largest changed-pixel count per frame / sum of changed pixels over the orbit. Counts include alpha. Diff images use the frame with the largest channel error.

| Scene and raster | Guard vs original | Guard + tiles vs original |
|---|---|---|
| ball-960x720 | 120 / 0 / 0 / 0 | 120 / 0 / 0 / 0 |
| base-960x720 | 120 / 0 / 0 / 0 | 120 / 0 / 0 / 0 |
| walrus-960x720 | [84 / 107 / 4 / 48](orbit/revised/revised-walrus-960x720.png) | [80 / 107 / 4 / 59](orbit/tiles/tiles-walrus-960x720.png) |
| heron-960x720 | 120 / 0 / 0 / 0 | 120 / 0 / 0 / 0 |
| otter-960x720 | 120 / 0 / 0 / 0 | 120 / 0 / 0 / 0 |
| walrus-1280x800 | [78 / 106 / 3 / 52](orbit/revised/revised-walrus-1280x800.png) | [76 / 109 / 3 / 57](orbit/tiles/tiles-walrus-1280x800.png) |
| heron-1280x800 | 120 / 0 / 0 / 0 | 120 / 0 / 0 / 0 |
| otter-1280x800 | 120 / 0 / 0 / 0 | 120 / 0 / 0 / 0 |

[All adjacent guard profiles](orbit/pre-tiles) · [Combined profiles, per-frame hashes and diffs](orbit/tiles)

### Change 3 decision

I deferred composite operand pruning after scoring changes 1 and 2 at `3118c4cc`. For the five original 960 views, halving the arithmetic of sufficiently large fields had forecast 0.0 / 5.6 / 7.8 / 16.9 / 34.2 ms of saving for ball / starter / walrus / heron / final otter. Much of the same work disappeared when change 1 rejected misses. On the stable build’s 960 p95 poses, even halving **all** variable isolated draw work in those guarded fields gives 0 / 1.24 / 9.25 / 0.46 / 5.77 ms. At 1280 the corresponding character figures are 3.32 / 0.58 / 9.92 ms. The replay and the full native profiles were taken in different operating windows. These conditional savings cannot be subtracted directly from a separately timed frame as a measured gain.

A prototype needs a spatial subdivision of the guarded region, interval certificates for operand dominance in each cell, and lazy evaluation of the surviving field expression. For Loft’s compact polynomial smooth minimum, an operand that is farther away than the blend support cannot affect the result. The comparison needs a float margin, the original equal-value choice, and the exact lower or upper blend rule. The remaining operations must preserve expression order. An initial implementation for smooth min/max trees would take an estimated one to two engineering weeks. It needs original-RGBA and field-value comparisons at cell boundaries; execution divergence and per-cell storage can offset the arithmetic saving. This is the operand-pruning direction developed in [Zanni’s sparse SDF evaluation](https://arxiv.org/abs/2304.09673).

On the slowest quiet final-otter tail frames, primary visibility consumed 22.55 ms out of 29.71 at 960, and 28.57 ms out of 36.90 at 1280. Holding the other passes fixed leaves 9.54 and 8.37 ms for Vis: reductions of 58% and 71%. At 1280 the slowest quiet walrus frame leaves 5.79 ms for a 19.34 ms Vis pass, also a 70% reduction. Its shade and classification passes alone took 10.32 ms on that frame. Operand savings alone at the measured replay rates fall short. Exact submission of fewer thin-row draws, cheaper surviving field evaluations and the non-Vis tail each have separate budgets. The original halved-fields forecast for heron has particularly little work left after the guard.

### Final native controls and field work

Each linked row covers 120 complete moving frames per attempt. Times are GPU p50 / p95 in ms. Immediate is the ordinary CLI. AB immediate and AB ring share a device and have independent brackets. The contiguous warm-device block first warmed the device, then bracketed the complete orbit. Q requires both measured load controls ≤1.10 ms. C retains the first attempt when all attempts of that type were flagged. The final column is the number of images that match the ordinary original capture out of 120.

| Character and raster | Immediate | AB immediate | AB ring | Warm block | Exact frames |
|---|---:|---:|---:|---:|---:|
| walrus-960x720 | 11.74 / 16.40 Q | 9.28 / 13.30 Q | 8.99 / 13.95 Q | 9.14 / 14.47 Q | 80 |
| heron-960x720 | 2.32 / 4.28 Q | 2.11 / 4.81 Q | 1.98 / 4.58 Q | 1.92 / 2.10 Q | 120 |
| otter-960x720 | 25.49 / 29.71 Q | 12.58 / 16.94 C | 12.38 / 17.64 Q | 12.76 / 17.46 Q | 120 |
| walrus-1280x800 | 15.69 / 21.05 Q | 14.79 / 20.74 C | 16.13 / 30.25 Q | 18.09 / 26.77 Q | 76 |
| heron-1280x800 | 3.02 / 6.88 Q | 2.81 / 6.15 Q | 3.19 / 7.16 Q | 2.75 / 3.93 Q | 120 |
| otter-1280x800 | 14.14 / 18.47 C | 24.50 / 36.90 Q | 19.53 / 34.15 Q | 21.97 / 35.42 Q | 120 |

The following range includes every quiet attempt across those four methods. The pass split belongs to the slowest quiet p95-ranked frame. Its gap is the extra work that would have to be removed to reach 16.7 ms in that measured window. A blank range means no attempt passed both brackets.

| Character and raster | Quiet / attempted | Quiet p95 range ms | Slowest method | Cull / Vis / other ms | Gap ms |
|---|---:|---:|---|---:|---:|
| walrus-960x720 | 4/4 | 13.30–16.40 | native | 0.08 / 11.38 / 4.94 | 0.00 |
| heron-960x720 | 4/4 | 2.10–4.81 | immediate | 0.00 / 1.89 / 2.91 | 0.00 |
| otter-960x720 | 3/7 | 17.46–29.71 | native | 0.46 / 22.55 / 6.69 | 13.01 |
| walrus-1280x800 | 3/5 | 21.05–30.25 | ring | 0.13 / 19.34 / 10.77 | 13.55 |
| heron-1280x800 | 5/6 | 3.93–7.16 | ring | 0.02 / 3.55 / 3.59 | 0.00 |
| otter-1280x800 | 3/7 | 34.15–36.90 | immediate | 0.35 / 28.57 / 7.97 | 20.20 |

Host submission covers 30 ordinary frames, after all GPU timing, draining the previous frame before each sample. It includes encoding and queue submission. These times run on the CPU.

| Scene | Commands, guard → tiles | Guard CPU p50 | Tiles CPU p50 / p95 | Median change | Warm-block control before / after ms |
|---|---:|---:|---:|---:|---:|
| walrus-960x720 | 21 → 712 | 0.92 | 1.27 / 1.74 | +0.35 | 0.46 / 0.45 |
| heron-960x720 | 23 → 23 | 0.87 | 0.66 / 1.15 | -0.21 | 0.45 / 0.46 |
| otter-960x720 | 23 → 1405 | 0.89 | 1.98 / 2.51 | +1.09 | 0.45 / 0.46 |
| walrus-1280x800 | 21 → 1022 | 0.87 | 1.62 / 1.98 | +0.76 | 0.90 / 1.03 |
| heron-1280x800 | 23 → 23 | 0.89 | 0.86 / 1.04 | -0.03 | 0.72 / 0.91 |
| otter-1280x800 | 23 → 2025 | 0.92 | 2.47 / 17.91 | +1.54 | 0.75 / 1.03 |

[Raw immediate profiles](orbit/tiles) · [AB timing](orbit/tiles-ab) · [Every warm-device attempt](orbit/warm-device)

At the actual p95-ranked pose of the ordinary combined run, each isolated guard draw ran seven times and the counter split field evaluations by whether the ray hit. The last column is half of each draw’s variable time above the independently measured empty pass, summed over listed fields. This is an optimistic estimate for operand pruning.

| Scene | P95 frame index | Fields | Hit / miss evaluations M | Half variable draw ms |
|---|---:|---|---:|---:|
| [base-960x720](orbit/residual-tiles/base-960x720.json.gz) | 80 | Otter | 3.879 / 6.287 | 1.24 |
| [walrus-960x720](orbit/residual-tiles/walrus-960x720.json.gz) | 7 | Hide, Cap | 3.671 / 7.277 | 9.25 |
| [heron-960x720](orbit/residual-tiles/heron-960x720.json.gz) | 98 | HeronHead | 0.136 / 0.542 | 0.46 |
| [otter-960x720](orbit/residual-tiles/otter-960x720.json.gz) | 52 | Head4, Torso | 3.693 / 7.159 | 5.77 |
| [walrus-1280x800](orbit/residual-tiles/walrus-1280x800.json.gz) | 90 | Hide, Cap | 4.857 / 9.231 | 3.32 |
| [heron-1280x800](orbit/residual-tiles/heron-1280x800.json.gz) | 88 | HeronHead | 0.173 / 0.650 | 0.58 |
| [otter-1280x800](orbit/residual-tiles/otter-1280x800.json.gz) | 101 | Head4, Torso | 4.919 / 9.728 | 9.92 |

The ordered-culler pairs isolate the effect of binning from the ordinary culler’s choice of equal-depth winner. The four score columns give exact frames, maximum channel error, largest changed-pixel count per frame and total changed pixels over the 120-frame clip.

| Scene | Original repeat | Ordered group of eight vs ordered original | Ordered tiles vs ordered original |
|---|---|---|---|
| walrus-960x720 | [78 / 110 / 4 / 60](orbit/repeat-original/repeat-original-walrus-960x720.png) | 120 / 0 / 0 / 0 | 120 / 0 / 0 / 0 |
| otter-960x720 | 120 / 0 / 0 / 0 | 120 / 0 / 0 / 0 | 120 / 0 / 0 / 0 |
| walrus-1280x800 | [83 / 106 / 4 / 50](orbit/repeat-original/repeat-original-walrus-1280x800.png) | 120 / 0 / 0 / 0 | 120 / 0 / 0 / 0 |
| otter-1280x800 | 120 / 0 / 0 / 0 | 120 / 0 / 0 / 0 | 120 / 0 / 0 / 0 |

On walrus-960x720, 22 of the 40 stable frames that differed from the first original matched the second original exactly. The pair of original samples does not enumerate every possible tie winner.

On walrus-1280x800, 17 of the 44 stable frames that differed from the first original matched the second original exactly. The pair of original samples does not enumerate every possible tie winner.

Both fixed-order walrus references used the existing stock `cull_single` body for every kind. The stable candidate preserves all 120 RGBA outputs at each raster. Its native AB ring measured 8.99 / 12.90 ms versus the original 18.80 / 25.44 at 960, and 10.84 / 16.93 ms versus 26.30 / 34.09 at 1280. All four were quiet; the larger deterministic run exceeded the 16.7 ms target by 0.23 ms. The original ordinary source retains the atomic culler. The prewritten ±25% small-raster and 22 ms large-raster predictions for the ordered ring both held. [Ordered original timing](orbit/ordered-original-ab) · [Ordered combined timing](orbit/ordered-tiles-ab)
