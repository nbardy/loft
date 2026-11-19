# Step 3 · motion on the Rust path

## Before building · feat/design-motion at 5d94531

Use the unchanged numerical predictions and tolerances in the 21 rows of the [consolidated proposal](../../docs/proposals/2026-11-19_motion_architecture.md#phase-1-the-crab-lives-vertical-slice-one-implementer). First instrument the actual frame kernel list on the original lineup. Predict >2,452 of the 4,905 compute pipelines dispatch each frame. Count the named camera separately. One load, two small frames at t=1.37 s.

Record 481 GPU state rows at 120 Hz with the two velocity changes at ticks 24, 204 and the stop at 384. Compare all bytes under four delivery schedules. Measure final transforms by reconstructing the distal site from its joint frame and length. Introduce the 2 cm perturbation *before* measuring that site. Time the submitted compute pass with timestamp queries. Compare live frames against frozen poses with identical draws. Use the 64-tick bound for live and a whole 480-tick dispatch for baking. Report every miss, including unresolved timings. The owner's visual verdict is separate from the mechanical evidence.

## Integration seam

Motion owns per-instance input history, step scheduling and presented rows. Render can consume a table of 32-byte rigid frames (translation, xyzw quaternion) through its own kinds. Keep end kinds, IK and support arithmetic in editable WGSL. Coordinate additive bundle changes through this note before merging either lane.

The recorded lineup at this revision has 4,884 fill pipelines (4,880 frame, four load) and 21 named camera pipelines. A named view adds two camera dispatches. The profiler reports the length actually passed to the frame encoder. GPU timing at 96×64 is a count experiment and cannot replace the 960×540 baseline.

Measured by the submitted frame list on Apple M4 Metal: **4,880 frame dispatches**, two camera dispatches, 5,539 modules, 4,905 compute and 654 render pipelines. At 96×64 the two timed frames took 29.48 and 30.03 ms, including 13.68 and 13.80 ms in the kernel pass. Cold load on this shared machine was 154.0 s. The majority prediction passes. Dividing 97 ms by pipeline count does not measure dispatch overhead: the kernel pass here cost about 2.8 µs per submitted kernel, including its arithmetic.

## Wire and host ownership

Wire 5 adds one row-local step declaration. It names an immutable plan table, initial state table, exclusive output pack, input and WGSL module. Values cross the island boundary. The step and presentation law cannot bind storage. Bundle V1 readers in this branch accept previous bundles without the new records. A script retains each tick, and a level retains every 60th tick; the same advance entry handles both. A presentation reads a script checkpoint or fuses up to 64 live ticks. Camera, other frame fills and draws consume the result in the same encoder.

## First score and revised cost model

The first correct GPU trace produced the predicted 22 releases. All 481 states were bit-identical at all four cadences after fixing the baked-to-live checkpoint selection. Reconstructing the local distal site exposed a 5.3 mm error in the original near-vertical quaternion; choosing its better-conditioned hemisphere reduced that to 0.00082 mm. Row-local displacement before the strain comparison preserves the symmetric tie.

The first instrumented 480-tick bake took 14.31 ms. Two-tick p95 was 0.085 ms for one and 0.088 ms for 256, then 0.103 ms for 1,000. These results contradict the proposal's idle-lane and bake cost model. A tick was sorting/solving its support twice, even when the set of planted anchors was unchanged. Geometry changes only at lift or touch, at most 44 of the 480 ticks. Cache the hull order, centroid and inverse covariance when anchors change. Keep the general constrained solve for a negative share. Predict unchanged lift ticks and observed holds, a 6–10 ms bake and less than 0.065 ms p95 for the solo and 256-row spans. These are revised predictions; the original gates still apply.

Caching reduced the bake to **6.80 ms** and left all 22 lift ticks and plant measurements unchanged. The solo median fell from 0.073 to 0.048 ms. Its p95 remained 0.073 ms; 256 reached 0.089 ms. The cold-to-warm change and the tail during actual contact changes both matter. The proposal's 5 ms bake gate remains open. Use interleaved, matching recorded poses for the final whole-frame comparison.

## Phase 2 predictions before the run

A pin is a point in deck coordinates. Present its joint frames through a rigid deck transform, then transform the observed distal site back into the deck frame for the signed margin. A deck translating 0.5 m/s for 120 ticks should move every planted site 0.500 ± 0.001 m in world coordinates while its deck-frame residual stays below 0.1 mm. For four pads at (±0.30, ±0.20) and pressure point (0.15, 0), linear load constraints have extrema on simplices of at most three pads. Enumerating those simplices should give [0.25, 0.50] on the positive-x positive-z pad, with least-norm share 0.375.

The extra fractional join instrument presents the largest representable f32 time below each tick and the tick itself. Extrapolating the previous acceleration and aim missed by 0.206 mm at the root and 8.150 mm at a distal site. The selected tick owns a span. Use its saved start velocity, selected acceleration and two attitude endpoints. Retain the prior aim while a swing retargets and its prior phase for the last partial tick. Expect less than 0.01 mm at all 480 joins without changing any lift decision.

Measured at all 480 joins, the new span reduced the left-limit differences to 0.00054 mm at the root and 0.0061 mm at the distal sites. The integer decisions still give 22 releases. The test uses the preceding representable f32 timestamp, with multiplication in f64 to distinguish it from the exact tick.
