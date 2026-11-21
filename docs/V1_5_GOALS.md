# Loft v1.5: goals and checkable targets (set 20 Nov 2026)

These come from the lead's critique (`docs/handoff/2026-11-20_critic/00_CRITIC.md`) and the chief scientist's reply
(`agent_notes/2026-11-20_v1/scientist_round1_reply.md`). Each target has a number or a yes/no, and a way to check it.
The order is the scientist's. Update the Status column as lanes land. Never edit a target to make it pass: change it
only with a dated note giving the reason.

**Measurement rule for every speed target:** paired, same-run A/B with the GPU uncontended. The triangle control
must be under 0.15 ms p95 and there must be no builds running during timing. Report p95 with a 95% confidence
interval. Numbers taken under contention don't count.

## P0: fix the heavy-lighting regression (critique #1 / scientist #11)

| ID | Target | Check | Status |
|---|---|---|---|
| G1.1 | `shade_tiles` exposes `(hit, px, world)` from the raster visibility buffer, plus optional MSAA/subpixel visibility, so RT shadows, one-bounce GI and ReSTIR run on top of the vis-buffer and never replace it with full-screen software primary rays | `engines/realtime_matched` uses it, with no 4-tap primary-ray `screen_pass` | Branch only (v15a). Matched B uses the Vis64 hit, pixel and World in five passes, with four samples at edges. B-1/B-20 had 27/483 pixels above 2/255 per 1,024,000. Secondary rays use the software BVH. |
| G1.2 | Matched heavy-lighting A/B, 20 groups: new ≤ old p95 | AB_OLD_VS_NEW.md §B rerun uncontended. Was 56–58 ms new against 26–29 ms old, under contention | Open. B-20 was 58.269 ms new vs 26.608 ms old on the historical contended run. Neither v15a preflight admitted a new pair (triangle p95 5.621/7.051 ms, limit <0.150 ms). |
| G1.3 | Matched light-lighting A/B, re-measured uncontended: new ≤ old | §A rerun | Open. Both A frames and programs matched main. Neither preflight admitted a timing pair. Historical contended A-20 was 10.465 ms new vs 11.000 ms old. |

## P1: modern GPU physics (critique #8 / scientist #8)

| ID | Target | Check | Status |
|---|---|---|---|
| G2.1 | `kernel ... allow(atomics, workgroup, subgroups, dynamic_loops)` lowers for any kernel name, not only `deposit_caustic` | A kernel test suite; no E405 for legal kernels | Branch only (v15b). An arbitrary typed kernel went from E405 to bundle export. GPU readbacks were integer 36 and float 18. StudyInk blocks the lane. |
| G2.2 | A sanctioned **NonDet** physics tier outside the CPU twin, with statistical twin checks; **Det** stays for gameplay and rollback rigs | The spec's lead ruling plus a NonDet example scene | Branch only (v15b). Exact stepping refused its NonDet law. Across 12 GPU observations the maximum error was 2.69e−7 (cap 3e−5). The tier ruling and scatter example are on the branch. |
| G2.3 | Fixed-point deterministic P2G scatter and a float-atomic fast tier, both measured on M4 | A P2G probe: particles/ms per tier | Open. v15b measured 3 fixed-point vs 1 float dispatch for 256 particles across 32 seeds. Maximum errors were 7.705e−6 and 1.080e−6. Particles/ms and deterministic P2G remain unverified. |
| G2.4 | One physics showcase (MPM or FLIP splash) running interactively | Frame p95 ≤ 16 ms at 1280×800 | Open. v15b exercised 256-particle scatter across 32 seeds. An interactive MPM or FLIP frame time remains unmeasured. |

## P2: variety through data-driven materials (critique #4 / scientist #7)

| ID | Target | Check | Status |
|---|---|---|---|
| G3.1 | A data-driven uber-kind: one compiled PBR/stylised pipeline reading a packed `MaterialTable` plus a texture atlas or bindless array indexed by `material_id`, beside compiled custom kinds | A 200-material scene builds with **≤ 3 pipelines** for materials | open (after cap1) |
| G3.2 | 1,000 unique materials: cold load ≤ 1 s, frame p95 within 10% of a 10-material scene | Paired measurement | open |
| G3.3 | Live material edits (parameter rows) at 0 ms compile | `set_param`/row upsert, with no pipeline creation | open |

## P3: picture gates that allow stochastic and temporal rendering (critique #7, #9 / scientist #9)

| ID | Target | Check | Status |
|---|---|---|---|
| G4.1 | **Deterministic mode:** ≤1/255 drift allowed on <0.1% of pixels for perf/compiler changes, with no structural change | Gate implementation and a mutant that exceeds it | Branch only (v15c). Ordered captures matched RGBA, ID and D32 on 19/19 frames. On a 3,000-pixel control, 2 one-code changes passed, 3 failed, and 1 two-code change failed. |
| G4.2 | **Stochastic/temporal mode:** a seeded error metric (PSNR or FLIP) against a converged reference, plus a temporal-stability check | A path-traced scene passes; a broken estimator fails | Branch only (v15c). Four seeded frames scored 41.73–42.07 dB with variance 4.491e−5. The gain mutant fell to 6.74 dB and the flicker mutant reached 0.005249 variance (cap 0.0025). |
| G4.3 | Quiet-GPU timing: conductors never time while builds run; Speed and Load accept only paired confidence intervals | The same code scores the same verdicts in three runs | Open. v15c rejected a build overlap (exit 6). Six Capsules-1 pairs gave a p95 delta CI of [−1.388, +1.013] ms. Three replays scored the same samples. The guard scores p50 at 0.350 ms, while acceptance requires p95 <0.150 ms. |

## P4: core size and escape hatches (critique #2, #3)

| ID | Target | Check | Status |
|---|---|---|---|
| G5.1 | Rust ≤ 18,000 lines (lean1 ceilings), enforced by `tools/check.sh lines` | Was 33,151 on 20 Nov | Open. IX4 counted 34,186 Rust code lines across nine crates with scc, excluding tests directories. |
| G5.2 | loft-std has no wgpu code; the loft-cli duplicate renderer is deleted | grep plus lines | Open. loft-std still uses wgpu. The single execution path is assigned to v15_exec. |
| G5.3 | Whole raw WGSL `pass` with custom bindings works from `.loft` | s1-escapes merged | Branch only (v15b). Raw slots read back [21, 33] from a 15,245-byte bundle. A two-word table exposed three words to arrayLength. StudyInk failed group 3 binding 3. |
| G5.4 | `msl(...)` passthrough with Metal slots auto-generated from PassDecl.bindings | bk1b design → lane | open |
| G5.5 | Whole-pass parity gate within 5% of hand-written controls (Hermit Shade, Shrine Vis) | s1-controls-fix merged | Control gate merged at `097dfae`. Shrine visibility IDs matched. Hermit HDR parity and the 5% timing target remain open. |

## P5: language against bloat (critique #5)

| ID | Target | Check | Status |
|---|---|---|---|
| G6.1 | Corpus median ≤ 10,000 tokens, with pictures matched | Was 96,204 (tok1) | deferred to a later v1.5 wave (std1f brief ready) |
| G6.2 | A per-scene token budget gate and a repeated-block lint | Fails a scene with unrolled rows | open |
| G6.3 | stdlib/ tree with styles, integrators, schedules and engines; copy-paste forking documented | std1a plus the port lanes | Partial. std1a merged at `f8fc612`. IX4 merged std1b geometry/noise and std1e gait. Styles, transport and flagship ports remain on candidate branches. |
| G6.4 | A written **style↔integrator contract**: what a style may read and write, per integrator class | A doc plus woodblock running under realtime and reference | Held. The Harbor woodblock needs an adapter, and the 128/256-sample reference returned NaN depth. See R4. |

## P6: motion (critique #8)

| ID | Target | Check | Status |
|---|---|---|---|
| G7.1 | Authored solvers agree with the CPU twin (Det tier) with FMA kept on in shading, plus an automated MSL leak check | x2b merged, with a 1,000-crab dispatch ≤ 0.15 ms | Open. std1e passed 18 CPU/Metal gait comparisons. The x2b contraction barrier, MSL leak check and dispatch budget still need verification. |
| G7.2 | The 5-crab lineup dance and crab_live ported | port1 merged | Held on port1. Both recognizers accepted 4/7 files. Lineup, Hermit and lineup cast failed. Five look pairs await lead review. Crab Live measured 15.62 ms p95 against 9.05 ms legacy. |

## P7: platform and capability (scientist §2)

| ID | Target | Check | Status |
|---|---|---|---|
| G8.1 | Inline RAY_QUERY wired into `World` on native (RT shadows, reflections, one-bounce GI) | A mesh scene with RT shadows | Open. v15a runs its shadow and bounce through the software BVH. Zero hardware ray-query paths landed. B-20 has no uncontended timing. |
| G8.2 | Web tier packs tables into fewer storage buffers (8-buffer cap) | spectral_caustic_study runs on web | open |
| G8.3 | MetalFX temporal upscaling via `PassRun::HostCallback` plus `wgpu_hal::metal` interop, with no native backend | Upscaled frame and measured ms | open |
| G8.4 | Native Metal backend: **deferred.** Build it only if Loft commits to RT pipelines or matrix-core neural rendering | A decision note | decided: deferred |
| G8.5 | Decide native-first or web-first | A decision note | Decided. Native-first with a strict web baseline under R6. |

## Pending branches to finish and merge (carried into v1.5)

| Branch | What remains |
|---|---|
| loft/x2a | Execution path: per-frame allocations; loft-std still uses wgpu; execution code 6,717 lines |
| loft/x2b | Twin FMA fix: 1,000-crab dispatch over budget; MSL leak guard missing |
| loft/x2c | Sky bake: shading cannot sample the bake (no binding) |
| loft/s1-escapes | Kernel allow, whole-pass `pass`, bake sampling, unproved: StudyInk integration incomplete |
| loft/s1-controls-fix | **Merged** (`tools/check.sh parity`, Shrine 20→0 IDs; Hermit HDR mismatch still tracked) |
| loft/pf1b | **Merged** (36/36 + 27/27 bit-exact; −0.30 ms Shrine, −0.11 ms Hermit) |

## Deferred (lead, 20 Nov 14:50)
- stdaot1 (std AOT and the pipeline cache): std parsing is a few ms. The pipeline cache comes back with the cold-load work.
- std1f (corpus regeneration): after the stdlib port lands.

## Round 1 decisions (chief scientist's answers, 20 Nov 15:01: agent_notes/2026-11-20_v1/scientist_round1_answers.md)

These supersede the rows above where they differ.

| ID | Decision / new target | Replaces | Status |
|---|---|---|---|
| R1 | **P0 order:** (1) `shade_tiles` exposes (hit, px, world) plus optional 4× MSAA vis-buffer, AND `EXPERIMENTAL_RAY_QUERY` is wired into `World` (matched Table B 58 → < 15 ms uncontended, predicted); (2) finish the x2c sky bake (the Shade pipeline lacks a cube binding): Hermit-20 3.75 → ~2.5 ms; (3) Shrine CoralBranch cluster culling, −0.25 to −0.45 ms. **Do not tune BSDF ALU or march steps now.** | G1, G8.1 | Item 1 held on v15a. B-20 had 483/1,024,000 pixels above 2/255. Hardware traversal and the <15 ms B-20 result are outstanding. |
| R2 | **Core ceilings:** wave 1 ≤ 18,000 scc lines (loft-std has 0 wgpu deps, loft-runner ≤ 2,200, loft-cli/native ≤ 450); wave 2 ~14,500–15,500 after schedules and BSDFs move to `.loft`. **Sequencing:** first ONE owner merges one execution path plus a wgpu-free loft-std (deleting ~4k lines); then a `tools/check.sh lines` ratchet. Never run 4 parallel lean lanes again. The compiler owns parse/check → typed HIR, AD and Lipschitz, Table+Kernel+Pass lowering, and bundles. The library owns all rendering, lighting, schedule and motion recipes. | G5.1, G5.2 | Open. IX4 counted 34,186 Rust code lines. v15_exec owns the executor and wgpu split before the size ratchet. |
| R3 | **Variety:** 6–10 material-family uber-shaders (StandardPbr, LayeredCoat, StylizedInk, DielectricGlass, Emissive, Foliage) over a packed `MaterialTable` (64–128 B rows) plus bindless textures, at 0 ms compile per material; compiled kinds plus a pipeline cache stay for novel maths. **Cut** compiler auto-merge. **Reject** a material VM for shading: Tier 0 is for live geometry previews only. | G3 | Open. cap1c owns the material families and packed table. |
| R4 | **Two style contracts:** `fn material(hit) -> Bsdf` (works under every physical integrator) and `fn shade_style(hit, light: LightSummary) -> rgb`. LightSummary carries key_dir, key_vis, key_radiance, indirect_irradiance, ao, footprint_m, depth_m and id. An integrator that doesn't provide LightSummary refuses a Style at compile time (for example woodblock with spectral_caustic). `footprint_m` replaces fragment-only dFdx/dFdy. | G6.4 | Held on std1c/std1d. The one-argument physical material query and direct woodblock import remain open. v15_stdlib_finish owns integration. |
| R5 | **Bloat is a data-serialisation bug** (95.7% of corpus tokens are baked float arrays). (a) `import_table` for binary sampled data; (b) regenerate the corpus from its procedural seeds with `build for` and row defaults (median → ~1.5–4k tokens); (c) fix ad.rs row-constant parameters (−112k tokens); (d) token caps go in `loft lint` and corpus CI only, **not compile errors**. | G6.1, G6.2 | Open. v15_bloat owns corpus regeneration, binary tables and lint budgets. |
| R6 | **Native-first,** with a strict `HostCaps::WEB_BASELINE` tier and no parity promise between native-high and web. **A 3-rung wgpu bridge** (~600–900 LOC): (i) `EXPERIMENTAL_RAY_QUERY` in World (verified on M4 in `cap1`), (ii) `EXPERIMENTAL_COOPERATIVE_MATRIX` (8×8×8 f32/f16 verified on M4 in `cap1`) plus `PASSTHROUGH_SHADERS` MSL for `bfloat`/unchecked loops, (iii) `MetalFX` via `HostCallback` + `wgpu_hal::metal`. No `objc2-metal` backend unless we commit to SBT RT pipelines, mesh shaders or multi-queue. | G8.4, G8.5 (decided) | Rung 1 remains open. v15a used software traversal for both secondary rays on M4. No native mesh-query timing. |
| R7 | **x2b:** replace the 900-line u32 soft-float with a motion-only FMA contraction barrier; 1,000-crab dispatch ≤ 0.06 ms (predicted); shading stays in MTLMathModeFast. | G7.1 | Open. v15_perf owns the motion-only barrier and dispatch measurement. |
| R8 | **The two-tier picture gate lives in code** (crates/loft-cli/src/gates.rs), as `Deterministic(tol)` and `Stochastic(PSNR/FLIP + temporal)`. | G4 (lane v15c) | Branch only (v15c). Two modes in a Python scorer yielded 19/19 ordered deterministic matches and 4 passing seeded frames. The production Rust gate still has one picture mode. |
| R9 | **One conductor:** a central concurrency pool plus a quiet-GPU timing lock that blocks builds and other lanes during Speed and Load scoring. | G4.3 | Branch only (v15c). A build overlap returned exit 6. The single six-pair capture reached a 1.858 ms triangle p50. No three independent quiet timing verdicts. |

## Execution waves at the 20 November close

IX4 scored `3c24a11`. Pictures held 19/19, Twin 36/36, Speed 6/12, Load 23/33 and Truth 34/34. All six latency cases were contended. Wave 15-2 defaults to one step and checks load and STALL before each lane. [IX4 audit](../agent_notes/2026-11-20_v1/OPEN.md#wave-x4-integration)

| Wave | Lanes | Covers | Sequencing |
|---|---|---|---|
| **Infra (standalone)** | `build1`, `cam1` | Cambium real `cargo-deps` seed + loud cold-seed / harvest fixes | Loft seed merged at `41c1f4d`. Cambium seed fixes published on release/v0.10. |
| **wave4** (`ix4` done `2f809a4`) | `std1b` + `std1e` **merged**; `std1c`, `std1d`, `port1` held for `v15_stdlib_finish` | G6.3, G6.4/R4, G7.2 | Done (`ix4.done`) |
| **wave15** (`iv15` done `47c4fec`) | `v15a`, `v15b`, `v15c` finished & audited; 3 narrow cross-crate deltas folded into `v15_exec` | G1, G4, G7.2, G8.1, R1 #1, R8, R9 | Done (`wave15.finished`) |
| **Anti-freeze (`final.sh`)** | `cl1`, `fs1`, `cam2`, `fs2`, `wrap1` | macOS 15.5 `proc_find` anti-freeze across `cleanup_tools`, `cambium`, `freeze_sampler` | freeze_sampler and Cambium pushed. cleanup_tools awaits a remote and reconciliation of local edits. |
| **wave15_2 phase 1** (`wave15_2.sh`) | **`v15_exec`** (solo!) | **R2** + held `v15a`/`v15b`/`v15c` deltas (`EXPERIMENTAL_RAY_QUERY` bindings, StudyInk binding, Rust `gates.rs`) | Queued after `final.finished`; runs **alone** per R2 |
| **wave15_2 phase 2** (`wave15_2.sh`) | `v15_stdlib_finish` (`std1d`+`std1c`+`port1`+R4), `v15_perf` (R1 #2–3 + R7), `v15_native` (R6 rungs 2–3), `cap1c` (R3 uber-families), `v15_bloat` (R5 corpus/lint), `cap1b` (G8.2/G2 P2G) → `iv15_2` | R1 #2–3, R3, R4, R5, R6 rungs 2–3, R7, G2, G5.4, G6.4, G7.2, G8.2, G8.3 | Calm-gated (`load < 20`, no `STALL`, `LOFT_MAX_STEPS=1`) |
