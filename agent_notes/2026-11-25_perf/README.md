# Why the character views are slow

Apple M4, Metal, macOS 15.5. The original fixtures are pinned beside this report. Baseline revision `2afc5e55`. The two CLI builds used the checkout's lab profile. All normal profiles use authored source, 30 warmup frames and 120 timed frames with immediate GPU timestamps. The image test hashes the decoded RGBA8 bytes of frames 0 through 119 at 60 Hz, including the first draw. The GPU pass numbers exclude CPU scene compilation, GPU queue waiting on the host, PNG encoding and display synchronization.

## Reading the diagnostics

The visibility shader rasterizes the back faces of every frustum-visible row's world-space AABB. The interpolated world position determines the ray direction. A hit writes both the row/distance pair and fragment depth. An analytic row runs its analytic query once per proxy fragment. A marched row evaluates its entire field, advances by distance divided by its row Lipschitz bound and stops on a hit, leaving its box or exhausting 192 evaluations. Replacing the box itself can change the interpolated ray, march start and sun projection. A mathematically better surface does not establish equal rendered bytes.

The stock step diagnostic colors the *winning* row. The proxy diagnostic clamps at 16 layers. See [the five original AOV rows](diagnostics.webp). The independent instrument inserts sharded counters directly into each fixture's generated fragment program and records fragments, geometrical hits, field evaluations on hits, field evaluations on misses and cap exhaustion. It counts before the hardware depth comparison. It renders each kind's actual rows for the counts and draws all kinds to count final visible pixels. Camera math in this independent instrument is rounded to f32 after projection assembly. That accounts for approximately pixel-scale differences from native masks. Its atomic shaders supply counts only. Rates come from the uninstrumented programs.

## Work model

For kind i, let P be the sum of covered proxy fragments across rows, H the number of rays returning a hit, S+ and S− the field evaluations on hitting and missing rays, and Z the visible pixels carrying the kind's ID. A traced field's work is (S+ + S−) times the cost of its full expression. A blended pair still pays for both operands at every evaluation. The study counts expanded WGSL length, distance and square-root calls, including loop trip counts, as one indicator of expression complexity. That is a source operation count. Compiler optimizations, divergent lanes and register pressure change the actual instructions and throughput.

The visibility estimate is E + Σ aᵢ Pᵢ over analytic kinds + Σ bᵢ (S+ᵢ + S−ᵢ) over traced kinds. E is the measured clear/store time. Rates aᵢ and bᵢ absorb vertex, row-load and SIMD costs at the calibration view. The calibration rotates the eye seven degrees, pulls it back 10%, and renders at 672×504. Each isolated calibration draw is repeated four times. Its rate is (median duration − E) / (4 × work per draw). The first estimate transfers these rates directly. The second multiplies the variable cost by the ratio of native calibration Vis to estimated calibration Vis after subtracting E from both. The third fits one shared multiplier per query class on the calibration views. The target view is excluded from all rate fits. The revision to the calibration procedure followed the first observed misses in starter otter and walrus. The original and revised forecasts are preserved in [the prediction record](predictions.md).

For shading, each occupied 8×8 kind tile tests 64 lanes. Each surviving Z pixel in these shaders requests four field samples for its normal, five for AO, one material and one integrator. The integrator evaluates the BSDF stack 14 times for ball, starter and heron, 15 for walrus and final otter, and twice in the sun-only control. Background presentation runs at output resolution. An idealized fresh-view estimate charges one query at the calibrated kind rate per winning pixel, one attachment clear/store and the measured time outside Vis. It is an optimistic work target. It presupposes a way to find and accurately intersect the winning field. Reusing visibility on an unchanged view has no such search cost.

## What was built

The native renderer can now retain the Vis64 color and D32 depth across frames. It verifies the stock culler and its resource bindings, checks reachable stages for time/sample dependence and mutable resources, compares uniforms, and invalidates on geometry-relevant live parameter changes. Geometry dispatch, bakes, orbits, editable loads, unrecognized cullers and host-owned command encoders take the original route. The cull, classify, shade and present passes still run. This extends Loft's existing shadow retention to eligible primary visibility. No scene or public language change is required. It is a native optimization. Raw runner hosts and reserialized shader text may still redraw.

Two *fresh-view* experiments use the original executable. One adds a rejection sphere ahead of the existing capsule and round-cone solvers in forked Loft source. The other uses hand-sized inner boxes for four poses, retains the original raster geometry and march entry, and limits miss traversal. The latter duplicates each selected field in an authored visible query. These are ablations. The inner boxes do not constitute a compiler proof for all poses. In particular, a generic rejection proof must cover the marcher's distance-dependent hit tolerance as well as the zero surface.

## Authoring a fast character

1. Put independent, spatially separated pieces in separate kinds. Keep pieces that need a continuous smooth join together.
2. Give spheres, rods and capsules their analytic visible query. Verify the *derived* proxy still fits the geometry.
3. Inspect a row's bounds in the compiled IR in metres. A face-sized field can acquire a full-screen box.
4. Check the field's derived Lipschitz bound. A good bound and a good box address different costs.
5. Move detail that changes only color or roughness into the material.
6. Inspect overdraw and step views together. A low winning step count can coexist with millions of long misses.
7. Profile the actual moving pose or camera when motion is the budget. Preserve the original lossless frames for comparison.
