Implemented `depthOfField` and `filmGrain` as composable `Style.post` passes. The copper-sphere example focuses the hammered ball with a circular aperture. Grain peaks in the midtones and repeats byte-for-byte at the same frame time. [Example](/Users/nicholasbardy/git/_wt/loft-dof/examples/materials/refocus.ts) · [Stills and notes](/Users/nicholasbardy/git/_wt/loft-dof/agent_notes/2026-11-18_dof/README.md)

At 1280×720 on Apple M4, depth of field took **3.34 ms/frame** and grain **0.10 ms/frame**. The existing post schedule adds **13.18 MiB** of texture storage, derived from the attachment formats.

The GPU chart measured a **7.42 px** background blur radius against **8.00 px** predicted, within the **0.8 px** tolerance. It also verified sharp focus and depth-edge coverage. Transparent layers use underlying opaque depth, or infinity where depth is clear.

Typecheck, targeted GPU tests, the example budget check, build, and four-sample still passed. Source and artefact renders matched. Both timing failures passed individually after the full suite received SIGTERM. The shared test lock blocked completion of the remaining checks.