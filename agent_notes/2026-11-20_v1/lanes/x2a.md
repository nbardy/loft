# X2a · One execution path

The main input was `1194779` and the execution input was D2c `1d89966`. Main’s scored renderer is `b0fdb44`. The intervening main commits do not change its renderer or the gate inputs.

The explicit ranges preserve the 4,488-byte GGX LUT binding and its 1,122-word runtime length. Predict 18/18 RGBA hashes equal to the retained main run. The shared recorder should keep 5/5, 5/5 and 13/13 warm commands at counts 1 and 20 because submitted native frames still reuse the proved-static caster wave. The retained main Load was 31/33, with the two Hermit counts at 14 pipelines against 13. Predict the common pipeline batch keeps the other 31 checks within their limits. The direct R16Float path needs the device format feature. Predict typed refusal before pipeline creation for a storage binding without it, and exact halfwords at every mip when an initializer can use packed copies.

The branch inherits D2c’s descriptor codec and Std interop changes. This lane writes exact source byte views into that existing descriptor, keeps numeric source addresses through shader remapping, reuses the shared recorder for native loads and preserves the submitted native cache policy. Compile and render pipeline misses join one native batch when the caller supplies the attachments at load. The native host supplies a monotonic measurement clock.

## Physical lines

The table uses tracked Rust and WGSL, with blank lines included. The execution columns count the runner's production source, native GPU sources up to the camera math, compiler bundle and bake preparation, the versioned descriptor, and Std's device-facing execution and tests. The span ledger records every charged file. Std's charged span alone is 2,419 lines after the inherited D2c changes. The 2,500-line total remains unmet.

| Crate | All source main → X2a | Execution main → D2c → X2a |
| --- | ---: | ---: |
| loft-abi | 498 → 498 | 0 → 0 → 0 |
| loft-cli | 5,616 → 4,750 | 2,310 → 1,168 → 1,444 |
| loft-format | 815 → 815 | 0 → 0 → 0 |
| loft-ir | 548 → 720 | 0 → 171 → 171 |
| loft-parse | 2,723 → 2,723 | 0 → 0 → 0 |
| loft-passes | 16,456 → 16,637 | 648 → 819 → 829 |
| loft-plan | 949 → 949 | 0 → 0 → 0 |
| loft-runner | 1,630 → 2,456 | 1,028 → 1,785 → 1,854 |
| loft-std | 4,321 → 4,241 | 2,497 → 2,419 → 2,419 |
| **Execution total** | | **6,483 → 6,362 → 6,717** |

The native executor, separate texture-bake and photon executors, bespoke draw and compute pipeline preparation, and triangle and twin probe execution moved onto the shared path. The remaining code includes native proof of caster reuse, timing, edit orchestration, motion result decoding, and host readback. General texture subresource views, sampler descriptors, index subranges and attachment load/store policies still use the inherited bundle defaults. Browser verification covered compilation for a host-owned wgpu device. Raw JavaScript GPUDevice import remains open.

## First score

On Apple M4 Metal at 1280 × 800, with the GPU lock, 30 warmups and 300 measured frames per scene, `7658265` matched **18/18** retained-main RGBA hashes. The capsule center was RGBA (150, 152, 60, 255) with the red HDR word `0x3277`. Twin held 36/36, Truth held 34/34, Speed held 10/12, Load held 23/33 and the previously queued Pictures verdict held 0/18. The controls reported zero contended verdicts. Shared compile activity was observed. Warm command counts were 5/5, 5/5 and 13/13. Native and archive matched at time 0, when the baked archive camera equals the native orbit camera. The archive control now supplies the native host camera for subsequent times.

| Scene | Main → X2a p95 ms | Main → X2a cold s | Main → X2a warm s | Pipeline creates |
| --- | ---: | ---: | ---: | ---: |
| Capsules 1 | 0.451 → 1.307 | 0.333 → 1.199 | 0.005 → 0.022 | 5 → 5 |
| Capsules 20 | 0.663 → 2.116 | 0.355 → 1.249 | 0.006 → 0.025 | 5 → 5 |
| Blobs 1 | 0.406 → 0.634 | 0.324 → 1.192 | 0.005 → 0.020 | 5 → 5 |
| Blobs 20 | 1.079 → 1.813 | 0.405 → 1.154 | 0.006 → 0.026 | 5 → 5 |
| Hermit 1 | 2.453 → 2.942 | 0.888 → 3.512 | 0.022 → 0.103 | 14 → 14 |
| Hermit 20 | 3.752 → 4.468 | 0.916 → 4.803 | 0.022 → 0.082 | 14 → 14 |

The initial range-adoption, shared encoder/query set, and typed R16 refusal probes passed. The load and dirty bake exercise, both host depth conventions, archived Classify fixture, archived Hermit/bake/spectral on the default device, and all five AOV executions passed. Four archived bundles repacked to the same bytes. The frozen reference inventory matched and the nine bead frames passed. The native suite found an R16Float mip-0 mismatch and the existing Tier 0 timing failure (0.573 ms versus 0.500 ms, no pixel differences over 1/255). The first R16 sample mismatch was Odd mip 0, texel (1, 0), direct halfword `0x2cd5` and packed `0x2cd6`. At that texel the authored value is 0.07554945. This predicts `0x2cd5` if the packed Metal encoding truncates the magnitude, as the direct storage write did.

## Controlled repeat

With both shared compile locks and the GPU lock held, `3f92e9d` matched **18/18** in the five-gate run. The native and decoded archive callers each matched **18/18** with the same host camera inputs. Warm command counts remained 5/5, 5/5 and 13/13. Packed R16Float and R8Unorm matched direct storage bytes at every sampled base and mip for odd 2D, cube and dirty 3D bakes. The typed ungated storage refusal, explicit binding-range probe and shared encoder test held. Independently, the original nine bead RGBA frames matched retained main.

Load improved to **29/33**. Beyond the two inherited Hermit pipeline-count misses, blobs-20's cold load was 0.7353 s against 0.500 s (0.169 s front end and 0.557 s longest pipeline). Hermit-20's warm load was 0.060012 s against 0.060 s (0.02457 s front end and 0.03544 s device). Capsules p95 was 1.1178/1.3252 ms against 0.8/1.1, giving Speed **10/12** with zero contended verdicts. The remaining native suite failure was the inherited Tier 0 timing test at 0.601 ms versus 0.500 ms, with no pixel error above 1/255.

Independent device shader-module creation is now in a worker batch ahead of the existing pipeline batch. This should reduce Hermit's warm device preparation by at least 3 ms if wgpu's shader preprocessing overlaps. It leaves the measured front-end slice and any variance in the driver's slowest cold pipeline outside that change.

## Final score

The last permitted repeat ran `ade7891` under the GPU lock and both compile locks. Adapter, frame inputs and sampling matched the previous runs. Native and decoded archive stills each matched **18/18** retained-main RGBA hashes with the same camera uniforms. The five-gate report also matched **18/18**. The packed/direct test verified all authored bases and mips again.

| Scene | Main → X2a p95 ms | Main → X2a cold s | Main → X2a warm s | Timed commands |
| --- | ---: | ---: | ---: | ---: |
| Capsules 1 | 0.451 → 0.835 | 0.333 → 0.321 | 0.0053 → 0.0102 | 5 |
| Capsules 20 | 0.663 → 1.0998 | 0.355 → 0.327 | 0.0062 → 0.0109 | 5 |
| Blobs 1 | 0.406 → 0.791 | 0.324 → 0.283 | 0.0051 → 0.0101 | 5 |
| Blobs 20 | 1.079 → 1.351 | 0.405 → 0.386 | 0.0055 → 0.0103 | 5 |
| Hermit 1 | 2.453 → 2.809 | 0.888 → 1.051 | 0.0215 → 0.0359 | 13 |
| Hermit 20 | 3.752 → 4.436 | 0.916 → 0.922 | 0.0224 → 0.0368 | 13 |

Load met **31/33**, with 14 pipeline creates against 13 at both Hermit counts. Twin held **36/36**, Truth **34/34**, and Speed **11/12**, with zero contended verdicts. The triangle control p50 ranged from 0.042 to 0.099 ms. Capsules-1 exceeded its 0.800 ms cap by 0.035 ms. Pictures held **0/18** against the existing frozen appearance verdicts. All 18 frozen reference files verified against the starting file hashes. The grammar and static checks passed, including 281/281 fixtures under both recognizers. Workspace all-targets and WebAssembly checks passed. Workspace doctests passed. The native suite passed 158 tests, with one full-resolution test marked ignored and run separately. The Tier 0 timing test measured 0.3275 ms overhead on this run, with zero pixels over 1/255.

Evidence in the primary checkout includes the [final gate](../../../out/meta/x2a/r3/gate.json), [RGBA comparisons](../../../out/meta/x2a/r3/pictures.json), [native suite](../../../out/meta/x2a/r3/native.log), [packed comparison](../../../out/meta/x2a/r3/packed.log), [line ledger](../../../out/meta/x2a/lines.json), [bead comparisons](../../../out/meta/x2a/bead-parity.txt), and [reference file verification](../../../out/meta/x2a/refs-after.log).

**ESCALATE** · The 2,500-line cap and general descriptor fields remain open. The final Speed gate missed one scene. The branch is available for review with the complete measurements above.
