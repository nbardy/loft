# Legacy and native Loft on capsules

Apple M4 Metal, macOS 15.5, wgpu 30.0.1. The legacy engine is `legacy/2026-11-19` at `acb7e8b`. The native engine is the starting `origin/main` at `1194779`. Both used 1280 × 800, fixed t = 0, one warmup, and 300 GPU-timestamped frames per case. Each timing block held the GPU lock and both compile locks.

## Measurements

Paired values give the first run and its repeat, in that order.

| Instance groups | Engine | GPU p95, ms | Cold device load, ms | Warm device load, ms | Shader modules | Pipelines | GPU commands/frame | Source tokens |
| ---: | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 1 | Legacy | 3.927 / 2.718 | 609.663 / 362.094 | 12.491 / 6.561 | 5 | 5 | 5 | 1,197 |
| 1 | Native | 2.973 / 2.269 | 513.708 / 607.304 | 5.568 / 2.607 | 4 | 5 | 5 | 621 |
| 20 | Legacy | 9.389 / 9.326 | 476.615 / 405.851 | 11.087 / 7.877 | 5 | 5 | 5 | 1,197 |
| 20 | Native | 2.798 / 1.019 | 449.818 / 470.712 | 2.070 / 1.828 | 4 | 5 | 5 | 621 |

Native p95 fell by 17–24% for one group and 70–89% for 20 groups. Warm device loads were lower at both counts in both runs. The cold device-load ordering reversed on the repeat. Native triangle-control p50 was 0.096 ms in both one-group runs and 0.036 / 0.030 ms in the 20-group runs, below the gate's 0.350 ms contention threshold. The native timing tail varied substantially even with those control readings.

Native source-to-device load was 535.338 / 622.789 ms cold and 15.078 / 10.376 ms warm for one group. For 20 groups it was 571.274 / 597.400 ms cold and 13.031 / 11.037 ms warm. The repeat timed the separate legacy Bun bundle builds at 270 ms and 80 ms wall time, including process startup and file output.

## Renders

Legacy is on the left and native is on the right. One group is on top and 20 groups below.

![Capsules contact sheet](../../out/meta/x2d/sheet.webp)

All four stills matched their repeat byte for byte. Both native t = 0 frames also matched the prior PF1 main RGBA hashes. Checksum verification passed for all 18 frozen gate references. [Native RGBA hashes](../../out/meta/x2d/native-pixels.txt) · [Reference verification](../../out/meta/x2d/references-after.log)

## Fixture and timing

- Each group has seven analytic capsules, including a zero-length capsule for the head. The endpoints, radii, layout, and palette formulas match. The legacy fixture in `tools/ab_legacy.sh` expresses the segment axis as row data because its ordered renderer places shapes by translation. Both cameras look at (0, 0.6, 0) along the same viewing direction with 40° vertical field of view. At t = 0 the camera is at x = 0, y = 1.8 + 0.6√count, z = 3.6 + 1.9√count. Legacy derives clip planes from camera distance. Native uses 0.05 and 400.
- Legacy uses the ordered `realtime()` engine with matte shading, one sampled bounce, ray-traced occlusion, four-sample visibility, and its default sky and film curve. Native uses indirect visibility, tile classification, dielectric GGX, fixed sky integration, and ACES presentation. The native capsules schedule has no shadow-map pass. These choices affect both the pictures and the GPU workload.
- GPU p95 is the sorted sample at zero-based index `round(0.95 × 299)`. Legacy spans its first camera timestamp through the last render pass. Native spans the first scheduled GPU pass through presentation and computes the camera on the CPU. GPU commands count draws and dispatches. The legacy count comes from the two named-camera dispatches, the emitted bundle’s draw schedule, and the profiler’s frame-kernel count. The native profiler records the count on every sampled frame.
- Cold and warm device loads are successive loads on one device with the same newly keyed shaders. Legacy uses `loft-draw profile --loads 2 --cold-nonce`, with the bundle read before the stopwatch and the output target created after it. Native uses the Load gate’s device phase, which includes its sized resources. Both exclude adapter/device acquisition and the source frontend from the table. Source counts use `o200k_base` on the entire authored file, including the inline legacy hit function and excluding imported libraries.

Run `tools/ab_legacy.sh LEGACY_WORKTREE NEW_OUTPUT_DIRECTORY` from the lane checkout. The paired profiles, load reports, and lossless stills are under `out/meta/x2d/`. [Measurements JSON](../../out/meta/x2d/results.json) · [Lane record](lanes/x2d.md)
