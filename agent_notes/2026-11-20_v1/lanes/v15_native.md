# v15_native · R6 rungs 2–3

Predictions before compilation on the Apple M4:
- The 512² neural field performs 8→16 ReLU→4 inference, 50.3M useful MACs. Both paths should match f64 CPU reference within 2e−6 absolute. Historical 8→16→3 ALU p50 is 0.238 ms. Full cooperative tiles avoid global hidden activations, but shared memory and barriers may consume the small ALU budget.
- A 2× temporal upscale shades one quarter of the 1280×800 scene. With the soft-shadow and AO workload, expect GPU p95 to fall by at least 20% after MetalFX, with at least 30 dB display-RGB PSNR.
- The sparse MSL fixture should return [29, 1] on Metal and [29, 0] through WGSL. The second word identifies the selected entry.
- The wasm32 runner should build with the web-sys host and no Metal or Objective-C dependency.

Measure 24 warmups and 64 alternating pairs using GPU timestamps. The triangle control is taken before and after. The MetalFX PSNR uses the downstream wgpu output after the split command buffers.
