# x2e · Three.js WebGPU embed

Base `1194779`. The existing lab CLI exported the count-one Hermit control as eight modules. `WebGpu` only loaded shader modules and buffers at base. There was no demo frame to time.

## Prediction before implementation

At 1280 × 800, two GPU submissions per frame share one device and queue. Three writes color and standard WebGPU depth first. The Loft schedule draws visibility and shades in its own textures, then loads those same host attachments and writes one fragment per visible Loft pixel. For a covered pixel, the cube survives when its stored depth is smaller, and the crab replaces it when Loft's stored depth is smaller. Loft preserves the host on Vis misses. Predict at least 1,000 overlap pixels for each ordering, zero device/buffer-map/readback calls beyond Three's one device request and a measurable CPU submission time. The light changed at the same camera should change at least 1,000 crab pixels. Measure using headless Chrome on M4, compare lossless compositor screenshots outside the renderer, and save the demo still as WebP q95.

## Bundle and workspace checks

| Count-one Hermit export | Before | After |
| --- | ---: | ---: |
| CLI exit | 0 | 0 |
| WGSL modules | 8 | 8 |
| Bundle bytes | 230,223 | 230,223 |

The exports match byte for byte (SHA-256 `9b094c6d033ad31dd93f4e911ba314661c9db04b452bdf3fc25c30c1bea752c5`).
The WASM change is gated to wasm32 with `web-sys`. The browser-only WGSL edits leave the emitted bundle and
native pipeline code intact. The worktree contains no changes to approved references.

- Optimized native workspace suite held **154/154**. Workspace doctests passed.
- `loft-std` and its reverse dependent `loft-runner` checked for wasm32 with `web-sys`. The runner reports
  its existing unused `Kernel.layout` warning. The optimized WASM built and loaded in Chrome.
- Static checks passed. Earley and llguidance both matched **281/281** expected classifications.

## Headless Chrome score

Chrome 157.0.8103.80 on MacBook Air M4 (WebGPU adapter `apple / metal-3`), 1280 × 800,
Hermit `count=1`, crossing cube, sun at 55°. The GPU run held `tools/gpu_lock.sh`. Other
lanes were active on the host. The demo issued 17 Loft passes, with Three and Loft submitting
in order to the same queue.

| Playwright control | Before · Three only | After · Three + Loft |
| --- | ---: | ---: |
| Queue completion wall p50 | 1.30 ms | 12.50 ms |
| Queue completion wall p95 | 2.81 ms | 17.21 ms |
| Loft passes | 0 | 17 |

Queue completion is CPU wall time from encoding through `onSubmittedWorkDone`, measured over
60 alternating pairs after 10 warmup pairs. During continuous composition, the 120 measured
frames after 30 warmups had 0.70 / 1.20 ms CPU submit p50 / p95 and 16.70 / 16.70 ms
frame-interval p50 / p95.

| Cube pose | Overlap | Cube pixels | Crab pixels | Neither control |
| --- | ---: | ---: | ---: | ---: |
| Front | 18,925 | 18,925 | 0 | 0 |
| Behind | 18,925 | 0 | 18,925 | 0 |
| Intersecting | 18,925 | 6,958 | 11,967 | 0 |

The proof uses lossless compositor captures of the background, cube-only and crab-only controls.
Changing sun azimuth from 55° to −55° changed 57,475 of 66,191 crab-mask pixels by at least
three codes in a channel. All 151 traced Three/Loft pass pairs used identical color and depth
texture handles, with load/store on both Loft attachments. The browser requested one device,
submitted on one queue, and made zero readback calls. Three mapped three buffers at creation
to upload geometry.

[WebP q95 screenshot](../../../out/meta/x2e/demo.webp) · [Raw browser measurements](../../../out/meta/x2e/browser.json) · [Playwright probe](../../../out/meta/x2e/verify.py)
