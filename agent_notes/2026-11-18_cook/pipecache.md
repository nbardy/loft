## Your lane: precompiled pipelines that load fast (worktree `~/git/_wt/loft-pipecache`)

`loft build` already makes an artefact (WGSL + tables + draw list), but the driver still compiles WGSL at load:
3–5 s per creature cold, 0.3–0.6 s warm (Mac). Nothing persists to disk. Goal: near-instant loads in the game and
fewer compiles in training.
- Rust loader (`rust/loft-loader`, wgpu 30): use wgpu's `PipelineCache` where the backend supports it (Vulkan), saved
  and loaded from a caller-given path, keyed by adapter/driver; on Metal state what the OS cache gives and whether
  `MTLBinaryArchive` is reachable through wgpu (if not, say so). Add an API to warm all of an artefact's pipelines up
  front (a loading screen), and report load times cold/warm. `cargo build --release` and run `loft-view` timings.
- Bun/Dawn side (`src/host/`, `loft serve`): find whether bun-webgpu exposes Dawn's blob cache / pipeline cache
  (check node_modules/bun-webgpu and patches/). If yes, persist it with a size bound; if no, document exactly what is
  missing and the smallest patch to add it.
- Measure load times for 3 artefacts (cold, warm, warm-from-disk) and record them.
