# Why the character views are slow

Apple M4, Metal, macOS 15.5. The original fixtures are pinned beside this report. Baseline revision `2afc5e55`. The two CLI builds used the checkout's lab profile. All normal profiles use authored source, 30 warmup frames and 120 timed frames with immediate GPU timestamps. The image test hashes the decoded RGBA8 bytes of frames 0 through 119 at 60 Hz, including the first draw. The GPU pass numbers exclude CPU scene compilation, GPU queue waiting on the host, PNG encoding and display synchronization.

The production change retains primary visibility for an unchanged camera and geometry. The native renderer matched the original byte for byte in 720/720 corresponding 960×720 frames. At 1280×800, heron and final otter matched in 120/120 each. The walrus matched in 119/120. On the remaining frame one pixel differed by a maximum of 24 in RGBA8, and the candidate kept a value the original had produced on 119 frames. [Timing and model tables](MEASUREMENTS.md) · [Raw measurement archive](measurements.tar.gz) · [Full-frame difference](diffs/candidate-1280x800-walrus-diff.png) · [Original, retained and amplified difference at the affected pixel](diffs/candidate-1280x800-walrus-original-candidate-diff.png).

The baseline walrus itself varies at (700, 407) in the 1280×800 image. Its original 120-frame reference produced (205, 164, 96, 255) on 119 frames and (181, 141, 87, 255) once. A second original-only clip reached 53 complete frames before the job cap. Five of those 53 differed from the first run at the same timestamp and every frame was one of the original two images. Reversing the row order in the independent visibility replay changed nine Rod IDs elsewhere. Its approximate camera chose Rod 1101 at (700, 407) in both orders, so the precise resolving stage for that native pixel remains unconfirmed.

## Reading the diagnostics

The visibility shader rasterizes the back faces of every frustum-visible row's world-space AABB. The interpolated world position determines the ray direction. A hit writes both the row/distance pair and fragment depth. An analytic row runs its analytic query once per proxy fragment. A marched row evaluates its entire field, advances by distance divided by its row Lipschitz bound and stops on a hit, leaving its box or exhausting 192 evaluations. Replacing the box itself can change the interpolated ray, march start and sun projection. A mathematically better surface does not establish equal rendered bytes.

The stock step diagnostic colors the *winning* row. The proxy diagnostic clamps at 16 layers. See [the five original AOV rows](diagnostics.webp). The independent instrument inserts sharded counters directly into each fixture's generated fragment program and records fragments, geometrical hits, field evaluations on hits, field evaluations on misses and cap exhaustion. It counts before the hardware depth comparison. It renders each kind's actual rows for the counts and draws all kinds to count final visible pixels. Camera math in this independent instrument is rounded to f32 after projection assembly. That accounts for approximately pixel-scale differences from native masks. Its atomic shaders supply counts only. Rates come from the uninstrumented programs.

## Work model

For kind i, let P be the sum of covered proxy fragments across rows, H the number of rays returning a hit, S+ and S− the field evaluations on hitting and missing rays, and Z the visible pixels carrying the kind's ID. A traced field's work is (S+ + S−) times the cost of its full expression. A blended pair still pays for both operands at every evaluation. The study counts expanded WGSL length, distance and square-root calls, including loop trip counts, as one indicator of expression complexity. That is a source operation count. Compiler optimizations, divergent lanes and register pressure change the actual instructions and throughput.

The authored expression gives those counts concrete meaning. The starter Otter evaluates 13 ellipsoids, one rounded box, ten smooth minima and four smooth maxima per step. The final skull evaluates 19 ellipsoids, one rounded box and 19 smooth operators. Walrus Hide calls 26 ellipsoids, five capsules, two rounded boxes, two explicit lengths, 32 smooth minima and three smooth maxima. The heron head uses ten capsule calls and three lengths, plus one neck capsule in each of seven loop iterations. Lowering and GPU optimization can simplify constant expressions.

The visibility estimate is E + Σ aᵢ Pᵢ over analytic kinds + Σ bᵢ (S+ᵢ + S−ᵢ) over traced kinds. E is the measured clear/store time. Rates aᵢ and bᵢ absorb vertex, row-load and SIMD costs at the calibration view. The calibration rotates the eye seven degrees, pulls it back 10%, and renders at 672×504. Each isolated calibration draw is repeated four times. Its rate is (median duration − E) / (4 × work per draw). The first estimate transfers these rates directly. The second multiplies the variable cost by the ratio of native calibration Vis to estimated calibration Vis after subtracting E from both. The third fits one shared multiplier per query class on the calibration views. The target view is excluded from all rate fits. Those two revisions followed the first observed misses for starter otter and walrus. A further calibration measures the rotated camera at the target raster, 960×720, then transfers elapsed native time in proportion to the two cameras’ weighted counts. Each forecast is saved before timing its held-out camera. The equal-raster fit uses only rotated-camera native times. The original and revised forecasts are preserved in [the prediction record](predictions.md).

For shading, each occupied 8×8 kind tile tests 64 lanes. Each surviving Z pixel in these shaders requests four field samples for its normal, five for AO, one material and one integrator. The integrator evaluates the BSDF stack 14 times for ball, starter and heron, 15 for walrus and final otter, and twice in the sun-only control. Background presentation runs at output resolution. The optimistic fresh-view estimates charge one analytic query per winning analytic pixel, one or eight field evaluations per winning traced pixel, one attachment clear/store and the measured time outside Vis. It is an optimistic work target. It presupposes a way to find and accurately intersect the winning field. Reusing visibility on an unchanged view has no such search cost.

## What was built

The native renderer can now retain the Vis64 color and D32 depth across frames. It verifies the stock culler and its resource bindings, checks reachable stages for time/sample dependence and mutable resources, compares uniforms, and invalidates on geometry-relevant live parameter changes. Geometry dispatch, bakes, orbits, editable loads, unrecognized cullers and host-owned command encoders take the original route. The cull, classify, shade and present passes still run. This extends Loft's existing shadow retention to eligible primary visibility. No scene or public language change is required. It is a native optimization. Raw runner hosts and reserialized shader text may still redraw.

Two *fresh-view* experiments use the original executable. One adds a rejection sphere ahead of the existing capsule and round-cone solvers in forked Loft source. The other uses hand-sized inner boxes for four poses, retains the original raster geometry and march entry, and limits miss traversal. The latter duplicates each selected field in an authored visible query. The four original poses plus the additional head-and-torso variant each matched the original in all 120 corresponding 960×720 frames. In the same GPU slot, the heron went from 44.46 / 48.84 ms to 8.7 / 10.3 ms p50 / p95. The walrus went from 27.18 / 43.68 to 20.6 / 23.7 and the head-only final otter from 124.64 / 151.24 to 54.1 / 61.2. All were flagged contended. The inner boxes are specific to these poses. In particular, a generic rejection proof must cover the marcher's distance-dependent hit tolerance as well as the zero surface.

## Authoring a fast character

1. Put independent, spatially separated pieces in separate kinds. Keep pieces that need a continuous smooth join together.
2. Give spheres, rods and capsules their analytic visible query. Verify the *derived* proxy still fits the geometry.
3. Inspect a row's bounds in the compiled IR in metres. A face-sized field can acquire a full-screen box.
4. Check the field's derived Lipschitz bound. A good bound and a good box address different costs.
5. Move detail that changes only color or roughness into the material.
6. Inspect overdraw and step views together. A low winning step count can coexist with millions of long misses.
7. Profile the actual moving pose or camera when motion is the budget. Preserve the original lossless frames for comparison.

## Direct evidence of waste

The starter head's 128 m proof box covers all 691,200 pixels. Its 58,638 hits take 3.85 million field evaluations and its misses take 48.95 million. Sixty whiskers alone generate 8.25 million proxy fragments for 5,054 analytic hits. The head's derived L is 3.41. The final otter has 3,729 Strand rows and two more full-screen fields, Head4 and Torso, with 128 m wide boxes and L of 5.20 and 3.74.

HeronHead's box is 128 m on each axis although the model is smaller than a metre. L is already 1.05. Only 13,788 rays hit this head. They take 143,246 field evaluations. Its 677,412 misses take 59,148,879. The *entire visible heron scene* averages just 7.21 steps on visible traced pixels. A tighter step multiplier cannot by itself explain or repair the empty-space cost.

The walrus hide has a 512 m wide proof box and L of 1.00. The cap's box is 192 m wide. The two fields together use 50.16 million evaluations on misses. The 1,113 Rod rows run 32.19 million analytic queries and return 154,878 hits, roughly 208 invocations per geometrical hit. Costs multiply before shading sees a single rod.

The final otter generates 116.71 million primary proxy fragments. Strand accounts for 109.30 million of them and reports 8,247 geometric hits, about 13,250 fragment invocations per hit. Head4 spends 69.75 million field evaluations on misses and Torso another 48.68 million. The isolated calibration attributes roughly 40 ms of the original median frame to Torso, 26 ms to Strand and 25 ms to Head4. It takes work on all three to make a new view fast.

The evaluator starts its interval proof on a cube extending 1,024 m in each direction. Its breadth-first stopping condition uses the width of the surviving *candidate cells*. The six face searches choose a leaf size from that width divided by 64, with a 62.5 mm minimum. Interval dependency can leave cells far from the object alive. The resulting boxes can be valid and very loose. Even the minimum leaf is wide compared with millimetre fibres. The older artist ablations agree on direction. The heron fell from 340–370 ms to 53–73 ms across successive design iterations. Replacing the final otter's real head by a simplified three-part skull took a head-only tile from 51.3 to 13.4 ms. Those are changed assets and separate experiments.

The all-occluded calibration clears depth to zero before drawing exactly the same geometry. Relative to its ordinary depth test, the median four-draw duration was 1.02× for ball Pelt, 0.93× for starter Otter, 1.04× for its whiskers, 1.04× for HeronHead, 1.26× for walrus Hide and 1.21× for Rod. Earlier opaque occupancy buys very little in this particular programmable-depth pass on Metal. The experiment measures elapsed execution. It does not expose the chip's internal early-depth decisions.

The sun-only ball control also makes ambient occlusion dead in the compiled lighting computation. Holding AO live on the direct term puts the two-sample shader at 0.990 ms. The 14-sample original is 2.379 ms. With AO held constant at one, the 14-sample shader costs 2.156 ms against 0.653 ms for sun-only. The twelve sky directions thus account for 1.39–1.50 ms under these controls. Keeping AO live costs 0.22–0.34 ms. All three repetitions were flagged contended and the shade medians repeated within 0.01 ms for each control.

## Fixed-camera timings

These are the first native runs, with zero of 120 nonzero Vis intervals after the change. Every first pair below was flagged contended. Later controls and eligibility are recorded in the measurement archive. Numbers are GPU milliseconds, p50 / p95.

| View | Original 960×720 | Retained 960×720 | Retained 1280×800 |
|---|---:|---:|---:|
| Ball, sky | 4.08 / 5.66 | 2.68 / 3.55 | — |
| Ball, sun only | 2.27 / 2.53 | 0.94 / 1.85 | — |
| Starter otter | 15.84 / 33.66 | 3.92 / 4.79 | — |
| Walrus | 26.53 / 99.26 | 1.86 / 2.15 | 2.50 / 2.67 |
| Heron | 37.01 / 41.19 | 1.00 / 1.25 | 5.00 / 5.87 |
| Final otter | 100.02 / 148.47 | 1.99 / 2.09 | 2.84 / 10.83 |

The highest candidate p95 across the first three runs, covering all three characters, was 2.64 ms at 960×720 and 13.72 ms at 1280×800. The first control bracket was consistently about 1.94 ms at the smaller size and 2.81 ms at the larger size. The native contention threshold is 1.10 ms on **each** bracket. The one-triangle control of roughly 0.08 ms is a separate instrument.

## Engineering choices for a changing view

| Priority | Change | Conditional fresh Vis saving in ms, ball / starter / walrus / heron / otter | Build cost and risk |
|---|---|---|---|
| 1 | Derive a second, certified interval for primary-ray rejection. Preserve the old triangles, march entry, L and sun camera. | 0 / 9.4 / 11.1 / 29.8 / 55.4 if it removes 90% of giant-field miss work | Several engineering days for proof refinement, compact row storage and shader integration. Prove the full acceptance band including the distance-dependent tolerance. A merely smaller box can change edge hits. |
| 2 | Cull thin primitive rows against conservative screen tiles before invoking the fragment shader. Draw the original proxy under the tile scissor. | 0 / 2.9 / 4.9 / 0 / 26.0 if queries approach four per true hit plus 36 work units per row | Roughly one engineering week for tile lists, indirect submission, near-plane handling and ordering. Extra vertex work and equal-depth ownership are the main risks. |
| 3 | Prove operands of composite fields unable to affect a hard or smooth minimum over a cell, then eliminate their evaluations. | 0 / 5.6 / 7.8 / 16.9 / 34.2 if arithmetic halves in the expensive fields | One to two engineering weeks for the proof, generated shader variants and testing. Preserve smooth-blend support, expression order, rounding and numerical bounds. |
| 4 | Reuse the remaining shading and presentation on a fully invariant view | Up to the remaining measured 1–4 ms at 960×720 in these first runs. No saving on a changing material or camera. | Additional dependency analysis across light, material, sky, present and host targets. An untracked input produces stale color. |

The savings overlap, especially for composite pruning after empty-space rejection. For the final otter, even the first two design targets together leave about 18.6 ms of the original 100.0 ms median at 960×720. The remaining gap includes traced hits and the cost of reaching a surface on rays that survive the guard. A 16.7 ms p95 leaves less headroom than a 16.7 ms median. No production moving-view optimization in this lane certifies that target.

The original sphere marcher and analytic solvers, interval proof and retained shadow map come from Loft. This lane extended the existing shadow invalidation analysis to primary visibility. The guarded-interval and strand-sphere experiments were derived directly from the counts and the existing solver algebra. Tiled rejection and branch-and-bound operand pruning are established techniques. Retaining the original proxy interpolation and march entry is the constraint imposed by this renderer's image contract.

## Reproducing a number

The scene paths below are relative to the copied `fixtures/` directory. The two ball controls are `start/ball.loft` and `start/ball_sunonly.loft`. The four other views are `start/otter.loft`, `walrus/tq.loft`, `heron/h02_hero_q3.loft` and `otter/hero_q3.loft`. Both native binaries used the same resolved fixture tree for the main image comparison. The ablations used independent copies of that tree.

Use the baseline binary from `2afc5e55` and the candidate from `12c91988` or its comment-only follow-up. Each was built with `tools/dev.sh build loft-cli --fast`. For timing, run `loft profile SCENE --size 960x720 --frames 120 --json FILE` with the declared GPU slot. For a counter replay, save `loft ir SCENE` as `NAME.ir` and `loft show SCENE --json` as `NAME.show` in the model directory. The counter uses wgpu-py 0.33.0, NumPy 2.5.5 and Metal timestamps. [Counter](count_rays.py) · [Held-out camera calibration](same_size.py) · [Frame hashing](hash_frames.py) · [Pixel comparison](compare_pixels.py) · [Scoring](score.py).

The sphere guard can be applied to an isolated standard library with [the Loft source patch](strand-guard.patch). [The field guard generator](guard_study.py) creates the four pose-specific authored queries from the original IR bounds and field bodies. Its optional `torso` argument adds the torso guard to a copy of the head-guarded otter.
