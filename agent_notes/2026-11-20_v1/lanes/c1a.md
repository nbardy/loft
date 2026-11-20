# C1a · Index-only Hermit surfaces

Base `173d6e9a2b4fa36fcb45c0957f75e9c44f8e6dfa` from `origin/main`. The C1 probe at `a4f3e75` is evidence for the design. The base and probe have identical renderer, compiler and scene inputs.

## Prediction before implementation

At counts 1 and 20, the same 36,864 indices address 6,369 vertex coordinates, including both ends of each UV axis. Integer remainder and quotient give the original numerators, which are divided by the original grid dimensions. The retained dependency proof makes the local point identical for every row. The selected row supplies the current pose. Camera and shadow replay the same indexed indirect arguments.

Predict 14 → 13 actual pipeline creates, eight shader modules, 17 commands per frame, and one → zero grid load dispatches at both counts. Each eligible kind drops one storage buffer (101,904 bytes for Hermit), two bind groups, two bind-group layouts for the single-grid scene, and the bake compute pipeline. The index buffer stays at 147,456 bytes. Expect cold ≤1.5 s and warm ≤0.060 s with a distinct nonce per pair, and p95 ≤5.0/5.6 ms at 1280×800 over 300 orbit frames after 30 warmups. The six C1 beauty comparisons predict zero differing RGBA pixels for beauty, depth and ID at 0, 4 and 9 s for both counts. Overdraw, an authored row-dependent surface, and archived surface modules use their corner entry.

## Delivery

The eligibility proof and corner shader stay in the emitted module. The new `vs_grid` entry consumes the original indices and evaluates the authored surface using the visible row. Native loading selects that entry for eligible surface draws. Archived modules with the baked `vs_indexed` entry use their original `vs` entry and its frame and kind bindings. Overdraw uses its existing corner path.

Actual pipeline telemetry records **14 → 13** creates at both counts, **8 modules**, and **17 frame commands**. The removed label is `HermitShell bake surface`. Grid load dispatches drop from one to zero. Production changes remove **48 physical lines net**. Each Hermit setup removes one 101,904-byte GPU point buffer, two bind groups, two bind-group layouts, one compute pipeline and its pipeline layout. It also removes one load command encoder, command buffer and submission. The 147,456-byte index buffer remains. The reconstructed index stream covers all 6,369 vertices and preserves all 36,864 corners and their order. [Main create telemetry](../../../out/meta/c1/grid.log) · [Lane load traces](../../../out/meta/c1a/score.json)

All **18** lossless 1280×800 beauty, depth and ID comparisons against matched main were exact at counts 1 and 20 and times 0, 4 and 9 s. At 0 s, the indexed and forced-corner paths also matched in all six count/AOV cases. The archived count-20 program loaded with 13 creates, drew 17 commands, and matched the baked main beauty frame. The row-dependent surface, frozen mesh control and Hermit-20 overdraw frame each matched main exactly. The textured procedural bake control still issued 20 load dispatches and reported 8/3/3/8/3 commands across its dirty-clock sequence. [RGBA hashes and difference counts](../../../out/meta/c1a/score.json) · [Lossless stills](../../../out/meta/c1a/images) · [Corner and archive comparison](../../../out/meta/c1a/grid/hermit-grid.json) · [Procedural bake trace](../../../out/meta/c1a/bake/bake.json)

## Matched timing

Apple M4 Metal at 1280×800, 30 warmup frames then 300 orbit frames, using the ordinary `loft profile` timestamp ring. All 300 frames of each run issued 17 commands. In the first batch, triangle-control medians were 0.096/0.096 ms (main/lane at count 1) and 0.032/0.030 ms (count 20). All fell below the gate's 0.35 ms contention threshold. Process snapshots record simultaneous compiler, grammar and desktop activity.

| Pass | Main 1 p50 / p95 ms | Lane 1 p50 / p95 ms | Main 20 p50 / p95 ms | Lane 20 p50 / p95 ms |
| --- | ---: | ---: | ---: | ---: |
| Cull | .004 / .007 | .004 / .005 | .005 / .006 | .005 / .010 |
| Visibility | .608 / .846 | .624 / .735 | 1.765 / 2.126 | 1.644 / 2.132 |
| Shadow | .520 / .686 | .534 / .671 | .814 / 1.394 | .736 / 1.155 |
| Classify | .010 / .183 | .004 / .171 | .000 / .371 | .000 / .185 |
| Shade | 1.790 / 2.297 | 1.828 / 2.159 | 2.736 / 3.958 | 2.510 / 3.408 |
| Present | .156 / .317 | .152 / .256 | .210 / .402 | .163 / .328 |
| Whole frame | 3.111 / 4.591 | 3.177 / 3.602 | 5.702 / 7.099 | 5.125 / 7.215 |

The ordinary `loft load` timer includes the first frame, uses the shared derivation cache, and forces a cold driver compile by changing the nonce. The focused Load gate instrument uses a fresh derivation cache for each pair and stops before drawing. Each pair shares a nonce, and different pairs use distinct nonces.

| Count | Main CLI cold / warm s | Lane CLI cold / warm s | Lane gate-style cold / warm s |
| ---: | ---: | ---: | ---: |
| 1 | 2.806 / .074 | 2.353 / .085 | 3.954 / .050 |
| 20 | 4.650 / .069 | 2.192 / .080 | 3.058 / .058 |

Initial CLI cold and warm loads exceed the 1.5 s and 0.060 s caps. Initial gate-style warm loads meet the cap, with 11 derivation hits at each count. Gate-style cold load exceeds the cap at both counts. Historical main gate-style cold/warm was 1.152/.024 s and 1.123/.024 s at counts 1 and 20. [Historical gate timing](../../../out/meta/del2/five-gates.json) · [Initial measured timing and exact commands](../../../out/meta/c1a/score.json) · [Command ledger](../../../out/meta/c1a/commands.jsonl)

The first timing repeat reversed each adjacent profile pair. At count 1 it measured 3.581/4.362 ms p95 (main/lane, cap 5.0 ms), with triangle control .043/.051 ms. At count 20 it measured 5.861/5.469 ms p95 (cap 5.6 ms), with control .096/.097 ms. Every lane count-1 run met the cap. The first lane count-20 run missed, and the repeat met it. The repeat's full per-pass p50/p95 and raw frame stamps are preserved with the initial runs. [Both timing batches](../../../out/meta/c1a/timing-summary.json)

The all-target check and single optimized build exited zero. All 114 tests passed across the 14 Cargo-emitted test executables, each launched from its crate directory under the GPU lock. The combined workspace suite and five-gate verdict belong to the integrator. Non-Metal devices, portable GPU execution, and archives beyond the captured v1 Hermit program were unmeasured. [Build, flags and diagnostics](../../../out/meta/c1a/commands-build.txt) · [Cargo artifacts](../../../out/meta/c1a/build.jsonl) · [Test results](../../../out/meta/c1a/gpu-batch.log)


## ESCALATE · Cold load and CLI warm load

Tested code tree `bbac856d5bf7ee03494158fd55b754bf69cf5484` on base `173d6e9a2b4fa36fcb45c0957f75e9c44f8e6dfa`. The second and final timing repeat ran `loft load engines/hermit.loft --set count=1|20 --nonce N` and `phase4_gate hermit-load OUT COUNT NONCE` under `tools/gpu_lock.sh`, with 1280×800 Apple M4 Metal frames. The exact paths, nonces, order, exits and process samples are in the command ledger. Prediction was ≤1.5 s cold and ≤0.060 s warm.

| Count | Main CLI cold / warm s | Lane CLI cold / warm s | Lane gate-style cold / warm s |
| ---: | ---: | ---: | ---: |
| 1 | 3.374 / .093 | 2.851 / .070 | 3.781 / .043 |
| 20 | 4.551 / .086 | 4.350 / .102 | 4.371 / .049 |

The smallest discriminating observation is the gate-style create trace. It records 13 actual creates and zero load-bake dispatches on every repeat. On the final cold runs, one existing visibility pipeline (`CrabFace vis`, count 1) took 3.413 s and one existing shade pipeline (`CrabLeg shade`, count 20) took 4.091 s. Both exceeded the entire cold budget alone. Matched main also missed every CLI load cap. The CLI warm readings include drawing and waiting for the first frame. All six lane gate-style warm measurements passed. The two timing reruns are exhausted. [Final command ledger](../../../out/meta/c1a/timing-rerun-2/commands.json) · [All timing and actual create records](../../../out/meta/c1a/timing-summary.json)

The lane's static check output is preserved with the build and test evidence. [Static check log](../../../out/meta/c1a/static.log)
