# Step 3 · motion on the Rust path

## Before building · feat/design-motion at 5d94531

Use the unchanged numerical predictions and tolerances in the 21 rows of the [consolidated proposal](../../docs/proposals/2026-11-19_motion_architecture.md#phase-1-the-crab-lives-vertical-slice-one-implementer). First instrument the actual frame kernel list on the original lineup. Predict >2,452 of the 4,905 compute pipelines dispatch each frame. Count the named camera separately. One load, two small frames at t=1.37 s.

Record 481 GPU state rows at 120 Hz with the two velocity changes at ticks 24, 204 and the stop at 384. Compare all bytes under four delivery schedules. Measure final transforms by reconstructing the distal site from its joint frame and length. Introduce the 2 cm perturbation *before* measuring that site. Time the submitted compute pass with timestamp queries. Compare live frames against frozen poses with identical draws. Use the 64-tick bound for live and a whole 480-tick dispatch for baking. Report every miss, including unresolved timings. The owner's visual verdict is separate from the mechanical evidence.

## Integration seam

Motion owns per-instance input history, step scheduling and presented rows. Render can consume a table of 32-byte rigid frames (translation, xyzw quaternion) through its own kinds. Keep end kinds, IK and support arithmetic in editable WGSL. Coordinate additive bundle changes through this note before merging either lane.

The recorded lineup at this revision has 4,884 fill pipelines (4,880 frame, four load) and 21 named camera pipelines. A named view adds two camera dispatches. The profiler reports the length actually passed to the frame encoder. GPU timing at 96×64 is a count experiment and cannot replace the 960×540 baseline.
