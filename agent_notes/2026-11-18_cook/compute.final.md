Implemented `compute()` for raw WGSL at build, load, and frame time, plus stepped simulations. Outputs feed expressions, raw drawables, and dependent compute passes. Frame outputs start from zero, so seeking reproduces the same pixels. Checks and `solve` report a named `undecided: raw compute` result. Compute uses artefact format v4, and both loaders still read v1–3.

Apple M4 Metal measurements at 1280×720:

| Example | Artefact bytes | Rust load | Median GPU frame |
|---|---:|---:|---:|
| SDF volume | 456,406 | 45 ms | 3.261 ms |
| Dust | 24,470 | 197 ms | 0.491 ms |
| Pattern | 20,208 | 59 ms | 0.102 ms |

The SDF bake took 122 ms and packed 442,368 bytes. Raw steps share `recur`’s scheduler. `recur` retains its CPU twin and checkpoint replay.

All 264 tests passed. Rust rendered all three examples, and `loft serve` verified the GPU-less compile path. Vulkan remains unmeasured. [Lane notes and stills](/Users/nicholasbardy/git/_wt/loft-compute/agent_notes/2026-11-18_compute.md)