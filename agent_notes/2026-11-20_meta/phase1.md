# Meta engine Phase 1: the hermit crab, render only

Plan: `docs/proposals/2026-11-20_meta_engine_plan.md` §5 Phase 1. Language: `agent_notes/dump_manual_nov_19th_alpha_v1.md`
§2, §3, §5, §6, §7. Branch `meta/phase1` from main `eb21166` (Phase 0 merged, Astra spec v1 present but not final).

## What Phase 1 builds (as I understand the request)

The draft's crab without motion, drawn by the Phase 0 spine, every gate live, at 1 and 20 crabs:

- **Shell**: `scan` bakes parallel-transport frames along the spiral at load; `sample_c2` reads them with a uniform cubic
  B-spline (C2 in t). The shell is a parametric surface `query surface(self, u, v) -> Pt[Shell]`, rasterised from a
  grid in the vertex stage; its normal is derived by forward AD (`diff`), never authored or finite-differenced.
- **Claw**: an SDF (`query trace`) whose Lipschitz bound is derived (round box, cut box, capsules, a rotated jaw map,
  sine teeth under a smootherstep).
- **Legs**: one kind, 12 rows per crab (lengths in rows); closed-form `visible` (three round cones) agreeing with `trace`.
- **Face**: an SDF with a mirrored map space; the gaze in `material` lifts each eye centre out with `.to_parent`
  (distinct left/right world positions) and reads `.cell`.
- **Bsdf algebra**: `material` returns a `Bsdf`, an ordered sum of library lobes (`ggx`, `clearcoat`, `thin_film`,
  `sss`, `emission`). A lobe is an ordinary library fn marked `@lobe` whose last three parameters are `n, v, l`; it
  returns `vec4(radiance factor, transmittance to the layers below)`. Each kind module gets its own `Bsdf` struct with
  only the lobes it uses, so a new lobe changes no other kind and no vis-buffer layout.
- **Vis64, tile classification, tile shading**: as Phase 0; the second Vis64 word is now kind-owned (ray t for traced
  and analytic kinds, packed (u, v) for surface kinds).
- **Real shadows across kinds**: a sun shadow map drawn by replaying every kind's own vis pipeline from an orthographic
  sun camera (a second RenderBundle, all rows). No extra pipelines, no stand-in geometry.
- **Two Phase 0 misses**: bound derivation per distinct local shape instead of per row (below); a disk pipeline cache
  where the backend allows it.

Decisions (mine, to revisit against the final specs; syntax lives in the parser):
- A query asked in a space S other than `World` is placed by the row's one `Xform[S -> World]` field; the compiler
  applies the pose. So the field is authored in local space, and the derived bound and L are a property of the local
  shape: rows that differ only in pose share one derivation. Derivations = distinct shapes, not instances.
- Lighting is one library fn named by the schedule: sun (shadow-mapped) + sky by a fixed 12-direction quadrature of the
  Bsdf + emission (the Bsdf evaluated at `l = 0` returns emitted radiance). This is a quadrature, not IBL; stated.
- Kinds: Ground, HermitShell, CrabFace, CrusherClaw, CrabLeg (K = 5). No antennae, no body motion (Phase 3).

Inherited vs ours: uniform cubic B-splines, double-reflection rotation-minimising frames (Wang et al. 2008), forward-mode
AD, GGX/Smith/Schlick, Airy thin-film reflectance, wrap-lighting SSS, ortho shadow maps with PCF, iq's closed-form
round-cone hit are textbook. Ours: pose-factored derivation keyed by shape, AD as a source-to-source pass shared by the
CPU twin and the GPU emit, lobes as plain library fns with a layering contract, shadows by replaying the vis bundle.

## Predictions (written before building, scored once at the end)

Frame passes for K kinds: cull 1 dispatch, vis K draws (bundle), shadow K draws (bundle), classify 1, shade K
indirect dispatches, present 1 draw. Pipelines: per kind one vis render + one shade compute, plus cull, classify,
present. Modules: one per kind plus three shared.

| | Draft §7 | Corrected, this build (K = 5) |
|---|---|---|
| Modules | 11 | **8** (5 kinds + cull, classify, present) |
| Pipelines | 12 | **13** (2 × 5 + 3) |
| GPU commands / frame | 13 | **18** (1 + 5 + 5 + 1 + 5 + 1) |
| 1 crab p50 / p95 (ms) | 2.1 / 2.4 | **1.8 / 2.5** |
| 20 crabs p50 / p95 (ms) | 4.0 / 4.6 | **2.1 / 2.8** |
| Cold / warm load (s), both counts | ~1.1 / ~0.15 | **0.75 / 0.11** |
| Cross-process warm (new process, same code) | — | **0.15 s** on Metal (OS shader cache) |

Draft differences: it counts motion (2 dispatches) and antennae (1) that Phase 1 does not have, and one tile-shade
dispatch per lighting stage; I add a Ground kind (shadows need a receiver) and the shadow bundle (+K draws, no
pipelines).

Correcting the draft's cold load with Phase 0's measurement: ~70 ms per pipeline when serial, and concurrent creation of
5 pipelines took 180–270 ms (≈1.5–2× parallelism in the Metal compiler service). The draft's 12 pipelines would then be
12 × 70 / 1.9 ≈ 0.45 s + front end ≈ 0.55 s, not 1.1 s. My modules are larger (lobes, light loop, PCF, B-spline), so
per pipeline ~120 ms (shade), ~80 ms (vis), ~60 ms (shared): 5 × 120 + 5 × 80 + 3 × 60 ≈ 1.18 s serial, / 1.9 ≈ 0.62 s,
+ front end (parse, check, AD, scan bake, 11 derivations, emit, naga) ~0.08 s + resources and first frame ~0.03 s
= **0.75 s**. Warm in process: pipelines ~0.4 ms each + front end ≈ **0.11 s**.

Per-pass model (ms, p50), 1 crab / 20 crabs, at 1280×800:

| Pass | 1 crab | 20 crabs | Work |
|---|---|---|---|
| cull | 0.02 | 0.02 | 17 / 321 rows |
| vis | 0.45 | 0.60 | shell grid 192×32 (37k / 737k vertex invocations); claw+face proxies ~150k / ~100k px traced ~40 steps |
| shadow (1024², ortho fit to casters) | 0.40 | 0.45 | same work from the sun |
| classify | 0.06 | 0.06 | 16k tiles |
| shade | 0.65 | 0.70 | ~1M px (ground ~70%, 1 lobe; crab 3–4 lobes) × 13 Bsdf evaluations, 9-tap PCF |
| present | 0.08 | 0.08 | full screen |
| pass boundaries | 0.15 | 0.15 | 6 passes |
| **sum** | **1.81** | **2.06** | |

p95 = p50 + the contention tail Phase 0 measured on this shared machine (≈ +0.7 ms).

Derived facts:
- Derivations (bound + L): **11 at 1 crab and 11 at 20 crabs** (ground 1, shell 1, face 1, claws 2, legs 6 distinct
  lengths); Phase 0 would have done 17 and 321.
- Derived L: face **1.00**, legs **1.00**, ground **1.00**; claw true bound 1 + 0.025·14 + 0.025·(15/8)/0.8 = **1.41**,
  interval-derived prediction **1.50** (between 1.41 and 1.60; smootherstep's interval derivative is loose).
- Module text byte-identical between 1 and 20 crabs.
- Twin: worst errors ≤ 1e-5 (trace, visible, material as Bsdf at a fixed light, surface point and AD normal).
- Truth: AD normal vs independent central differences of the CPU surface: worst angle **≈ 3e-3 rad** (f32 noise);
  sampled shell points inside every derived AABB; mirrored eyes lift to world centres 0.84 m apart.

Line budgets after Phase 1: runner 643 → ~820 (≤ 1,000); IR + gates 991 → ~1,250 (≤ 1,500); passes 3,980 → ~6,000;
lib 111 → ~450; engines 104 → ~300.

Pipeline cache: wgpu 30's Metal backend stubs `create_pipeline_cache` (no `PIPELINE_CACHE` feature; it detects but never
uses `MTLBinaryArchive`). So the disk cache through `wgpu::PipelineCache` works on Vulkan only; on Metal the only
persistent cache is the OS's own Metal shader cache, keyed by the generated MSL. Prediction: a second process with the
same code loads in ~0.15 s; with a fresh nonce it is cold again.
