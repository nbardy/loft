# C1d · Native input replay

Base `173d6e9a2b4fa36fcb45c0957f75e9c44f8e6dfa` from `origin/main`.

## Prediction before implementation

At 120 Hz the initial state is tick 0. There are four law iterations through
tick 4. With the edit from 1 to 2 at tick 0 the final scalar is 8
(`da03dcf104ced4c55c1de902903a431c0c613717d8e389a90c29f286ad9704d3`).
At tick 2, tick 1 contributes 1 and ticks 2–4 contribute 2 each, for 7
(`fe3426074932920e93d67e22dbd90a3cd19e3e447260e98bb4ae8a16b7ee9177`).
The [planner control](../../../out/meta/c1/resume.log) independently measured
both words. Both public commands should exit 0. Each dispatch loads once and
simulates four steps in either command. Truth reads ticks 0–4. Rendering
can defer readback to the final tick and batch the law steps between edits.

At tick 2, Param=3 followed by Level=2 leaves final x=7. A tick-4 edit
to 2 gives 5. No edits gives 4. A tick-2 value of 99 clamps to 4 and gives
13. In the recovering check, no edits give x=0, 1, 2, 3, 4 and the tick-2
miss exits 2 despite the matching final identity. Applying drive=0 at
tick 0 on a zero-length log also exits 2 and leaves x=0.

Unknown names, vector payloads and bundle mismatches exit 5. A final hash
mismatch exits 3. Events, pose packets and cross-device native logs exit 6.
A fresh binary and JSON recording should replay with the same state and exact
RGBA pixels. The 481-tick Scramble control at `scramble_speed=-0.3` should
retain its 32-byte state identity
`d953a59bb7d78610ef76609e8557b4af4bc07b7b72fd7e50b238e5b6d6efbffe`
and reproduce the archived release picture. Snapshot and timed controls should
reproduce baseline verdicts and pixels. A compatible expression-twin log
should verify its historical identity. Pinned bundle mismatches should exit 5.

The prior snapshot loop reloads each dispatch on every Truth tick. For
the no-edit four-tick case, that is five loads and ten simulated steps.
For the 481-tick control, it is 482 loads and 115,921 steps per active row.
The persistent path should reduce those counts to 1/4 and 1/481 respectively.

## Implementation

A native motion session attaches one runner and owns one loaded instance per
compiled dispatch. The log driver submits through tick (t-1) before a
tick-(t) edit. The existing scalar range check supplies the clamped value
for the picture and for the parameter slot resolved from each instance's
metadata. Truth advances and reads verdicts at each integer tick, including
zero. Rendering batches between edits and reads the final rows. Ordinary
snapshot and timed sampling use a fresh session with the same decoder.

## Measured

Apple M4 Metal, one scoring batch under `tools/gpu_lock.sh`. The lock was free
at acquisition. The before executable matched the byte hash published for
`33cdcbf`; its crates, libraries, engines and Cargo inputs match this lane's
base. The raw commands, exits, adapter signature, overrides, wall-clock
observations and comparisons are in the [score report](../../../out/meta/c1d/score.json).

| Four-tick input | Before check / render | After check / render | Final f32 |
| --- | --- | --- | --- |
| Tick-0 Param = 2 | 0 / 0 | 0 / 0 | 8 |
| Tick-2 Param = 2 | 6 / 6 | 0 / 0 | 7 |
| Tick-2 Param = 3, then Level = 2 | — | 0 / 0 | 7 |
| Tick-4 Param = 2 | — | 0 / 0 | 5 |
| No inputs | — | 0 / 0 | 4 |
| Tick-2 Param = 99 | — | 0 / 0 | 13 |

Both commands verified the independent four-byte BLAKE3 identities in the
prediction. The unit instrument recorded one loaded instance and four
simulated steps per active row in each four-tick replay and one instance
and 481 steps in the Scramble control. The tick-2 replay repeated with an
identical PNG and state hash. The fresh JSON log also round-tripped with
identical PNG bytes. [Unit instrumentation](../../../out/meta/c1d/cli-unit-tests.log)

Both commands returned 5 for unknown names, vector payloads and a different
bundle, 3 for a bad final state hash, and 6 for Event and PosePacket inputs.
Check returned 6 on cross-device and checkpoint requests. The recovering
fixture reached its expected final state of 4 and still exited 2 with its
tick-2 miss. A zero-length trace with a tick-zero edit also exited 2 at
tick zero. Both fixtures rendered successfully. Pinned Scramble and the
archived logs from earlier bundle identities returned 5. A freshly encoded
timed expression-twin log verified the historical state identity
`aee66a2dfb6c1707cb3b017d55b4dd0cd91894c6edd8c92f0c79dba276267d72`.

The 481-tick Scramble recording at `scramble_speed=-0.3`, binary replay and
Truth check all exited 0 with the predicted 32-byte state identity. Record,
replay, the base executable's fresh render and the archived release image
all produced the same PNG SHA-256
`9d0cb26137cb513260dda2f827e866bdfb9ba4c68e94d7995c0d46d4b4e7d34e`
and zero differing RGBA pixels at 1280×800.
[Binary log](../../../out/meta/c1d/scramble-native.loftlog) ·
[Replay PNG](../../../out/meta/c1d/scramble-replay.png)

The snapshot controls both exited 0 at tick 481. The timed controls ran
three 64×64 frames from tick 481 at 120 fps with default parameters and
no warmups. The six verdicts and three PNGs matched exactly. GPU p50/p95
were 3.196/12.548 ms before and 2.034/2.175 ms after, with eight observed
commands in each control. One three-frame pair leaves timing variability
unmeasured. [Before timing](../../../out/meta/c1d/timed-baseline.json) ·
[After timing](../../../out/meta/c1d/timed-candidate.json)

The two-job all-target check and single lab build exited 0. All 47 tests
in the six emitted CLI test executables passed from the crate directory.
The build also emitted four example executables. [Check](../../../out/meta/c1d/check-1.log) ·
[Cargo artifact JSON](../../../out/meta/c1d/build.jsonl) ·
[Scoring script and evidence](../../../out/meta/c1d/)

## Scope and remaining measurements

Supported replay inputs are scalar Param and Level on the recorded native
device. Host Event and PosePacket inputs, cross-device tolerance and native
checkpoints retain their typed unsupported outcomes. Event output draining
is used by the session's staging ring. Long traces with emitting dispatches
were unmeasured in the replay probes. The combined full suite and five-gate
run are integrator measurements.
