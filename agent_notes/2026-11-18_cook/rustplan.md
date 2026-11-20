## Your lane: measure, then plan a lighter, faster compiler (worktree `~/git/_wt/loft-rustplan`; design + measurement only)

The per-scene compile (Bun start, scene import, expression graph build, lower to WGSL) costs 0.45–0.85 s wall and
1–1.5 CPU s per training program; memory has never been measured. The owner wants it lightweight and fast and asks
whether a Rust/wgpu stack is the way.
- Measure on 5 representative scenes (examples/ creatures, a pbr scene, a soft medium, the heron solve): wall, CPU,
  peak RSS of `loft build`, broken down by phase (Bun start, import, graph build, interning, lowering, WGSL emit,
  tables, writing), and the same for `loft serve`'s compile child. Use Bun's profiler or timers; say which.
- Find the top costs and fix the cheap ones in TypeScript now if they are clearly safe (measure before/after; keep
  output byte-identical; stop at ~3 changes).
- Then write `docs/proposals/2026-11-18_compiler_speed.md`: what a Rust port of the compiler would cost and gain
  (with numbers from your profile), versus alternatives (a warm compile worker, snapshotting the graph, caching
  lowered modules by structural hash), and a recommendation with a pass/fail first experiment. The scene authoring
  language stays TypeScript-like for the model unless you argue otherwise with evidence.
