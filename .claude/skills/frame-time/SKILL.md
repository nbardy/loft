---
name: frame-time
description: Measure or reduce what a Loft scene costs per frame on the GPU or CPU. Use for performance work and frame-time comparisons.
---

# Measuring a frame

Use `tools/dev.sh run loft-cli -- gate` from the Loft checkout. The Speed gate measures GPU p95 and commands per frame
for the scenes in `engines/gates.json`. The report at `out/meta/gate.json` includes device and scene measurements.
`gate --only NAME` selects one configured scene for diagnosis.

State the scene, resolution, device, instance count and budget. Predict the result before changing code. Run one
GPU measurement at a time and compare warmed measurements within the same clock state. Report load and pictures
alongside speed using the approved reference frames. See the Phase 1 notes in `agent_notes/2026-11-20_meta/phase1.md`.
