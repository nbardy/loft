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

At 1280×800, frame 60 of the walrus differs between the screen-strip and guard builds at 1,140 pixels, with a largest channel error of 133. The strip already fails the picture requirement on this frame. It is an experimental timing result. The hardware-scissor build uses the old clip coordinates throughout. [Strip shader](strip.wgsl) · [Native scissor projection](../../crates/loft-native/src/scissor.rs)

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

### Hardware scissors on the moving camera

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
