Implemented Rust loading-screen warm-up and atomic Vulkan cache files keyed by adapter and driver. Activating an artefact with retained pipelines took **0.08–1.28 ms** of CPU setup. All three artefacts rendered byte-identical 384×240 frames across Vulkan cache modes.

On Apple M4 via MoltenVK, restarts from saved caches took **25.54 s** for Shell, **5.12 s** for Serpent, and **9.58 s** for Reefback Titan, including cache import. Cache files ranged from 120 KB to 5.05 MB. Native Vulkan startup remains unmeasured.

Metal provides OS-managed shader reuse, and wgpu 30 exposes no `MTLBinaryArchive` hook. Bun/Dawn persistence requires the native callback bridge documented in the notes. [Apple Metal walkthrough](https://developer.apple.com/videos/play/wwdc2020/10615/) · [wgpu cache support](https://docs.rs/wgpu/30.0.1/wgpu/struct.PipelineCache.html#implementations)

Release builds, typecheck, and nine targeted tests passed. The full suite exited on SIGTERM before totals. One Dawn test reached its 180-second limit in both runs.

[API examples, timings, and verification notes](/Users/nicholasbardy/git/_wt/loft-pipecache/agent_notes/2026-11-18_pipecache.md)