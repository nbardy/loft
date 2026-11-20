# Can a Loft author match hand-written Metal?

**Keep wgpu as the default. Open whole WGSL passes and the pass list to scene authors now. Defer a shipping Metal backend until a tile-memory experiment earns it.** On main, a Hermit shading function and its hand-written WGSL equivalent produce identical Naga arithmetic. An author who wants to halve the number of times that function runs faces a different problem. Loft chooses the dispatch, the visibility-buffer lifetime and much of the code around the call. Several of those decisions currently require editing Rust. That is the lock-in worth fixing first. [Philosophy §0.5](../PHILOSOPHY.md#05-agentic-development-inverts-the-old-trade) · [Frame-cost directions](../../agent_notes/2026-11-20_v1/OPEN.md#frame-cost-directions-owner--astra-20-nov)

## 1. What runs for a shell pixel

I compiled `stdlib/engines/hermit.loft` at main `914df6c`, with its default 20 crabs. The shell's material calls `rib(uv.x, TAU * uv.y)` to set the ridge term in its BSDF. The surface also calls it to displace the shell. These are the complete authored function and the complete function in the generated `HermitShell` shader. [Hermit source](../../stdlib/engines/hermit.loft) · [Expression emitter](../../crates/loft-passes/src/emit.rs)

```loft
fn rib(u: f, a: f) {
  let c = 0.5 + 0.5 * cos(28 * u)
  0.06 * c * c * c * c * (0.6 + 0.4 * cos(5 * a))
}
```

```wgsl
fn loft_user_rib(loft_local_u: f32, loft_local_a: f32) -> f32 {
  let loft_local_c: f32 = (0.5 + (0.5 * cos((28 * loft_local_u))));
  return (((((0.06 * loft_local_c) * loft_local_c) * loft_local_c) * loft_local_c) * (0.6 + (0.4 * cos((5 * loft_local_a)))));
}
```

**Measured at the compiler boundary.** I added an equivalent raw WGSL helper in a scratch copy and changed just the material call to use it, leaving the differentiable surface alone. The two functions' serialized Naga expression arrays compared equal, including operand references and f32 literals. Each has **9 multiplies, 2 adds, 2 cosines and 0 loads**. The local `c` is reused. There are no type tags, dynamic calls or tape operations in the specialized body. This establishes parity through Naga for this function. Machine instructions, register allocation and whole-pass GPU time need a separate control.

Reproduce the source and lowered program from the workspace root. To repeat the control, add the helper below to a copy of Hermit and replace only `let ridge = rib(` with `let ridge = hand_rib(`. In `loft show`, follow `loft_user_hand_rib` to its raw callee. Compare that callee’s `expressions` field in the S-expression with the field for `loft_user_rib`. The derivative lives in `loft_user_rib__jvp`.

```loft
fn hand_rib(u: f, a: f) -> f = wgsl(u: f, a: f) -> f {
  let c = 0.5 + 0.5 * cos(28.0 * u);
  return 0.06 * c * c * c * c * (0.6 + 0.4 * cos(5.0 * a));
}
```

```sh
tools/dev.sh run loft-cli --bin loft -- show stdlib/engines/hermit.loft --kind HermitShell --plan
target/lab/loft ir stdlib/engines/hermit.loft --sexp
target/lab/loft check stdlib/engines/hermit.loft
```

**Measured outside the function.** The compiler staged 321 rows across five kinds (one ground, 20 shells, 20 faces, 40 claws and 240 legs). The author specified the shell’s 192×32 grid. Loft selected analytic hits for ground and legs, and sphere tracing for face and claws. Compiling at counts 1 and 20 gave eight byte-identical WGSL modules in the same order. The returned plan was Cull → Vis (5 kinds) → Shadow (4) → Classify → Shade (5) → Present. That accounts for 17 frame commands. The native path can bake the 193×33 unique grid positions once and draw indexed geometry. The bake added a fourteenth load pipeline against a budget of thirteen. A compile-time choice bought frame time and had a separately measurable load cost. [Grid bake](../../crates/loft-passes/src/emit/surface.rs) · [Measured Hermit trade-off](../../agent_notes/2026-11-20_v1/lanes/d1b.md) · [Gate budgets](../../engines/gates.json)

**Where compilation happens.** Parsing, unit and space checking, forward differentiation, scan baking (256 frames for `whorl`), shape-bound derivation and module construction run before steady-state drawing. Space and unit checks cost zero per invocation. The generated derivatives, sample tables and geometry kernels can change each frame’s work. In particular, ordinary arithmetic emits operators and calls, while `dir` can emit a normalization and a typed transform can emit a matrix operation. On main, `naga_module` builds **WGSL text → Naga parse → validation**. The native CLI passes the module to wgpu as `ShaderSource::Naga`. Portable bundle lowering remaps bindings in Naga, writes WGSL, and the embedded loader parses it for reflection and submits WGSL. Spec 07 describes direct construction of Naga IR. Removing today's parse/write round trips primarily improves load and edit latency. wgpu/Metal still specializes and compiles pipelines. [Compiler](../../crates/loft-passes/src/lib.rs) · [Bundle writer](../../crates/loft-passes/src/bundle.rs) · [CLI shader creation](../../crates/loft-cli/src/native/mod.rs) · [Embedded loader](../../crates/loft-runner/src/encode.rs) · [Spec 07 §6.3](../spec/07_PLATFORM_POWER_EXTENSIONS.md#63-why-loft-emits-nagamodule-directly)

## 2. What an author can replace on main

| Author's change | What actually works |
|---|---|
| Write a raw WGSL function | **Yes.** A `wgsl(x: f) -> f { return abs(x); }` expression captures typed names in scope. The parser lifts it to a helper and the emitter copies its body into the surrounding module. A scratch Capsules material compiled with the raw body intact. The RGB reference integrator already writes its bounce loop this way. The island inherits the enclosing shader interface. [Parser](../../crates/loft-parse/src/parser.rs) · [Reference engine](../../stdlib/engines/reference.loft) |
| Call it from a proof or motion law | The source CPU evaluator refuses a raw body (E405), symbolic AD refuses it (E307), and a raw dependency in the proven part of a trace has no Lipschitz certificate (E211). A motion law reaching unmarked raw WGSL gets E504. `wgsl det (...)` is the author's determinism assertion. A separate runner interpreter can execute a supported subset of row-local Naga compute. [Checker](../../crates/loft-passes/src/check.rs) · [Evaluator](../../crates/loft-passes/src/eval.rs) · [AD](../../crates/loft-passes/src/ad.rs) · [Compute twin](../../crates/loft-std/src/compute_twin.rs) |
| Supply a whole WGSL compute or render pass with private resources from a scene | **No general source path.** I inserted each of `pass user_compute reads [ColorHdr] writes [Aux] { dispatch(1) }` and `pass overlay reads [ColorHdr] writes [Aux] { raster(verts: 3) }` after shading in Capsules. Both reached E405, “custom entry is outside this phase’s renderer”. Reading an unavailable input fails earlier with E505. A valid `kernel wave_step(n: u) { let x = n + 1 }` reached the E405 pending-lowering diagnostic. `kernel … allow(...)` checks typed Loft operations and effects. The spectral engine has a dedicated lowering for `deposit_caustic`. A `screen_pass` accepts an author function with signature `(v3, v3, u, u) -> rgb` inside a compiler-owned entry. [Lowering](../../crates/loft-passes/src/lib.rs) · [Spectral exception](../../crates/loft-passes/src/paths.rs) · [Language](../spec/01_LANGUAGE_V1.md) · [Kernel contract](../spec/PATCH_V1_2.md) |
| Supply the same pass from a host | **Yes, within the bundle ABI.** Construct a WGSL `ModuleBlob`, tables and a `PassDecl`. `Compute` accepts fixed XYZ, row-count or indirect dispatch. `RenderBundle` accepts authored vertex/fragment entries, indirect/indexed draws and declared attachments. Choose sparse slots within the access-class groups. The runner reflects entry use and checks it against the declaration. Its tests bind writable storage at group 2 slot 5. Raster currently supports storage vertex pulling, restricted pipeline state and sample counts 1 or 4. A host can also encode its own wgpu work around `encode_into` and share resources. [Runner fixture](../../crates/loft-runner/src/tests.rs) · [Pass linker](../../crates/loft-std/src/schedule.rs) · [Render pipeline](../../crates/loft-runner/src/embed.rs) |
| Replace the schedule | **Yes for the built-in vocabulary.** Capsules supplies its own complete schedule, names the kinds and omits the shadow pass. Default elaboration treats a schedule containing `render_bundle` as complete. For other schedules it adds the selected render recipe to the authored operations. Lowering checks resource availability and duplicate producers, accepts Vis64 for the draw bundle and rejects `tile: 16` with E405. The path recipe lowers its screen integrator into Present. At the bundle level the host owns the ordered pass list, can hot-swap a compatible bundle and can use bounded repeats. The runner refuses a conditional `PassLoop` and uses one queue across queue hints. [Capsules](../../stdlib/engines/capsules.loft) · [Recipe elaboration](../../crates/loft-passes/src/defaults.rs) · [Schedule linker](../../crates/loft-std/src/schedule.rs) · [Hot swap](../../crates/loft-runner/src/lib.rs) |

**First implementation.** Let a source `pass` name a complete WGSL module, its entries and dispatch or draw, and map its reflected bindings to declared tables and attachments. Lower it to the existing `PassDecl`. Allow a complete custom pass list with no default renderer injected. Use one resolved list for CLI profiling and embedded playback. Make the first acceptance case a scene that replaces Classify and Shade with one full-screen pass and a custom table, survives bundle export/reload, and reports both pass time and pixel differences. Today's exporter also refuses programs with path or texture-bake resources. Add those descriptors so the path engines can run through the same interface. [Pass ABI](../../crates/loft-ir/src/schema.rs) · [Export boundary](../../crates/loft-passes/src/bundle.rs)

## 3. Native shader code inside a wgpu pass

**MSL passthrough is feasible in the pinned wgpu 30.0.1.** A native host must enable `Features::PASSTHROUGH_SHADERS` on the device. The unsafe `create_shader_module_passthrough` descriptor accepts MSL source or a compiled `metallib` on Metal, and SPIR-V words on Vulkan. Supply the entry names and compute workgroup sizes. wgpu-core passes the selected payload to the HAL without Naga's shader validation. Browser WebGPU takes WGSL. PTX requires a CUDA-capable executor, device memory and launch/synchronization path. Treat CUDA interop with a graphics frame as a separate cost. [Pinned wgpu API](https://docs.rs/wgpu/30.0.1/wgpu/struct.Device.html#method.create_shader_module_passthrough) · [Descriptor](https://docs.rs/wgpu/30.0.1/wgpu/type.ShaderModuleDescriptorPassthrough.html) · [Backend selection](https://github.com/gfx-rs/wgpu/blob/40f4a34ebaf56f9a046231f54125ad046239d3f3/wgpu-core/src/device/resource.rs#L2499-L2564)

**Proposed author syntax**, illustrated with an ordinary material term. The same selection applies to entire stage modules.

```loft
fn ridge(u: f, a: f) -> f =
  msl(u: f, a: f) -> f {
    float c = 0.5f + 0.5f * metal::cos(28.0f * u);
    return 0.06f * c*c*c*c * (0.6f + 0.4f * metal::cos(5.0f * a));
  } fallback wgsl(u: f, a: f) -> f {
    let c = 0.5 + 0.5 * cos(28.0 * u);
    return 0.06 * c*c*c*c * (0.6 + 0.4 * cos(5.0 * a));
  }
```

Resolve once at load from the actual backend, device features enabled by the host and any required MSL language/GPU family capabilities. Require a portable variant or an explicit `fallback refuse("requires Apple tile shading")` for a native-only pass. Record the selected target, features and hash in the profile/replay. Main's `HostCaps` lacks native target selection, `ModuleBlob` holds one target, hot swap takes `&str` WGSL and the linker rejects every target except `Wgsl`. Resolve alternatives with one interface to a single module before linking. Include the target, ABI and math mode in the module/pipeline cache keys and invalidate incompatible hot swaps. `Msl`, `Spirv` and `Ptx` already have wire tags. [Schema](../../crates/loft-ir/src/schema.rs) · [HostCaps](../../crates/loft-ir/src/bundle.rs) · [Backend trait](../../crates/loft-runner/src/backend.rs) · [Spec §6](../spec/PATCH_V1_2.md)

Start with whole modules. A helper-level MSL island needs an **entire MSL translation unit** for its caller. Lower the portable enclosing stage via Naga's MSL writer with a typed stub, replace that definition at function emission using a symbol/signature map, and compile the assembled stage as MSL. A WGSL entry cannot link a separate MSL helper module through wgpu. Preserve an inspectable assembled stage and source map. Defer a general Loft-to-MSL emitter until a measured code-generation gap survives a whole-module override.

**Binding rule.** Give the pass one resource signature. Group 0 fixes Frame/Inputs/Params at slots 0/1/2. For bundle groups 1/2/3, use read-only storage, writable storage, and textures/samplers respectively. Preserve table IDs, access, stage visibility, offsets, strides, texture formats, sampling mode and attachment formats across variants. `loft show` displays the CLI shader's pre-remap addresses. Generate native wrapper parameters from the *resolved* pass layout. The pinned Metal HAL allocates buffer, texture and sampler indices separately per stage, takes visibility into account and reserves internal slots. A WGSL binding number is insufficient to infer `[[buffer(n)]]`. Use the exact pinned layout map (or restrict passthrough to a verified subset), supply explicit lengths where required, and check compiled pipeline reflection, workgroup metadata and sentinel reads/writes before enabling a variant. Exercise sparse slots and mixed stage visibility against the pinned HAL whenever wgpu is updated. Refuse a layout mismatch before encoding. On direct Metal, the same table can generate argument-buffer `[[id(n)]]` fields and drive the host encoder. The scoped unsafe call belongs at this verified native boundary. [Bundle remapping](../../crates/loft-passes/src/bundle.rs) · [Binding checks](../../crates/loft-std/src/bindings.rs) · [Pinned Metal slot assignment](https://github.com/gfx-rs/wgpu/blob/40f4a34ebaf56f9a046231f54125ad046239d3f3/wgpu-hal/src/metal/device.rs#L797-L975)

**Gate semantics.** Run Pictures, Speed and Load on the selected native variant. Record native target and counters available. Naga operation counts apply only to the portable stage. Twin has no observation for an opaque native body. Every Truth claim that depends on it is **Unknown** with the missing twin identified. An author can supply a CPU implementation of the same pass contract. Run paired samples, exact motion/replay bits where required, and publish Held or Missed from that comparison. A matching image or a WGSL fallback alone cannot certify the native body. A shipping gate requiring Held stays open until that evidence exists. [Verdicts](../../crates/loft-ir/src/lib.rs) · [Current five gates](../../crates/loft-cli/src/gates.rs)

## 4. A direct Metal backend

**Later.** Measure two changes independently. First run the same entry code, resources and pass boundaries through wgpu/Metal and a thin `objc2-metal` executor. Expect approximately **0 ms GPU gain** from changing the calling API alone. Then change the schedule to use capabilities the portable encoder cannot express. An MSL passthrough module still runs inside a wgpu render or compute pass. It cannot issue a Metal tile dispatch in the middle of rendering. wgpu's transient attachment usage helps when the attachment can already be discarded. Loft's compute Shade reads the visibility texture *after* the render pass. [Apple's single-pass deferred renderer](https://developer.apple.com/documentation/metal/rendering-a-scene-with-deferred-lighting-in-c%2B%2B) · [Metal tile stages](https://developer.apple.com/videos/play/tech-talks/604/) · [wgpu texture usages](https://docs.rs/wgpu/30.0.1/wgpu/struct.TextureUsages.html#associatedconstant.TRANSIENT_ATTACHMENT)

**A forecast for Apple M4 at 1280×800.** The archived Hermit-20 orbit measured 5.461 ms GPU p95 with Vis/Shadow/Shade medians of 1.395/0.636/2.180 ms. Shrine at the close camera, t=0, measured about 3.90 ms p95 with medians near 2.10/0.41/0.31 ms. Shrine spends more time finding the visible surface than shading it. These are recorded wgpu baselines. The estimates below are hypotheses for *frame p95 saved*, conditional on equal output. Frame p95 must be measured across complete frames. [Hermit pass stamps](../../agent_notes/2026-11-20_v1/lanes/d1b.md) · [Shrine ABBA measurements](../../agent_notes/2026-11-20_v1/lanes/w2.md)

| Scene | Predicted tile-memory gain | Predicted ICB-only gain | Predicted combined frame |
|---|---|---|---|
| Hermit-20 | 0.12–0.28 ms, central 0.19 | 0–0.03 ms GPU, central 0.01 | **5.26 ms** central |
| Coral Shrine | 0.04–0.16 ms, central 0.09 | 0–0.02 ms GPU, central 0.01 | **3.80 ms** central |

The model counts bytes first. A single full write and read of `rg32uint` costs 1280 × 800 × 8 × 2 = **16.384 MB** of logical traffic. At an *assumed* effective 80 GB/s that is 0.205 ms. Moving Shadow before Vis lets Vis and Shade share one render encoder. Keep the two visibility words and eligible depth in tile memory, dispatch tile-local material work, and discard temporary attachments at their last use. This also avoids constructing global per-kind tile lists. For Hermit, resolve the empty-pixel sky inside that pass or carry coverage into Present. For Shrine, the woodblock outline compares the center ID to four neighbors, including across hardware tile boundaries. Retaining a compact 32-bit ID attachment for the subsequent outline cuts the two-way saving to 8.192 MB or 0.102 ms under the same assumption. Preserve depth/ID when the host, picking or an AOV consumes them. Cache hits, compression, tile spills and material divergence can erase the entire modeled saving. Capture external bytes and occupancy. [Visibility, outline and present shaders](../../crates/loft-passes/src/emit.rs) · [Formats and recording](../../crates/loft-cli/src/native/mod.rs)

ICBs can let GPU culling select or encode draws and avoid CPU rebinding. The current renderer already records render bundles and uses indirect instance counts. The roughly 0.075 ms command estimate in OPEN describes a five-command frame. Hermit issues seventeen and already amortizes much of its CPU recording. Predict 0–0.08 ms lower **CPU encode** time for these scenes, measure it separately, and give binary archives credit only on cold/warm Load. No matrix or bfloat speedup is budgeted for either scene. [Existing command recorder](../../crates/loft-runner/src/command.rs) · [Apple GPU-encoded ICBs](https://developer.apple.com/documentation/metal/encoding-indirect-command-buffers-on-the-gpu) · [Frame-cost directions](../../agent_notes/2026-11-20_v1/OPEN.md#frame-cost-directions-owner--astra-20-nov)

**Build order and incremental line estimate.** Keep the frontend, table declarations, bundle codec and CPU reference shared. Land the source-pass interface and the WGSL controls first. Spend 250–450 experimental Rust/MSL lines on one direct replay of the Vis/Shadow/Shade inputs and the tile variant before committing to the following production lanes. The production estimates count handwritten Rust and MSL.

| Order | Capability and exit condition | Estimated lines |
|---|---|---:|
| 1 | Implement `ExecutionBackend` over `objc2-metal`. Attach a host-owned device/queue, allocate or adopt buffers, textures and views, validate row strides, uploads, resizes, lifetimes and external handle ownership. Round-trip a row without an implicit copy. | 400–650 |
| 2 | Generate MSL for portable stages with Naga `msl-out` and use authored MSL for native-only stages. Derive argument-buffer layouts, resource residency and per-stage slots. Create compute/render pipeline states and `MTLBinaryArchive` keyed by device/toolchain, shader, layout, formats, sample count and math mode. Verify a cache miss and a warm hit. | 500–850 |
| 3 | Encode the common list into a host command buffer, with direct/indirect draws and dispatches, attachment load/store/resolve, fences as needed, timestamps and readback. Cover both depth conventions and resize. Replay the same images and clock before enabling native optimizations. | 600–950 |
| 4 | Add GPU-generated ICBs and a render-pass variant with tile functions, imageblocks and memoryless attachments. Validate attachment liveness, tile-size/footprint limits and spill behavior. Expose simdgroup matrix and native bfloat behind actual device-family, language and pipeline support. Verify numeric fixtures for each. | 350–650 |
| 5 | Close standalone/embedded parity for replay, snapshots, hot swap, resource epochs, path/bake descriptors, CPU stepping and host interaction. Keep a platform-specific adapter out of the language frontend. | 450–750 |

Total **2,300–3,850 implementation lines**, plus **350–650** lines of parity fixtures and instrumentation. For scale, the existing native CLI preparation module alone is about 1,200 lines. A macOS-only implementation in this scope adds a second backend to every binding, synchronization, format, math, replay and hot-reload change. It needs an Apple GPU CI lane as well as portable CI. An independent general-purpose MSL emitter would add another compiler parity obligation. Use Naga plus small authored native stages unless a pinned shader comparison justifies that expense. [Backend interface](../../crates/loft-runner/src/backend.rs) · [Native preparation](../../crates/loft-cli/src/native/mod.rs) · [Apple GPU feature tables](https://developer.apple.com/metal/Metal-Feature-Set-Tables.pdf)

**Funding decision.** Run locked, paired replays on the same M4 against the fastest pixel-equivalent whole-pass WGSL control. Measure identical-code wgpu/direct Metal, then tile-only, ICB-only and combined. Freeze Hermit's 300-frame orbit and Shrine's 100 close-view frames, warm each for 30, reverse run order and repeat on a second session. Record CPU encode, GPU pass and frame p95, command counts, external traffic, spills, control time, cold/warm load and image/depth/ID comparisons, including tile edges. Ship a second backend if the lower confidence bound on the combined GPU saving exceeds **0.30 ms on either scene**, the other regresses by at most 2%, and the five gates preserve the portable verdicts and output. Below that threshold, keep shipping wgpu and the independent MSL shader escape. The central forecasts favor deferral.

## 5. Put a hand-written control in the Speed gate

**Require Loft's hot pass to be within 5% of an independently hand-written, output-equivalent pass** on the same backend. The upper limit for `p95(Loft GPU pass time) / p95(hand GPU pass time)` is 1.05. Five percent is about 0.11 ms on Hermit's 2.18 ms Shade median and 0.015 ms on Shrine's 0.31 ms Shade median, which illustrates the required timing resolution. It is a release tolerance against a named control. There is no finite test for optimality against every possible implementation.

| Engine and workload | Whole pass to hand-write | Output obligations |
|---|---|---|
| Realtime / Hermit-20 | Shade, including all five kind dispatches. Give the control the same Vis64, shadow map and rows and let it choose how to traverse them. | Linear HDR, empty pixels, final RGBA, shadow and material lobes |
| Capsules-20 | Vis, ray/capsule intersections over the 140 rows | Visibility words, lower-row winner at depth ties, depth |
| Blobs-20 | Vis, bounded SDF march | Hit/miss, row ID, hit distance and depth through silhouette pixels |
| Woodblock / Coral Shrine | Vis over the close camera's primary world, the largest measured pass. Keep carved lantern and all visible branches. | Visibility and depth plus the final image from the unchanged shadow, shade and outline passes |
| Reference | Full-screen `fs_present` including world traversal and the bounce loop on the phase-4 `WALL` scene | Same camera rays, seeds, bounce/sample counts, linear radiance and displayed RGB |
| Spectral caustic | Full-screen spectral `fs_present` on `scenes/spectral_caustic_study.loft`, 128², four samples | Same photon bins, hero wavelengths, dielectric paths, seeds and RGB |
| Roam | Fused `Scramble.advance` from `scenes/crab.loft` running `scramble_step` | State words per tick, contact/check results, checkpoints and catch-up |
| Act | Fused `HermitBoss.advance` from `scenes/boss.loft` running `boss_impact` across both beats | State words, timed hits, slip and recovery, event payloads and checks |

These eight controls cover the four shipped render recipes, both motion libraries in their callers, and every scene program in `engines/`. Use the existing gate sizes and counts for raster, and pin 128² and four samples for the two path controls. Scale motion to 1,024 identical-schema rows with varied initial phases and include a six-tick catch-up. The control author may choose workgroup shape, caching, branching and internal scratch. Pin the externally visible input/output contract, precision, side effects, geometry, quality and sampling sequence. Include all clears, extra launches and conversions the control needs. Keep RNG/input snapshots and the hand sources under review. Copying `loft show` or calling the raw reference loop from both arms would hide a regression. [Gate manifest](../../engines/gates.json) · [Reference fixture](../../crates/loft-cli/examples/phase4_gate.rs) · [Spectral scene](../../scenes/spectral_caustic_study.loft) · [Roam caller](../../scenes/crab.loft) · [Act caller](../../scenes/boss.loft)

**Instrument and verdict.** Compare decoded attachments and changed table bytes before timing. Use exact per-device bits for motion, IDs and deterministic cleanup frames. Apply the existing documented per-pixel replay envelope for variable Shrine beauty pixels, preserve exact depth and separately score the owner's frozen references. Pair corresponding replay times under `tools/gpu_lock.sh`, keep both pipelines resident, use ring timestamps around the entire candidate pass, and time the whole frame as a guard against shifting work. After 30 warmups per arm, collect four 300-frame blocks in ABBA order. Alternate the starting arm across sessions. For sub-resolution passes, batch independent copies of the same input, divide by the copy count and check full-frame timing separately. Preserve raw stamps, adapter/driver, hashes and triangle control. Re-run a contended block (current control cutoff 0.35 ms). Missing timestamps, differing outputs or an unresolved interval cannot earn a pass. [Profiler](../../crates/loft-cli/src/profile.rs) · [Gate timing and contention](../../crates/loft-cli/src/gates.rs)

Use a paired block bootstrap across corresponding replay times, keeping adjacent frames together. Pass when the upper 95% bound on the ratio is ≤1.05. Fail when the lower bound exceeds 1.05. If the interval straddles the limit, repeat once with reversed order, then report inconclusive if it still straddles. Report whole-frame and pass ratios together and keep the existing absolute Speed budget. Later add independent MSL controls on Metal, first on the same schedule via passthrough, then against the best native pass graph with equivalent end-to-end work. The first comparison can expose shader translation cost. The second can expose a schedule or hardware ceiling.

**Budget.** Estimate 80–250 WGSL lines per control and 200–350 shared harness lines, about 840–2,350 lines for eight controls. Four blocks × (30 + 300) frames × eight fixtures = **10,560 frame executions per backend** before repeats. At 5 ms/frame that is 53 seconds of GPU work. Reserve 3–8 minutes per clean run for pipeline preparation, readback and slower fixtures. Record the actual duration. MSL adds a second hand-authored shelf and another paired run. Run changed controls during development and the full shelf for a release or compiler/backend change.

## 6. Which compiler decisions can cost a frame?

| Decision affecting steady-state time | What the author controls today | Smallest fix |
|---|---|---|
| Expression expansion, inlining and contraction | Ordinary scalar maths preserves the operations and the helper call in the measured example. Authors can write `fma` explicitly. Loft chooses reachable helpers, prepared BSDF evaluations and call sites. There is no source directive for inline/noinline or pipeline-wide FP contraction. Naga and the driver determine machine inlining, register allocation and permissible contraction. A raw WGSL helper still goes through that path. | Show final per-entry WGSL/MSL and selected compiler options. Allow a whole-entry override with native math options or a precompiled library where instruction selection matters. Require a pass-level timing and numerical control before changing defaults. [Emitter](../../crates/loft-passes/src/emit.rs) · [Lighting specialization](../../crates/loft-passes/src/emit/lighting.rs) |
| Deterministic per-thread maths and solver parallelism | Motion rewrites six transcendentals, including `sin` and `exp`, to fixed polynomial implementations with explicit FMAs paired with CPU `mul_add`. Other operations are rejected. Row laws fuse into a 64-wide fixed-tick dispatch. The dedicated solver emits a one-invocation workgroup and private state, even for colored sweeps. These choices change instruction count and occupancy. | Let a scheduled pass supply its own workgroup algorithm and twin. Preserve fixed ticks, write ownership, determinism and checkpoint tests on claimed motion paths. Mark unaudited native paths Unknown. [Motion](../../crates/loft-passes/src/motion.rs) · [Solver](../../crates/loft-passes/src/solver.rs) · [Deterministic WGSL](../../crates/loft-abi/src/det.wgsl) |
| Work done per visible or shaded pixel | The author can change `visible`, `trace`, grid resolution, material, light and path estimator. The generated traced proxy caps the march at 192 steps, uses the row's derived L and a fixed hit tolerance. Reconstructing the normal can add four field samples and ambient occlusion another five. Generated PCF uses nine comparisons. BVH traversal, surface baking, material specialization, sky/BSDF evaluation and path stage placement affect actual work. | Publish the enclosing visibility/shading kernels as replaceable recipes. Let authors supply the traversal, normal/AO, sampler or full integrator at the same boundary. Keep conservative bounds and report the evidence for substitutions. Moving Shrine shadows from the map to per-hit traversal once took it from 4.210 to 19.437 ms despite removing the shadow-map draws. [Renderer](../../crates/loft-passes/src/emit.rs) · [Path assembly](../../crates/loft-passes/src/paths.rs) · [Paired shadow control](../../agent_notes/2026-11-20_v1/lanes/d1b.md) |
| Row layout and memory traffic | Field order is authored. The compiler packs an array of f32 words per row, bitcasts non-float fields, appends seven words of bounds/L, builds LUTs and emits loads. The generated renderer chooses Vis64, depth32, HDR16 and per-kind tile lists. A host can build other tables in a bundle. | Expose table layout, stride, format, access and lifetime to source passes so a scene can use packed or structure-of-arrays storage. Reflect the bytes identically for readback and the twin. Keep the seven hidden words an implementation detail of the default renderer. [Renderer ABI](../../crates/loft-passes/src/program.rs) · [Table schema](../../crates/loft-ir/src/schema.rs) |
| Dispatch shape, pass boundaries and synchronization | The default classifies 8×8 tiles with a 32-kind mask, then indirect-dispatches Shade once per kind. A tile containing several kinds can be revisited. The renderer determines stage boundaries, resource materialization and fusion. Native CLI preparation and portable bundle expansion can group the same draws differently. wgpu executes on the supplied queue. | Lower authored compute/raster and complete schedules through the same pass contract. Explore portable fusion first. Describe attachment liveness and hazards for native tile groups or async overlap. An indirect loop with zero work needs real lowering. Charge barriers, repeated tile visits and any intermediate stores to the candidate. [Classifier](../../crates/loft-passes/src/emit.rs) · [Bundle expansion](../../crates/loft-passes/src/bundle.rs) · [Recording](../../crates/loft-runner/src/command.rs) |
| Backend codegen, pipeline readiness and editing | wgpu's safe pipeline can insert bounds handling, workgroup initialization and loop protection. Its public encoder lacks Metal tile functions and ICB authoring. Pipeline compilation normally occurs at load, but a first use, edit, cache miss or promotion can enter the interactive path. Tier 0 actually interprets a dirty kind before specialization. An isolated promotion test measured 0.667 ms against a 0.500 ms budget. | Give trusted native variants explicit layouts and bounds contracts. Prewarm pipelines, measure cold/warm load and first-frame stalls separately, and expose promotion state and Tier 0 timing. Isolate native-only scheduling behind the measured backend decision above. [Metal codegen options](https://github.com/gfx-rs/wgpu/blob/40f4a34ebaf56f9a046231f54125ad046239d3f3/wgpu-hal/src/metal/device.rs#L183-L232) · [Tier 0 measurements](../../agent_notes/2026-11-20_v1/OPEN.md) |

Loft is a light transpiler for the measured shading expression, and a substantial compiler for staging, geometry, motion and frame construction. A ceiling exists wherever a faster implementation requires changing the Rust emitter or leaving the pass API. The deliverable that would change that answer is a scene author replacing a whole hot pass, timing it against Loft's default and shipping the faster version with the same resource contract. Most of that work can ship on wgpu. Direct Metal earns its place where the faster implementation needs a different encoder.

## Scientist checks

### 1. Where contraction is allowed

**Shading retains Metal's fast math. Main also permits contraction in ordinary motion arithmetic.** The proposed D2 binary32 fix has not landed. On `fc7b5ed` I compiled Hermit-20 and added just the law below plus `schedule Main { run_laws(pulse) }` to a copy. The shell material already calls `wave`. All eight original WGSL modules compared byte-for-byte equal. The ninth, `CrabFace.advance`, dispatched the law over 20 rows with a 64-wide workgroup. In a separate control, an unmarked raw sine helper built in the material and raised E504 as soon as the law called it. [Hermit](../../stdlib/engines/hermit.loft) · [Compiler module assembly](../../crates/loft-passes/src/lib.rs) · [E504 check](../../crates/loft-passes/src/check.rs)

```loft
law pulse(h: CrabFace, dt) writes h.{aggro} { h.aggro = h.aggro + dt * wave(h.aggro) }
```

These excerpts pair `loft show` WGSL with the Naga 30.0.1 MSL captured immediately before wgpu-hal compiled each library. In Hermit's `CrusherClaw.fs`, the loop takes up to 192 distance samples. In `CrusherClaw.shade`, the 14-direction lighting loop evaluates the prepared BSDF.

```wgsl
// fs, inside the march
let d = q_trace(row, lro + lrd * t);
if d < 1e-4 + 2e-4 * t { hit = true; break; }
t += d * step;

// shade, GGX denominator and lighting-loop accumulator
let loft_local_q: f32 = ((1.0 - (loft_local_nh * loft_local_nh)) + ((loft_local_nh * loft_local_nh) * loft_local_a2));
loft_local_c = (loft_local_c + (loft_eval_prepared(loft_local_b, loft_local_n, loft_local_v, loft_local_l, loft_lighting) * loft_local_e));
```

```metal
// fs
float _e69 = q_trace(_e8, _e49 + (_e51 * _e66));
float _e70 = t;
if (_e69 < (0.0001 + (0.0002 * _e70))) {
    hit = true;
    break;
}
float _e77 = t;
t = _e77 + (_e69 * step);

// shade
float loft_local_q = (1.0 - (loft_local_nh * loft_local_nh)) + ((loft_local_nh * loft_local_nh) * loft_local_a2_);
loft_local_c = _e42 + (_e44 * _e45);
```

The shell and law each get their own copy of `wave`. The excerpts below are from the two helpers, the range reduction in `det_sin`, and the law store.

```wgsl
return (0.5 + (0.5 * sin(loft_local_a)));
return (0.5 + (0.5 * det_sin(loft_local_a)));
let r = fma(-floor(a / tau + 0.5), tau, a);
loft_local_h.r_aggro = (loft_local_h.r_aggro + (loft_local_dt * motion_loft_user_wave(loft_local_h.r_aggro)));
```

```metal
return 0.5 + (0.5 * metal::sin(loft_local_a));
float _e1 = det_sin(loft_local_a);
return 0.5 + (0.5 * _e1);
float r_2 = metal::fma(-(metal::floor((a_4 / 6.2831855) + 0.5)), 6.2831855, a_4);
float _e8 = motion_loft_user_wave(_e7);
loft_local_h.r_aggro = _e5 + (loft_local_dt * _e8);
```

**Did Metal actually contract?** I rendered a 64×64 sphere whose material reads the row values `a=1+2⁻²³`, `b=1−2⁻²³`, `c=−1`. It selects green for the fused result `−2⁻⁴⁶` (bits `a8800000`) and red for the separately rounded zero. Ordinary `a*b+c` matched explicit `fma(a,b,c)` on all 4,096 pixels. Forcing the product through a runtime integer identity changed 1,804 pixels, including the centre from green to red. This proves contraction occurs on this shading path. The MSL for the march and BSDF establishes eligibility at those sites. Disassembly would be needed to count the instructions there. [Material probe](../../agent_notes/2026-11-20_v1/bk1/witness-mad.loft) · [Full WGSL, captured MSL and witness images](../../agent_notes/2026-11-20_v1/bk1/capture.tar.gz)

`Emitter::new` starts with `motion=false`. Only the row-law and solver emitters enable it and append `DET_WGSL`. The motion call emitter substitutes six fixed transcendentals. Ordinary multiply/add still takes the common operator path. On the archived Crab tick-2 readback, CPU/Metal produced `3c162fca/3c162fcb`. Explicit FMA gave `3c162fcb` on both, and a forced intermediate rounding gave `3c162fca` on both. The Metal HAL compiles each selected entry with `setPreserveInvariance(true)` and leaves its math mode at the language default. At compile time both `shade` and `advance` reported Metal 3.2, Fast (2), with invariance enabled. [Emitter](../../crates/loft-passes/src/emit.rs) · [Law](../../crates/loft-passes/src/motion.rs) · [Solver](../../crates/loft-passes/src/solver.rs) · [Three-way readbacks](../../agent_notes/2026-11-20_v1/design/d2.md) · [Pinned Metal compilation](https://github.com/gfx-rs/wgpu/blob/40f4a34ebaf56f9a046231f54125ad046239d3f3/wgpu-hal/src/metal/device.rs#L257-L286)

**D2 scoping rule.** Root strict lowering at the scheduled fixed-tick entry. Follow the operations that compute state, pose, check verdicts and events, including helpers, raw dependencies and prior interpolation. D2 proposes u32 significand operations in portable WGSL. Round each ordinary binary32 operation and round explicit FMA once in both twins. A native substitute must also reproduce subnormal, signed-zero and exceptional results. D2a calls `det_check::lower` only on the completed motion and solver modules, where its walk starts at entry zero. Preserve that single-entry boundary. A rewrite in shared `naga_module`, an in-place rewrite of a helper shared with a draw, or a device-wide Safe option would reach rendering. Specialize shared helpers by entry and math mode, and key compiled libraries accordingly. Shade is itself a compute entry. Its stage kind cannot select motion semantics. [D2a verifier](https://github.com/nbardy/loft/blob/71b0be06235d4b1fc68e9782b75245a27009ca5a/crates/loft-passes/src/det_check.rs#L481-L558) · [D2a integration status](../../agent_notes/2026-11-20_v1/OPEN.md#d2-integration-on-20-november)

**Frame cost on Apple M4.** I changed only `MTLCompileOptions.mathMode` at wgpu-hal's Naga library compile. A used its observed Fast default, B used Safe. All shader hashes, frame inputs and command counts matched. Each locked 1280×800 ring-timestamp block had 30 warmups. Frame time spans the earliest pass start to the latest pass end. The reported cost is the mean Safe block p95 minus the mean Fast block p95.

| Scene | Frames per block | Fast A1 | Safe B1 | Safe B2 | Fast A2 | Safe frame cost |
|---|---:|---:|---:|---:|---:|---:|
| Hermit-20 orbit, 17 commands | 300 | 5.481 ms | 10.244 ms | 10.457 ms | 5.859 ms | **+4.680 ms (+83%)** |
| Coral Shrine, radius 6.4 m, t=0, 12 commands | 100 | 3.920 ms | 4.388 ms | 4.969 ms | 3.842 ms | **+0.797 ms (+21%)** |

Triangle medians ranged from 0.029 to 0.096 ms, all below the 0.35 ms contention cutoff. At t=0 Safe changed 46,604 Hermit pixels and 2,192 Shrine pixels. Safe changes several optimizer permissions and still gave the fused colour in the material probe. It cannot implement D2's required rounding by itself. The narrower PF1 experiment prefixed **Shade only** with `#pragma METAL fp contract(off)`. Its two order-specific frame-p95 costs were +1.450/+0.274 ms for Hermit and +0.717/−1.028 ms for Shrine. The latter changes sign with order. A uniform 10–15% penalty is unsupported, and neither experiment prices software binary32 in a march. [Capture and math-mode patch](../../agent_notes/2026-11-20_v1/bk1/capture.patch) · [Raw timestamps and RGBA comparisons](../../agent_notes/2026-11-20_v1/bk1/capture.tar.gz) · [Shade-only experiment](../../agent_notes/2026-11-20_v1/design/pf1.md) · [Metal math modes](https://developer.apple.com/documentation/metal/mtlmathmode)

### 2. Generate the Metal signature from the linked pass

On wgpu, wgpu-hal and Naga **30.0.1**, the flat index belongs to a **stage of a pipeline layout**. `PassDecl.bindings` supplies logical group, slot, access and table ID. The generator also needs the fixed group 0 layout and the resolved resource type, array count and stage visibility. Use the same linked layout that creates the wgpu pipeline. For an MSL-only entry, obtain those types and stage uses from its typed resource parameters and table/texture declarations. Generate the `kernel`, `vertex` or `fragment` signature, built-ins, stage inputs/outputs, address spaces, texture access and attributes around the author's body. Supply the compute workgroup size in the passthrough descriptor. [Loft slot schema](../../crates/loft-ir/src/schema.rs) · [Reflection and visibility](../../crates/loft-std/src/bindings.rs) · [Runner pipeline layout](../../crates/loft-runner/src/encode.rs)

1. For each stage, start buffer, texture and sampler counters at zero. Nonzero pipeline immediates take the first buffer slot. Loft currently sets `immediate_size=0`. Walk groups in ascending order and entries in ascending binding order, including visible layout entries unused by this particular entry. Skip entries invisible to the stage. Loft's fixed frame, inputs and params consume buffers 0–2 in the bundle runner.
2. Uniform/storage buffers consume one buffer slot, sampled/storage textures one texture slot, and samplers one sampler slot. A binding array (`entry.count`) consumes one **argument-buffer** slot, including a texture array. Reject resource classes the Loft linker cannot represent. On a pin change, compare the generated attributes against the new HAL and rerun the sparse-slot and subrange probes. Validate limits and regenerate on layout, stage or backend-version changes. A precompiled metallib must match the generated interface.
3. After all groups the HAL reserves the next buffer slot for every stage with visible storage buffers, and unconditionally in the vertex stage to support vertex pulling. On the Naga path `_mslBufferSizes` contains u32 **bound byte lengths** in specialized module global-variable order, expanding binding-array elements in order, followed by mapped vertex-buffer lengths. There is one Metal buffer attribute for this table. Field ordinals and Metal resource indices are independent. Use the logical range after offset, and account for the runtime array's member offset, element size and stride. [wgpu-core ordering](https://github.com/gfx-rs/wgpu/blob/40f4a34ebaf56f9a046231f54125ad046239d3f3/wgpu-core/src/device/bgl.rs#L58-L85) · [Metal slot allocation](https://github.com/gfx-rs/wgpu/blob/40f4a34ebaf56f9a046231f54125ad046239d3f3/wgpu-hal/src/metal/device.rs#L796-L967) · [Naga field layout](https://github.com/gfx-rs/wgpu/blob/40f4a34ebaf56f9a046231f54125ad046239d3f3/naga/src/back/msl/writer.rs#L4599-L4651)

**Raw MSL needs Loft to supply the lengths.** Both Metal passthrough stage-construction paths set `sized_bindings: vec![]`. For compute, the encoder's size update is empty and it skips the upload to the reserved slot. In the captured Naga `advance` signature, `rows [[buffer(4)]]` and `loft_advance_args [[buffer(3)]]` are followed by `constant _mslBufferSizes& _buffer_sizes [[buffer(5)]]`. Copying the last parameter into raw compute MSL would read an unbound slot. Where a raw entry needs runtime lengths, add one compiler-owned read-only sizes table at a free group 1 binding before deriving Metal indices. Assign one u32 byte count per runtime binding in logical (group, slot, element) order. Generate the MSL field access and host upload from that same order and the explicit bound ranges. Refresh the table and bindings together on a rebind or resize. Bind it as a normal resource on both MSL text and metallib paths. [Passthrough render stages](https://github.com/gfx-rs/wgpu/blob/40f4a34ebaf56f9a046231f54125ad046239d3f3/wgpu-hal/src/metal/device.rs#L382-L398) · [Passthrough compute](https://github.com/gfx-rs/wgpu/blob/40f4a34ebaf56f9a046231f54125ad046239d3f3/wgpu-hal/src/metal/device.rs#L1809-L1826) · [Encoder upload rule](https://github.com/gfx-rs/wgpu/blob/40f4a34ebaf56f9a046231f54125ad046239d3f3/wgpu-hal/src/metal/command.rs#L443-L473)

A locked M4 passthrough probe generated this signature from the layout. Group 1 slot 7 held the view length, group 2 slot 5 the writable words, and group 3 slot 9 a 40-pixel-wide texture. Metal reserved buffer 5. The shader reads the length from the bound table at buffer 3.

```metal
kernel void stamp(device const uint& bytes [[buffer(3)]],
                  device uint* answer [[buffer(4)]],
                  texture2d<uint, access::read> img [[texture(0)]]) {
    answer[0] = img.get_width() + bytes/4u;
}
```

At byte offset 256 of one 512-byte allocation, WGSL `arrayLength` and raw MSL both returned **42** with an eight-byte view and **41** with a four-byte view. Authors name resources. The linker owns every Metal index. [Executable binding probe](../../agent_notes/2026-11-20_v1/bk1/slots.rs)

### 3. A full WGSL pass from a scene

**The source CLI stops before dispatch.** I used this entire Naga-valid module and a two-u32 zeroed table. Its expected readback after one workgroup is `[42, 0]`.

```wgsl
@group(2) @binding(5) var<storage, read_write> answer: array<u32>;

@compute @workgroup_size(1)
fn stamp() {
    answer[0] = 40u + arrayLength(&answer);
}
```

`loft render stamp.wgsl --size 1x1 --out raw.webp` exited 1 with E404 at the first `@`. In a copy of Capsules I inserted `pass stamp reads [] writes [Answer] { dispatch("stamp.wgsl", "stamp", 1) }` before tonemapping. Rendering the scene exited 1 with **E405, “custom entry is outside this phase’s renderer”**. The parser accepts the explicit pass body as a block. Lowering checks read availability and returns E405 unconditionally before resolving the body or opening the file. The CLI’s lowered render plan enumerates Cull, Vis, Shadow, Classify, Shade and Present. Its separate fixed-tick dispatch list binds motion resources. [Native plan](../../crates/loft-passes/src/program.rs) · [Parser](../../crates/loft-parse/src/parser.rs) · [Schedule lowering](../../crates/loft-passes/src/lib.rs) · [Probe inputs and CLI output](../../agent_notes/2026-11-20_v1/bk1/capture.tar.gz)

**Positive control and a range bug.** On the M4 the same shader ran after a host encoded and reloaded a 619-byte bundle with `PassRun::Compute`, fixed dispatch `[1,1,1]`, and table 11 at (2,5). An eight-byte host storage buffer returned `[42, 0]`. The runner's own eight-byte table returned **`[43, 0]`**. It allocates with `VERTEX`, wgpu-core pads 8 to 12 physical bytes, and `as_entire_binding()` leaves the binding size unspecified. The Metal HAL then uses the remaining allocation length. Pass the explicit logical byte length on both shader paths. [Bundle readback probe](../../agent_notes/2026-11-20_v1/bk1/runner.rs) · [WGSL module](../../agent_notes/2026-11-20_v1/bk1/stamp.wgsl) · [Allocator](../../crates/loft-runner/src/lib.rs) · [Binding](../../crates/loft-std/src/interop.rs) · [wgpu-core padding](https://github.com/gfx-rs/wgpu/blob/40f4a34ebaf56f9a046231f54125ad046239d3f3/wgpu-core/src/device/resource.rs#L1081-L1100) · [HAL bound length](https://github.com/gfx-rs/wgpu/blob/40f4a34ebaf56f9a046231f54125ad046239d3f3/wgpu-hal/src/metal/device.rs#L1175-L1185)

**Smallest source change.** Extend the explicit compute pass with one complete WGSL module, entry, dispatch and logical binding-to-table map. For a new output, give the table an element type, capacity and initial value (here, two zeroed u32s). Resolve the module relative to the scene, validate with Naga and reflect the selected entry, check the concrete resources and access against its declarations, then emit existing `ModuleBlob`, `TableDecl` and `PassRun::Compute` records. Route that scheduled pass through the existing runner in the source CLI and preserve its position and producer/consumer edges on export/reload. Executing and reloading the shader above with the runner-owned table is sufficient for the first acceptance case. [Open target](../../agent_notes/2026-11-20_v1/OPEN.md#source-authored-wgsl-compute-passes)

**The hand control owns the whole pass.** For Hermit, let it read the same visibility, shadow and row inputs and produce the same HDR and final pixels using one full-screen launch if that wins. Charge Classify plus all per-kind Shade dispatches to Loft, and charge the control for every launch, clear, conversion, intermediate and barrier it needs. Check output before timing, timestamp each whole alternative and report frame p95 alongside it. Section 5's release limit remains an upper 95% bound of 1.05 on the Loft/hand p95 ratio. Reimplementing an inner BSDF inside the generated 8×8 tile schedule cannot test the cost of choosing that schedule.

<details>
<summary>Reproduce the CLI rejection and both binding probes</summary>

From the root of a scratch checkout on Metal, build the two probe files as CLI examples. The bundle probe checks the eight-byte host buffer first and then asserts the same result for the runner-owned table.

```sh
p=agent_notes/2026-11-20_v1/bk1
mkdir -p out/bk1
cp "$p/stamp.wgsl" out/bk1/
awk '/tonemap_aces/ {print "  pass stamp reads [] writes [Answer] { dispatch(\"stamp.wgsl\", \"stamp\", 1) }"}
     {print}' stdlib/engines/capsules.loft > out/bk1/stamp.loft
cp "$p/slots.rs" crates/loft-cli/examples/bk1_slots.rs
cp "$p/runner.rs" crates/loft-cli/examples/bk1_runner.rs
cp "$p/stamp.wgsl" crates/loft-cli/examples/
tools/dev.sh build loft-cli --fast --bin loft --example bk1_slots --example bk1_runner
naga out/bk1/stamp.wgsl
tools/gpu_lock.sh target/lab/loft render out/bk1/stamp.loft --size 1x1 --out out/bk1/probe.webp
tools/gpu_lock.sh target/lab/examples/bk1_slots
tools/gpu_lock.sh target/lab/examples/bk1_runner
```

To inspect the shading pair independently, use Naga 30.0.1. This offline command reproduces the arithmetic. For the live capture, I applied the linked patch to a scratch copy of the registry's `wgpu-hal-30.0.1`, selected that copy with Cargo's `patch.crates-io`, and set `LOFT_PROBE_MSL` to an existing output directory. The patch records the source immediately before Metal compilation. Setting `LOFT_PROBE_SAFE=1` selects arm B.

```sh
target/lab/loft show stdlib/engines/hermit.loft --kind CrusherClaw --json |
  jq -r '.modules[0].wgsl' > out/bk1/claw.wgsl
for entry in fs shade; do
  naga --compact --entry-point "$entry" --metal-version 3.2 \
    --index-bounds-check-policy Restrict out/bk1/claw.wgsl "out/bk1/$entry.metal"
done
```

</details>
