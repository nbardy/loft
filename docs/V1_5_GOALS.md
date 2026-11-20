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
| G1.1 | `shade_tiles` exposes `(hit, px, world)` from the raster visibility buffer, plus optional MSAA/subpixel visibility, so RT shadows, one-bounce GI and ReSTIR run on top of the vis-buffer and never replace it with full-screen software primary rays | `engines/realtime_matched` uses it, with no 4-tap primary-ray `screen_pass` | open (lane v15a) |
| G1.2 | Matched heavy-lighting A/B, 20 groups: new ≤ old p95 | AB_OLD_VS_NEW.md §B rerun uncontended. Was 56–58 ms new against 26–29 ms old, under contention | open |
| G1.3 | Matched light-lighting A/B, re-measured uncontended: new ≤ old | §A rerun | open |

## P1: modern GPU physics (critique #8 / scientist #8)

| ID | Target | Check | Status |
|---|---|---|---|
| G2.1 | `kernel ... allow(atomics, workgroup, subgroups, dynamic_loops)` lowers for any kernel name, not only `deposit_caustic` | A kernel test suite; no E405 for legal kernels | open (lane v15b, building on s1-escapes) |
| G2.2 | A sanctioned **NonDet** physics tier outside the CPU twin, with statistical twin checks; **Det** stays for gameplay and rollback rigs | The spec's lead ruling plus a NonDet example scene | open |
| G2.3 | Fixed-point deterministic P2G scatter and a float-atomic fast tier, both measured on M4 | A P2G probe: particles/ms per tier | open (cap1 design) |
| G2.4 | One physics showcase (MPM or FLIP splash) running interactively | Frame p95 ≤ 16 ms at 1280×800 | open |

## P2: variety through data-driven materials (critique #4 / scientist #7)

| ID | Target | Check | Status |
|---|---|---|---|
| G3.1 | A data-driven uber-kind: one compiled PBR/stylised pipeline reading a packed `MaterialTable` plus a texture atlas or bindless array indexed by `material_id`, beside compiled custom kinds | A 200-material scene builds with **≤ 3 pipelines** for materials | open (after cap1) |
| G3.2 | 1,000 unique materials: cold load ≤ 1 s, frame p95 within 10% of a 10-material scene | Paired measurement | open |
| G3.3 | Live material edits (parameter rows) at 0 ms compile | `set_param`/row upsert, with no pipeline creation | open |

## P3: picture gates that allow stochastic and temporal rendering (critique #7, #9 / scientist #9)

| ID | Target | Check | Status |
|---|---|---|---|
| G4.1 | **Deterministic mode:** ≤1/255 drift allowed on <0.1% of pixels for perf/compiler changes, with no structural change | Gate implementation and a mutant that exceeds it | open (lane v15c) |
| G4.2 | **Stochastic/temporal mode:** a seeded error metric (PSNR or FLIP) against a converged reference, plus a temporal-stability check | A path-traced scene passes; a broken estimator fails | open (v15c) |
| G4.3 | Quiet-GPU timing: conductors never time while builds run; Speed and Load accept only paired confidence intervals | The same code scores the same verdicts in three runs | open (v15c) |

## P4: core size and escape hatches (critique #2, #3)

| ID | Target | Check | Status |
|---|---|---|---|
| G5.1 | Rust ≤ 18,000 lines (lean1 ceilings), enforced by `tools/check.sh lines` | Was 33,151 on 20 Nov | open |
| G5.2 | loft-std has no wgpu code; the loft-cli duplicate renderer is deleted | grep plus lines | open |
| G5.3 | Whole raw WGSL `pass` with custom bindings works from `.loft` | s1-escapes merged | open |
| G5.4 | `msl(...)` passthrough with Metal slots auto-generated from PassDecl.bindings | bk1b design → lane | open |
| G5.5 | Whole-pass parity gate within 5% of hand-written controls (Hermit Shade, Shrine Vis) | s1-controls-fix merged | open |

## P5: language against bloat (critique #5)

| ID | Target | Check | Status |
|---|---|---|---|
| G6.1 | Corpus median ≤ 10,000 tokens, with pictures matched | Was 96,204 (tok1) | deferred to a later v1.5 wave (std1f brief ready) |
| G6.2 | A per-scene token budget gate and a repeated-block lint | Fails a scene with unrolled rows | open |
| G6.3 | stdlib/ tree with styles, integrators, schedules and engines; copy-paste forking documented | std1a plus the port lanes | in progress (wave x4) |
| G6.4 | A written **style↔integrator contract**: what a style may read and write, per integrator class | A doc plus woodblock running under realtime and reference | open |

## P6: motion (critique #8)

| ID | Target | Check | Status |
|---|---|---|---|
| G7.1 | Authored solvers agree with the CPU twin (Det tier) with FMA kept on in shading, plus an automated MSL leak check | x2b merged, with a 1,000-crab dispatch ≤ 0.15 ms | open |
| G7.2 | The 5-crab lineup dance and crab_live ported | port1 merged | in progress (wave x4) |

## P7: platform and capability (scientist §2)

| ID | Target | Check | Status |
|---|---|---|---|
| G8.1 | Inline RAY_QUERY wired into `World` on native (RT shadows, reflections, one-bounce GI) | A mesh scene with RT shadows | open |
| G8.2 | Web tier packs tables into fewer storage buffers (8-buffer cap) | spectral_caustic_study runs on web | open |
| G8.3 | MetalFX temporal upscaling via `PassRun::HostCallback` plus `wgpu_hal::metal` interop, with no native backend | Upscaled frame and measured ms | open |
| G8.4 | Native Metal backend: **deferred.** Build it only if Loft commits to RT pipelines or matrix-core neural rendering | A decision note | decided: deferred |
| G8.5 | Decide native-first or web-first | A decision note | open |

## Pending branches to finish and merge (carried into v1.5)

| Branch | What remains |
|---|---|
| loft/x2a | Execution path: per-frame allocations; loft-std still uses wgpu; execution code 6,717 lines |
| loft/x2b | Twin FMA fix: 1,000-crab dispatch over budget; MSL leak guard missing |
| loft/x2c | Sky bake: shading cannot sample the bake (no binding) |
| loft/s1-escapes | Kernel allow, whole-pass `pass`, bake sampling, unproved: StudyInk integration incomplete |
| loft/s1-controls-fix | Hermit HDR mismatch in the control pass |
| loft/pf1b | Grouped long marches: missed its noise band (re-measure under quiet timing) |

## Deferred (lead, 20 Nov 14:50)
- stdaot1 (std AOT and the pipeline cache): std parsing is a few ms. The pipeline cache comes back with the cold-load work.
- std1f (corpus regeneration): after the stdlib port lands.

## Round 1 decisions (chief scientist's answers, 20 Nov 15:01: agent_notes/2026-11-20_v1/scientist_round1_answers.md)

These supersede the rows above where they differ.

| ID | Decision / new target | Replaces |
|---|---|---|
| R1 | **P0 order:** (1) `shade_tiles` exposes (hit, px, world) plus optional 4× MSAA vis-buffer, AND `EXPERIMENTAL_RAY_QUERY` is wired into `World` (matched Table B 58 → < 15 ms uncontended, predicted); (2) finish the x2c sky bake (the Shade pipeline lacks a cube binding): Hermit-20 3.75 → ~2.5 ms; (3) Shrine CoralBranch cluster culling, −0.25 to −0.45 ms. **Do not tune BSDF ALU or march steps now.** | G1, G8.1 |
| R2 | **Core ceilings:** wave 1 ≤ 18,000 scc lines (loft-std has 0 wgpu deps, loft-runner ≤ 2,200, loft-cli/native ≤ 450); wave 2 ~14,500–15,500 after schedules and BSDFs move to `.loft`. **Sequencing:** first ONE owner merges one execution path plus a wgpu-free loft-std (deleting ~4k lines); then a `tools/check.sh lines` ratchet. Never run 4 parallel lean lanes again. The compiler owns parse/check → typed HIR, AD and Lipschitz, Table+Kernel+Pass lowering, and bundles. The library owns all rendering, lighting, schedule and motion recipes. | G5.1, G5.2 |
| R3 | **Variety:** 6–10 material-family uber-shaders (StandardPbr, LayeredCoat, StylizedInk, DielectricGlass, Emissive, Foliage) over a packed `MaterialTable` (64–128 B rows) plus bindless textures, at 0 ms compile per material; compiled kinds plus a pipeline cache stay for novel maths. **Cut** compiler auto-merge. **Reject** a material VM for shading: Tier 0 is for live geometry previews only. | G3 |
| R4 | **Two style contracts:** `fn material(hit) -> Bsdf` (works under every physical integrator) and `fn shade_style(hit, light: LightSummary) -> rgb`. LightSummary carries key_dir, key_vis, key_radiance, indirect_irradiance, ao, footprint_m, depth_m and id. An integrator that doesn't provide LightSummary refuses a Style at compile time (for example woodblock with spectral_caustic). `footprint_m` replaces fragment-only dFdx/dFdy. | G6.4 |
| R5 | **Bloat is a data-serialisation bug** (95.7% of corpus tokens are baked float arrays). (a) `import_table` for binary sampled data; (b) regenerate the corpus from its procedural seeds with `build for` and row defaults (median → ~1.5–4k tokens); (c) fix ad.rs row-constant parameters (−112k tokens); (d) token caps go in `loft lint` and corpus CI only, **not compile errors**. | G6.1, G6.2 |
| R6 | **Native-first,** with a strict `HostCaps::WEB_BASELINE` tier and no parity promise between native-high and web. **A 3-rung wgpu bridge** (~600–900 LOC): (i) RAY_QUERY in World, (ii) PASSTHROUGH_SHADERS MSL for simdgroup_matrix/bfloat, (iii) MetalFX via HostCallback + wgpu_hal. No objc2-metal backend unless we commit to SBT RT pipelines, mesh shaders or multi-queue. | G8.4, G8.5 (decided) |
| R7 | **x2b:** replace the 900-line u32 soft-float with a motion-only FMA contraction barrier; 1,000-crab dispatch ≤ 0.06 ms (predicted); shading stays in MTLMathModeFast. | G7.1 |
| R8 | **The two-tier picture gate lives in code** (crates/loft-cli/src/gates.rs), as `Deterministic(tol)` and `Stochastic(PSNR/FLIP + temporal)`. | G4 (lane v15c) |
| R9 | **One conductor:** a central concurrency pool plus a quiet-GPU timing lock that blocks builds and other lanes during Speed and Load scoring. | G4.3 |
