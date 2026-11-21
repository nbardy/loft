# v15_native · R6 rungs 2–3

## Predictions

- The 512² 8→16 ReLU→4 field has 50.3M useful MACs. The padded 8×8 matrix route computes 67.1M. Expect both to agree with f64 within 2e−6 absolute. Shared-memory traffic and barriers may consume the 0.238 ms historical ALU budget.
- Rendering one quarter of the samples for 2× temporal scaling should cut GPU p95 by at least 20% at ≥30 dB display-RGB PSNR. The sparse slot witness should yield [29, 1] in MSL and [29, 0] in WGSL.

## Apple M4 Metal results

- With 24 warmups and 64 alternating pairs, the 512² ALU arm took 1.022/1.314 ms p50/p95 and cooperative f32 took 0.528/0.678 ms. Each matched 1,048,576 f64 output components within 2.74e−7. The 0.238 ms historical median came from a different 8→16→3 network and remains unmet. The adapter exposed both 8×8×8 precisions.
- A follow-up predicted 0.42–0.50 ms by reducing scratch from 6,400 to 4,096 bytes and unrolling four ReLUs per lane. It reached 0.886/0.892 ms with the same numeric error, while its paired ALU moved to 2.372/2.646 ms. The branch keeps the original shader.
- Temporal MetalFX at 640×400 → 1280×800 moved full-graph p50/p95 from 8.469/8.891 to 4.778/5.319 ms. The downstream wgpu capture preserved the scene’s silhouette and lighting. PSNR against full resolution was 42.161 dB for MetalFX and 43.617 dB for bilinear. Triangle p50/p95 was 0.107/0.110 ms before and 0.098/0.099 ms after.
- The MSL bfloat and unchecked-loop witness returned [29, 1] through generated sparse buffer and texture slots. The same bundle on `HostCaps::WEB_BASELINE` returned [29, 0].

## Verification

- `tools/dev.sh check loft-runner --target wasm32-unknown-unknown --features web-sys` passed with zero Metal or Objective-C packages in the target dependency graph. Both witnesses passed Earley and llguidance. The 306 previously scored grammar inputs and recognizers match the v15_exec revision. The static rerun passed the pin and corpus checks before stopping the redundant full grammar sweep.
- The optimized workspace build produced 33 test executables. The suite passed 179 tests, including the Metal slot test. One test was ignored.
- Runtime sources added 555 physical lines and removed 84. Crate files added 1,350 lines including the witnesses and benchmark.
- Build with `tools/dev.sh build loft-native --fast --example native_bridge` and run `tools/quiet_gpu.sh target/lab/examples/native_bridge out/native_bridge`. Timing logs, WebP previews and lossless comparisons are in `out/meta/v15_native/` in the primary checkout.
