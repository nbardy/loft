---
name: frame-time
description: Measure or reduce what a Loft scene costs per frame on the GPU or CPU. Use for performance work and frame-time comparisons.
---

# Measuring a frame

Use `tools/gpu_lock.sh tools/dev.sh run loft-cli -- gate` from the Loft checkout. The Speed gate measures GPU p95 and commands per frame
for the scenes in `engines/gates.json`. The report at `out/meta/gate.json` includes device and scene measurements.
`gate --only NAME` selects one configured scene for diagnosis.

For a scene outside the gate, use `loft profile SCENE --size WxH --frames 120 --json OUT` inside one
`agent-build --gpu --max-minutes 5 "what it is"`: read the median, p95, `gpu_pass_ms` (time per pass) and `contended`
from the JSON. A single frame from `loft still`, `loft clip` or a contact sheet is a cold frame, not a measurement:
three character workers and their lead reported cold frames as frame times for a day (25 Nov).
`loft clip SCENE --aov sdf_steps` and `--aov overdraw` draw where the visibility pass spends its time; the steps
view shows only rays that hit, and the rays that miss are the expensive ones.

State the scene, resolution, device, instance count and budget. Predict the result before changing code. Run one
GPU measurement at a time and compare warmed measurements within the same clock state. Report load and pictures
alongside speed using the approved reference frames. See the Phase 1 notes in `agent_notes/2026-11-20_meta/phase1.md`.
