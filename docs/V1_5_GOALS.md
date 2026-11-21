# Loft v1.5: goals and checkable targets (set 20 Nov 2026)

These come from the lead's critique (`docs/handoff/2026-11-20_critic/00_CRITIC.md`) and the chief scientist's reply
(`agent_notes/2026-11-20_v1/scientist_round1_reply.md`). Each target has a number or a yes/no, and a way to check it.
The order is the scientist's. Update the Status column as lanes land. Never edit a target to make it pass: change it
only with a dated note giving the reason.

**Measurement rule for every speed target:** paired, same-run A/B with the GPU uncontended. The triangle control
must be under 0.15 ms p95 and there must be no builds running during timing. Report p95 with a 95% confidence
interval. Numbers taken under contention don't count.

The 21 November [integration scoreboard](../agent_notes/2026-11-20_v1/V1_5_CLOSEOUT.md) identifies the tested revision, branch decisions and evidence for each number. Both grammar recognizers passed 311/311 expected cases on merged main. Named lane measurements were collected before integration.

## P0: fix the heavy-lighting regression (critique #1 / scientist #11)

| ID | Target | Check | Status |
|---|---|---|---|
| G1.1 | `shade_tiles` exposes `(hit, px, world)` from the raster visibility buffer, plus optional MSAA/subpixel visibility, so RT shadows, one-bounce GI and ReSTIR run on top of the vis-buffer and never replace it with full-screen software primary rays | `engines/realtime_matched` uses it, with no 4-tap primary-ray `screen_pass` | Unified runtime. Matched B uses Vis64 and hardware secondary traversal in five passes. B-20 differed from the software render at 1,852/1,024,000 pixels above 2/255. |
| G1.2 | Matched heavy-lighting A/B, 20 groups: new ≤ old p95 | AB_OLD_VS_NEW.md §B rerun uncontended. Was 56–58 ms new against 26–29 ms old, under contention | v15-exec measured B-20 at 10.523 ms versus 14.583 ms legacy. Candidate p95 CI [10.253, 10.979] ms. Triangle 0.118/0.066 ms before/after. |
| G1.3 | Matched light-lighting A/B, re-measured uncontended: new ≤ old | §A rerun | Open. No admitted A-20 pair in the Wave 15-2 evidence. Historical contended results were 10.465 ms new and 11.000 ms old. |

## P1: modern GPU physics (critique #8 / scientist #8)

| ID | Target | Check | Status |
|---|---|---|---|
| G2.1 | `kernel ... allow(atomics, workgroup, subgroups, dynamic_loops)` lowers for any kernel name, not only `deposit_caustic` | A kernel test suite; no E405 for legal kernels | Unified runtime. Arbitrary named kernels exported, integer/float readbacks were 36/18, and the StudyInk binding passed native and bundle replay. |
| G2.2 | A sanctioned **NonDet** physics tier outside the CPU twin, with statistical twin checks; **Det** stays for gameplay and rollback rigs | The spec's lead ruling plus a NonDet example scene | Unified runtime. Det stepping rejects NonDet laws. v15b recorded maximum error 2.69e−7 across 12 observations against 3e−5. |
| G2.3 | Fixed-point deterministic P2G scatter and a float-atomic fast tier, both measured on M4 | A P2G probe: particles/ms per tier | cap1b, Apple M4. Spread fixed8 and native f32 deposits processed 66,000 and 66,300 particles/ms. All 128 f32 trials met the error bounds. |
| G2.4 | One physics showcase (MPM or FLIP splash) running interactively | Frame p95 ≤ 16 ms at 1280×800 | cap1b measured 7.054 ms GPU p95 for the 65,536-particle, 27-tap FLIP splash at 1280×800. Triangle p95 0.099/0.110 ms. |

## P2: variety through data-driven materials (critique #4 / scientist #7)

| ID | Target | Check | Status |
|---|---|---|---|
| G3.1 | A data-driven uber-kind: one compiled PBR/stylised pipeline reading a packed `MaterialTable` plus a texture atlas or bindless array indexed by `material_id`, beside compiled custom kinds | A 200-material scene builds with **≤ 3 pipelines** for materials | Held on cap1c. cap1c measured two material shade pipelines for the 200-material PBR/ink fixture. Six authored families use packed 64-byte rows. |
| G3.2 | 1,000 unique materials: cold load ≤ 1 s, frame p95 within 10% of a 10-material scene | Paired measurement | Held on cap1c. cap1c measured 773–827 ms cold and frame p95 of 6.474 ms at 1,000 rows versus 6.938 ms at 10. Paired delta CI [−0.647, −0.192] ms. |
| G3.3 | Live material edits (parameter rows) at 0 ms compile | `set_param`/row upsert, with no pipeline creation | Held on cap1c. cap1c observed zero pipeline creations on a 64-byte row upload. Separate 640×400 render changed 145 pixels confined to the selected row. |

## P3: picture gates that allow stochastic and temporal rendering (critique #7, #9 / scientist #9)

| ID | Target | Check | Status |
|---|---|---|---|
| G4.1 | **Deterministic mode:** ≤1/255 drift allowed on <0.1% of pixels for perf/compiler changes, with no structural change | Gate implementation and a mutant that exceeds it | The production Rust scorer accepted 19/19 ordered candidate frames against incoming main. IDs and D32 matched in all 19. The largest difference was seven pixels at one color code. The 3,000-pixel boundary controls passed. |
| G4.2 | **Stochastic/temporal mode:** a seeded error metric (PSNR or FLIP) against a converged reference, plus a temporal-stability check | A path-traced scene passes; a broken estimator fails | Four seeded candidate frames scored 41.725–42.069 dB with temporal variance 0.0000449137. The gain mutant failed at 6.744 dB. The variance cap is 0.0025. |
| G4.3 | Quiet-GPU timing: conductors never time while builds run; Speed and Load accept only paired confidence intervals | The same code scores the same verdicts in three runs | Open. The release timing collector completed 1/6 pairs. The next candidate triangle measured 0.3745 ms against a strict 0.150 ms limit. Speed and Load have no paired 95% verdict. The first pair exposed a reproducible Hermit resource regression. |

## P4: core size and escape hatches (critique #2, #3)

| ID | Target | Check | Status |
|---|---|---|---|
| G5.1 | Rust ≤ 18,000 lines (lean1 ceilings), enforced by `tools/check.sh lines` | Was 33,151 on 20 Nov | Miss. The reduced tree contains 37,984 Rust code lines, 19,984 above budget. The tested candidate with v15-perf contained 38,249. |
| G5.2 | loft-std has no wgpu code; the loft-cli duplicate renderer is deleted | grep plus lines | Unified executor. loft-std has zero wgpu dependencies across targets and features. The CLI native subtree has zero Rust code lines. |
| G5.3 | Whole raw WGSL `pass` with custom bindings works from `.loft` | s1-escapes merged | Unified runtime. Raw binding readbacks [21, 33], logical storage ranges, and native versus decoded-bundle StudyInk pixels verified in v15-exec. |
| G5.4 | `msl(...)` passthrough with Metal slots auto-generated from PassDecl.bindings | bk1b design → lane | Held on v15-native. v15-native sparse-slot witness returned [29, 1] on Metal and [29, 0] through the web-baseline WGSL fallback. |
| G5.5 | Whole-pass parity gate within 5% of hand-written controls (Hermit Shade, Shrine Vis) | s1-controls-fix merged | Control gate merged at `097dfae`. Shrine visibility IDs matched. Hermit HDR parity and the 5% timing target remain open. |

## P5: language against bloat (critique #5)

| ID | Target | Check | Status |
|---|---|---|---|
| G6.1 | Corpus median ≤ 10,000 tokens, with pictures matched | Was 96,204 (tok1) | 187 live sources. Median 3,983, p95 7,340 and total 786,454 o200k tokens. v15-bloat verified 854/854 ordered corpus frames. |
| G6.2 | A per-scene token budget gate and a repeated-block lint | Fails a scene with unrolled rows | v15-bloat verified 170/170 regenerated scenes under strict lint. Unrolled seed exits 7 in strict mode and 0 in advisory mode. |
| G6.3 | stdlib/ tree with styles, integrators, schedules and engines; copy-paste forking documented | std1a plus the port lanes | Partial. Geometry, noise and gait ports are on main. The style, transport and flagship ports remain on v15-stdlib-finish. |
| G6.4 | A written **style↔integrator contract**: what a style may read and write, per integrator class | A doc plus woodblock running under realtime and reference | Held on v15-stdlib-finish. Seven Harbor style swaps preserved Vis64/D32. At 256 spp, 5,120 of 16,000 Harbor HDR pixels were unwritten. |

## P6: motion (critique #8)

| ID | Target | Check | Status |
|---|---|---|---|
| G7.1 | Authored solvers agree with the CPU twin (Det tier) with FMA kept on in shading, plus an automated MSL leak check | x2b merged, with a 1,000-crab dispatch ≤ 0.15 ms | Held on v15-perf. All three rigs matched 481/481 ticks in the release candidate. Its WGSL/MSL leak check passed. The lane measured 0.02767 ms p95 for 1,000 crabs. |
| G7.2 | The 5-crab lineup dance and crab_live ported | port1 merged | Held with v15-stdlib-finish. Both recognizers passed its 379 tracked sources, Crab Live matched 480/480 ticks and 30 approval still hashes held. Look approvals are outstanding. |

## P7: platform and capability (scientist §2)

| ID | Target | Check | Status |
|---|---|---|---|
| G8.1 | Inline RAY_QUERY wired into `World` on native (RT shadows, reflections, one-bounce GI) | A mesh scene with RT shadows | Unified runtime. Native World uses Metal AABB traversal and authored intersections. B-20 measured 10.523 ms p95. The dedicated mesh-shadow acceptance case remains open. |
| G8.2 | Web tier packs tables into fewer storage buffers (8-buffer cap) | spectral_caustic_study runs on web | Open for spectral_caustic_study on web. cap1b verified 1,572,864 Chrome grid words for the particle fixture. The browser timing window failed admission. |
| G8.3 | MetalFX temporal upscaling via `PassRun::HostCallback` plus `wgpu_hal::metal` interop, with no native backend | Upscaled frame and measured ms | Held on v15-native. v15-native measured temporal MetalFX at 5.319 ms p95 versus 8.891 ms full resolution, 640×400 to 1280×800, at 42.161 dB against full resolution. |
| G8.4 | Native Metal backend: **deferred.** Build it only if Loft commits to RT pipelines or matrix-core neural rendering | A decision note | decided: deferred |
| G8.5 | Decide native-first or web-first | A decision note | Decided. Native-first with a strict web baseline under R6. |

## Branches carried into v1.5

| Branch | Integration status |
|---|---|
| loft/x2a | Folded into v15-exec. The executor is shared, per-frame uploads use the staging belt, and loft-std has zero wgpu dependencies. |
| loft/x2b | Retained. The motion-only barrier supersedes soft-float and reaches 0.02767 ms. The separate Det verifier and bounded-loop regression are outside the merged ancestry. |
| loft/x2c | Its investigation and the v15-perf cube binding are included. The lane measured Hermit-20 at 3.569 ms p95. |
| loft/s1-escapes | The v15b work is in the unified runtime. StudyInk passed native versus decoded-bundle pixel parity. |
| loft/s1-controls-fix | **Merged** (`tools/check.sh parity`, Shrine 20→0 IDs; Hermit HDR mismatch still tracked) |
| loft/pf1b | **Merged** (36/36 + 27/27 bit-exact; −0.30 ms Shrine, −0.11 ms Hermit) |

## Deferred (lead, 20 Nov 14:50)
- stdaot1 (std AOT and the pipeline cache): std parsing is a few ms. The pipeline cache comes back with the cold-load work.
- std1f (corpus regeneration): after the stdlib port lands.

## Round 1 decisions (chief scientist's answers, 20 Nov 15:01: agent_notes/2026-11-20_v1/scientist_round1_answers.md)

These supersede the rows above where they differ.

| ID | Decision / new target | Replaces | Status |
|---|---|---|---|
| R1 | **P0 order:** (1) `shade_tiles` exposes (hit, px, world) plus optional 4× MSAA vis-buffer, AND `EXPERIMENTAL_RAY_QUERY` is wired into `World` (matched Table B 58 → < 15 ms uncontended, predicted); (2) finish the x2c sky bake (the Shade pipeline lacks a cube binding): Hermit-20 3.75 → ~2.5 ms; (3) Shrine CoralBranch cluster culling, −0.25 to −0.45 ms. **Do not tune BSDF ALU or march steps now.** | G1, G8.1 | B-20 passed at 10.523 ms. Held v15-perf. Its lane measured Hermit-20 at 3.569 ms versus 3.666 ms and Shrine at 1.728 ms versus 1.807 ms. The release raised Hermit modules from 8 to 9 and pipelines from 14 to 15 against budgets of 8 and 13. |
| R2 | **Core ceilings:** wave 1 ≤ 18,000 scc lines (loft-std has 0 wgpu deps, loft-runner ≤ 2,200, loft-cli/native ≤ 450); wave 2 ~14,500–15,500 after schedules and BSDFs move to `.loft`. **Sequencing:** first ONE owner merges one execution path plus a wgpu-free loft-std (deleting ~4k lines); then a `tools/check.sh lines` ratchet. Never run 4 parallel lean lanes again. The compiler owns parse/check → typed HIR, AD and Lipschitz, Table+Kernel+Pass lowering, and bundles. The library owns all rendering, lighting, schedule and motion recipes. | G5.1, G5.2 | Miss. Reduced tree 37,984 (+19,984), runner 2,178/2,200, CLI native 0/450 and loft-std wgpu dependencies 0. Incoming main had 36,303 core lines. The release candidate with v15-perf had 38,249. |
| R3 | **Variety:** 6–10 material-family uber-shaders (StandardPbr, LayeredCoat, StylizedInk, DielectricGlass, Emissive, Foliage) over a packed `MaterialTable` (64–128 B rows) plus bindless textures, at 0 ms compile per material; compiled kinds plus a pipeline cache stay for novel maths. **Cut** compiler auto-merge. **Reject** a material VM for shading: Tier 0 is for live geometry previews only. | G3 | Held on cap1c. cap1c implements six authored families and 64-byte rows. The 200-material fixture used two shade entries. Compiled BSDF calls select families explicitly. Tier 0 remains a geometry evaluator. |
| R4 | **Two style contracts:** `fn material(hit) -> Bsdf` (works under every physical integrator) and `fn shade_style(hit, light: LightSummary) -> rgb`. LightSummary carries key_dir, key_vis, key_radiance, indirect_irradiance, ao, footprint_m, depth_m and id. An integrator that doesn't provide LightSummary refuses a Style at compile time (for example woodblock with spectral_caustic). `footprint_m` replaces fragment-only dFdx/dFdy. | G6.4 | Held. v15-stdlib-finish implements both contracts and rejects a LightSummary mismatch. Reference depth passed at 64/128/256 spp. Harbor and area-light beauty left 5,120 and 896 HDR pixels unwritten. |
| R5 | **Bloat is a data-serialisation bug** (95.7% of corpus tokens are baked float arrays). (a) `import_table` for binary sampled data; (b) regenerate the corpus from its procedural seeds with `build for` and row defaults (median → ~1.5–4k tokens); (c) fix ad.rs row-constant parameters (−112k tokens); (d) token caps go in `loft lint` and corpus CI only, **not compile errors**. | G6.1, G6.2 | Binary import_table and build loops cover 170 regenerated ports. Corpus median 3,983. Lane AD control saved 111,792 source tokens. Strict budgets run in lint and corpus CI. |
| R6 | **Native-first,** with a strict `HostCaps::WEB_BASELINE` tier and no parity promise between native-high and web. **A 3-rung wgpu bridge** (~600–900 LOC): (i) `EXPERIMENTAL_RAY_QUERY` in World (verified on M4 in `cap1`), (ii) `EXPERIMENTAL_COOPERATIVE_MATRIX` (8×8×8 f32/f16 verified on M4 in `cap1`) plus `PASSTHROUGH_SHADERS` MSL for `bfloat`/unchecked loops, (iii) `MetalFX` via `HostCallback` + `wgpu_hal::metal`. No `objc2-metal` backend unless we commit to SBT RT pipelines, mesh shaders or multi-queue. | G8.4, G8.5 (decided) | Rung 1 on main at 10.523 ms p95 for B-20. Held rungs 2–3 on v15-native: cooperative f32 0.528 ms p50 versus 1.022 ms ALU, MetalFX 5.319 ms p95 versus 8.891 ms. Its runner counts 2,248/2,200 lines. |
| R7 | **x2b:** replace the 900-line u32 soft-float with a motion-only FMA contraction barrier; 1,000-crab dispatch ≤ 0.06 ms (predicted); shading stays in MTLMathModeFast. | G7.1 | Held with v15-perf. Its 149-line motion barrier passed the release WGSL/MSL leak test. The lane measured 0.02767 ms p95, candidate CI [0.02742, 0.02783] ms. |
| R8 | **The two-tier picture gate lives in code** (crates/loft-cli/src/gates.rs), as `Deterministic(tol)` and `Stochastic(PSNR/FLIP + temporal)`. | G4 (lane v15c) | Implemented in Rust. Release candidate passed 19/19 deterministic frames and 4/4 stochastic frames. The deliberately broken estimator failed at 6.744 dB. |
| R9 | **One conductor:** a central concurrency pool plus a quiet-GPU timing lock that blocks builds and other lanes during Speed and Load scoring. | G4.3 | The conductor reserves both build slots and the GPU. The release collected one pair with triangle p95 of 0.100833/0.109 ms. The next arm hit 0.3745 ms and invalidated the window. Three independent quiet verdicts remain open. |

## Execution waves at the 20 November close (historical sequencing)

IX4 scored `3c24a11`. Pictures held 19/19, Twin 36/36, Speed 6/12, Load 23/33 and Truth 34/34. All six latency cases were contended. Wave 15-2 defaults to one step and checks load and STALL before each lane. [IX4 audit](../agent_notes/2026-11-20_v1/OPEN.md#wave-x4-integration)

| Wave | Lanes | Covers | Sequencing |
|---|---|---|---|
| **Infra (standalone)** | `build1`, `cam1` | Cambium real `cargo-deps` seed + loud cold-seed / harvest fixes | Loft seed merged at `41c1f4d`. Cambium seed fixes published on release/v0.10. |
| **wave4** (`ix4` done `2f809a4`) | `std1b` + `std1e` **merged**; `std1c`, `std1d`, `port1` held for `v15_stdlib_finish` | G6.3, G6.4/R4, G7.2 | Done (`ix4.done`) |
| **wave15** (`iv15` done `47c4fec`) | `v15a`, `v15b`, `v15c` finished & audited; 3 narrow cross-crate deltas folded into `v15_exec` | G1, G4, G7.2, G8.1, R1 #1, R8, R9 | Done (`wave15.finished`) |
| **Anti-freeze (`final.sh`)** | `cl1`, `fs1`, `cam2`, `fs2`, `wrap1` | macOS 15.5 `proc_find` anti-freeze across `cleanup_tools`, `cambium`, `freeze_sampler` | freeze_sampler and Cambium pushed. cleanup_tools awaits a remote and reconciliation of local edits. |
| **wave15_2 phase 1** (`wave15_2.sh`) | **`v15_exec`** (solo!) | **R2** + held `v15a`/`v15b`/`v15c` deltas (`EXPERIMENTAL_RAY_QUERY` bindings, StudyInk binding, Rust `gates.rs`) | Merged into main at `5e252da`. |
| **wave15_2 phase 2** (`wave15_2.sh`) | `v15_stdlib_finish` (`std1d`+`std1c`+`port1`+R4), `v15_perf` (R1 #2–3 + R7), `v15_native` (R6 rungs 2–3), `cap1c` (R3 uber-families), `v15_bloat` (R5 corpus/lint), `cap1b` (G8.2/G2 P2G) → `iv15_2` | R1 #2–3, R3, R4, R5, R6 rungs 2–3, R7, G2, G5.4, G6.4, G7.2, G8.2, G8.3 | IV15-2 admits perf, cap1b and bloat. Stdlib, cap1c and native remain held. See the 21 November scoreboard. |
