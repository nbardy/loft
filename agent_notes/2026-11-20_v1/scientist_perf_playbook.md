# Chief scientist: where frame time goes on M4, and a ranked performance playbook (20 Nov 2026, verbatim)

Numbers in this file are the scientist's estimates unless measured in Loft.

On your current numbers—**Capsules at `0.46 ms**`, **Coral Shrine at `3.90 ms**`, **20 Crabs at `5.46 ms**`, and command overhead at **`~75 µs`**—your bottlenecks are squarely inside **per-pixel ALU, raymarch divergence, and unified-memory bandwidth**, not CPU submission.

Here is where the time is actually going on an Apple M4 under `wgpu`/`naga`, followed by the highest-ROI optimizations ranked from 5-line wins to architectural upgrades.

---

### 1. Where Current Bottlenecks Actually Show Up

#### A. Raymarch Step Amplification & SIMDgroup Divergence (Likely #1 on Crabs & Shrine)

* **Global Lipschitz inflation:** If one detailed feature (like serrated claw teeth or sharp coral ridges) pushes a kind's global Lipschitz bound $L$ from `1.0` to `2.3`, every ray marching through empty space around the smooth parts of that kind takes **2.3× more steps**.
* **Grazing-angle silhouettes & loose proxies:** Apple GPUs execute in **32-thread SIMDgroups**. If 31 pixels miss or hit in 6 steps and **1 silhouette pixel** grazes an edge for 64 steps, the entire 32-pixel block stalls for all 64 iterations.
* **Shadow / AO re-marching:** If secondary shadow rays march the exact same SDFs at full precision without early-out or coarser proxies, shading cost easily doubles or triples.

#### B. Unified Memory Bandwidth & Full-Screen Pass Round-Trips

* An M4 MacBook Air has high ALU throughput relative to its unified memory bandwidth (~120 GB/s). Every time a separate pass writes a full-screen buffer out to RAM and the next pass reads it back, you pay a real bandwidth tax:
* `VisBuf` (`rgba16uint` + `depth32float` = 12 B/px, or 48 B/px at 4× MSAA) flushed to RAM after rasterization and read back by `shade_tiles`.
* `HdrBuf` (`rgba16float` = 8 B/px) written by `shade_tiles`, read by fog/bloom/post, and written again to the swapchain.


* **AoS cache-line waste:** Reading a few shading fields out of a wide `RowTable` struct (or reading `PoseBuf` / `StateBuf` rows with non-shading fields mixed in) pulls unused bytes into L1/L2 cache on every pixel.

#### C. Hidden `wgpu` / `naga` Shader Taxes

* **Injected runtime bounds checks & shared-memory zeroing:** By default, `wgpu` tells `naga` to emit array/slice clamping on every buffer access and zero-initialize `var<workgroup>` memory (and `naga` still injects `textureLoad` bounds checks on Metal even though Metal 2.2+ handles out-of-bounds texture reads in hardware). In tight inner loops (BVH traversal, SDF tables, matrix/field lookups), runtime bounds checks alone can cost **1.5–2.5×**.
* **Codegen shape & FMA leaks:**
* Aggressively inlining a heavy SDF or BSDF across multiple call sites produces giant flattened CFGs with high register/Phi pressure that Metal's LLVM backend optimizes worse than clean, structured helper functions (even though M4's Dynamic Caching softens classic fixed-partition occupancy cliffs, spilling registers to thread-local memory still tanks throughput).
* If the motion determinism fix (disabling FMA contraction) accidentally leaks into shading shaders, you lose fused multiply-add throughput across every dot product and noise octave.



#### D. Empty-Tile Dispatch & Uncompacted Kind Shading

* If `shade_tiles` runs one full-screen dispatch per `kind` (checking `if vis.kind_id != MY_KIND { return; }`), a scene with 12 kinds reads `VisBuf` **12 times** across the entire screen even if each kind only covers 5% of pixels.

---

### 2. Ranked Performance Playbook (Highest ROI First)

#### Tier 1: Immediate Wins Inside `wgpu` + `.loft` (Portable Across Mac, Windows & Web)

| Optimization | Effort | Expected Impact | How It Works in Loft |
| --- | --- | --- | --- |
| **1. Disable redundant `wgpu` runtime checks on verified passes** | **~10 LOC** (`loft-runner`) | **10–40%+** on buffer-heavy loops | On native builds, compile Loft-verified shaders via `unsafe { device.create_shader_module_trusted(desc, wgpu::ShaderRuntimeChecks::unchecked()) }`. Because Loft's compiler already proves table/loop bounds, you eliminate `naga`'s injected clamp ops and workgroup zeroing without writing a new backend. |
| **2. Per-kind tile lists + `DispatchExtent::Indirect**` | **~120 LOC** (`engines/realtime`) | **2–4×** on multi-kind scenes (Coral Shrine / 20 Crabs) | One cheap 8×8 tile-classification pass scans `VisBuf` once, builds a compact list of `(tile_x, tile_y)` per `kind_id`, and dispatches each kind's shading pass via `DispatchExtent::Indirect` only on tiles where that kind actually appears. |
| **3. Coarse-to-Fine Raymarching (Quarter-Res Pre-Pass) + Over-Relaxation** | **~150 LOC** (`std::geom` / `engines`) | **2–3×** fewer SDF steps | March a 1/4-res cone-tracing depth pre-pass to advance rays through empty space, then start full-res sphere tracing at `min(neighbors_depth) - margin`. Combine with enhanced sphere tracing ($\omega \approx 1.3\text{–}1.6$, falling back to $\omega = 1$ on overshoot) and piecewise/split Lipschitz bounds for spiky sub-parts. |
| **4. Fuse `shade_tiles` + Tonemap / Post** | **~80 LOC** (`engines`) | **0.4–1.0 ms** saved | When no multi-pass bloom/DOF is active, apply fog + AgX/ACES tonemapping at the end of the shading pass directly to the swapchain—eliminating a full-screen `HdrBuf` (`rgba16float`) DRAM write + read + barrier. |
| **5. `f16` Shading Math + Hot/Cold SoA Table Split** | **~100 LOC** (`loft-passes`) | **15–35%** ALU & bandwidth | Keep shading-hot columns (albedo, roughness, inverse transform, $L$) tightly packed in a separate buffer from cold build/motion columns. Use `f16` (`HostCaps::F16`) for normals, BSDF intermediates, and `VisRec` attributes—Apple M4 runs `f16` at 2× ALU rate and half register/bandwidth footprint. |
| **6. Bake Static Procedural Noise / Masks / Sky IBL (`bake_tex`)** | **~100 LOC** (Phase 4) | **High** on procedural-heavy scenes | Pre-bake multi-octave surface noise, caustic/ink masks, and sky irradiance cubemaps at load time into mipmapped textures instead of evaluating 6 octaves of 3D noise per pixel per bounce. |

#### Tier 2: Algorithmic & Hardware Acceleration (Phase 4–5)

* **Hardware `RAY_QUERY` on M4 (`HostCaps::RAY_QUERY`):** M4's dedicated ray intersection units are 2× faster than M3's. Routing triangle mesh shadows, reflections, and AO through hardware TLAS/BLAS ray queries beats software compute `Bvh2` cleanly as mesh counts scale.
* **Half-Resolution Secondary Effects:** Trace soft shadows, ambient occlusion, and indirect diffuse bounces at half resolution with a fast depth/normal-aware bilateral upsample.
* **Verify Codegen Shape (`bk1` Control Gate):** Keep medium/large BSDF and noise helpers as non-inlined functions when called multiple times so Metal's LLVM backend keeps register pressure low, and assert FMA stays enabled in shading MSL.

#### Tier 3: Native `MetalBackend` (Only After Tier 1–2 Are Measured)

* **On-Chip Tile Memory (`MTLStorageModeMemoryless` + Tile Shaders):** Rasterize `VisBuf` and run `shade_tiles` inside the *same* `MTLRenderCommandEncoder` via `dispatchThreadsPerTile`. `VisBuf` never touches unified RAM at all—this is the single biggest win `wgpu` cannot express on Apple Silicon.
* **True `AsyncCompute` (`QueueClass::AsyncCompute`):** Overlap motion/physics/BVH refit on a second `MTLCommandQueue` alongside rasterization.
* **Indirect Command Buffers (ICBs) + `MTLBinaryArchive`:** Only matters once scenes grow to hundreds of kinds/passes or cold-start pipeline compile stutter becomes visible.

---

### 3. Five-Minute Diagnostic Loop (So You Never Optimize Blind)

Before changing shaders, run this three-step triage on 20 Crabs and Coral Shrine so you know *which* bottleneck actually owns the `5.46 ms` / `3.90 ms`:

1. **`loft profile --json` (Pass Timestamps + `naga` Telemetry):** Pinpoint the exact millisecond share of `cull`, `vis_raster`, `shade_tiles`, `shadows`, and `post`, alongside static ALU / transcendental / storage-load counts per shader.
2. **Inspect `--aov sdf_steps` and `--aov lipschitz_ratio`:**
* Bright red on silhouettes → add quarter-res coarse pre-pass / tighter proxy AABBs.
* Deep blue (`< 0.3`) across 90% of a creature → global $L$ is inflated by one tiny sharp detail; split the kind or use local $L$.


3. **Capture 1 frame in Xcode Metal Debugger (GPU Counters on M4):** Check the three hardware limiters:
* **ALU / F32 Limiter high** → cut march steps, switch BSDF intermediates to `f16`, bake procedural noise, verify FMA is on.
* **Memory / L1-L2 Bandwidth Limiter high** → fuse passes, compact tile lists, split hot/cold SoA tables, and (eventually) move `VisBuf` into Metal tile memory.
* **Divergence / Synchronization high** → check SIMDgroup step variance, remove barriers by fusing passes, and try `ShaderRuntimeChecks::unchecked()`.
