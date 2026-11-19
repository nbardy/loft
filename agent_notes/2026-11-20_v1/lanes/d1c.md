# D1c · Authored edit to first usable frame

## Prediction before implementation

The recorded 12-frame p95s are 3.124958 ms editable and 0.682833 ms specialized, a 2.442125 ms gap. The reported 0.063666 ms times queue writes after source compilation. Predict 1.2 ms from receipt to final queue write (2 ms cap), zero foreground GPU pipeline creations and a +0.35 ms GPU p95 difference (+0.5 ms cap). Predict field absolute error at most 1e−5, at least 99.9% of RGB pixels within 1/255, zero clean-kind pipeline creations and zero stale installs. Score at 1280×800 with the same camera and the gate's rounded-rank p95 over 12 frames per tier. Also record paired per-frame stamps and repeated values.

For an authored `sphere(p - shift, radius)`, the tape should be 3, 2, 9, 1, 42, 55 (input, row, subtract, constant, sphere, return), with two live registers. At zero shift, both edits should certify Lipschitz 1 and enclose the sphere at the original proof grid's ±0.625 m then ±0.6875 m, before outward padding. One radius slot, one proxy row and one instance bound change. A shadow-casting kind changes the derived caster camera if it extends the caster union. Generation advances by one only after accepting its writes. A box composition changes the tape and reaches the VM. A rejected edit leaves the latest valid generation available for drawing and promotion.

Shared imports `b92f8c057585a476d9b1d9fec82d94f114f6de0b` contain both scored Phase 4/5 tips. The lead override starts this lane from `d4b7646` through the shared merge with `origin/main`. Primary main at `8fd710db37d4b1d701f6b0d4cd97accf9b84e434` lacks that commit. The m23 Speed miss is recorded at `out/meta/m23/BLOCKED`.
