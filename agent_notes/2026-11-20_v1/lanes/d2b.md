# D2b · Tier 0 edit cost

Base `26117b1`. Reuse the authored compiler and shaders at `f5bea03` by path. The frozen wire tape remains the source for interpreter controls. The render session publishes an edit-local execution plan alongside it.

## Prediction before implementation

Predict 0.80 ms from source bytes through final queue write, +0.30 ms paired whole-frame p95 including Present, and zero foreground pipeline creates. Budget 0.35 ms for parse and checking, 0.30 ms for plan, proof and specialization, and 0.15 ms for publication. The frame estimate is +0.05 ms staging, +0.19 ms visibility and +0.06 ms shade. Measure the hypotheses independently. Gates are 2.000 ms, +0.500 ms, zero foreground creates or changed clean pipeline pairs, field error ≤1e−5, and 99.9% within one code point. Exact fixture frames should have zero differing pixels.

The old 10-word clip does at most 2,010 wire dispatches per hit. A fused clip does zero dispatches and three primitive/combiner operations on each of the same 201 possible visits. A 63-word residual can do up to 12,663 word visits per covered pixel before staging. At 160 × 100 8×8 tiles that is up to 12.97 billion word visits. The high-pressure case can exceed the frame cap despite a fast clip. Nonconstant loop work scales with actual trip counts. Report these separately from compile time and timestamp controls.

Budget two decoded u32 words per plan instruction (16 KiB for the maximum tape), at most 16 KiB workgroup memory for a shared decode, and up to 512 B per invocation for the residual register bank plus 128 B loop counters. Aim for at most eight live vec4s in fused fields. These are implementation storage bounds. A hardware spill claim needs compiler or counter evidence.

Three independent locked runs at 1280×800. Predeclare A/A signed paired p95 ≤0.100 ms and triangle median ≤0.350 ms. Warm up each session, keep one on Tier 0, promote one after 400 ms quiet, and queue both frames before reading either clock. Keep per-pass deltas and raw timestamp endpoints. The common-recorder one-submission repetition belongs to D2c integration.

## Archived measurements on M4 Metal

| Edit or control | Source → upload, ms | Paired frame p95, ms |
| --- | ---: | ---: |
| V1 literal, confirmation | 0.505 | +2.327 |
| V1 structural | 0.466 | +0.120 |
| V1 bounded loop | 0.466 | −0.026 |
| V1 clip, confirmation | 0.448 | +2.478 |
| V1 reverse | 0.705 | +0.540 |
| V1 union | 3.027 | +0.230 |
| V1 two-sided residual | 2.280 | +3.622 |
| D2 slab residual | 2.631 | +3.273 |
| D2 slab fused, both runs | 0.472 / 0.485 | +0.246 / +0.170 |
| D2 slab A/A | — | +0.159 |
| D2 general A/A, both runs | — | +0.086 / +0.980 |

The archived A/B timing used separately submitted frames with a read between submissions. The D2b score queues both first. Report paired whole-frame quantiles and each pass's paired delta in schedule order. All bounds and frames are compared with a fresh Tier 1 compile, with promotion checked separately.

## Implementation and work controls

The private plan versions virtual definitions across a bounded loop expansion, folds finite portable constant arithmetic, drops dead definitions and linear-scans the staged schedule. Straight-line rendering reads predecoded instructions. Shade workgroups share up to 128 instruction pairs after a uniform barrier, and stage row and frame operands from each invocation's current row and frame. Larger or unexpanded legal tapes retain the verified 32-register path. Render-only source admission and the existing native session interface are unchanged. A general invariant combining row with frame data is prepared once per invocation before the march or shade samples. Cross-invocation caching of those results remains open. The fixed compute VM checks LUT ranges and texture layers against the bound resources. Authored traces still require an applicable asset binding before lowering those operations.

The prewarmed evaluators are ordered binary trees with up to four leaves. They fuse certified sphere and box expressions, signed coordinate planes, absolute-coordinate clips and either CSG join at every interior node. The remaining arithmetic stays in the staged residual. A fast bound needs continuous enclosure and a witnessed interior point in each snapped extremal cell. Both tiers also use the same fresh interval derivation when the fast witness is unavailable. AO still uses the normal obtained from the four independent normal taps.

The reserved row-buffer allocation is 57,448 bytes per dirty kind. The uploaded prefix varies with tape, constants and private plan. Shade reserves 1,024 bytes of workgroup instruction storage and the stand-alone compute VM reserves 16,384 bytes. The residual shader declares one 512-byte staged bank, a 512-byte sample bank and 128 bytes of loop counters, before backend allocation and elimination. A four-leaf fused tree passes eight vec4-equivalent operands plus scalars. These are source-level bounds. Metal hardware register allocation and spill bytes are unobserved.

The archived executable's 63-op baseline had 16 logical live vec4s and measured **10.219792 ms source to upload** and **+78.355583 ms paired p95**. Per-pass paired p95 was `[0.000501, 38.133126, 0.432417, 40.240792, 0.579916]` ms. The independent fresh SdfSteps frame covered **1,024,000 hits across 16,000 tiles**. It counted **8,301,676 march visits** with a nine-step maximum and **9,216,000 shade visits**. That is 1,103,613,588 wire instruction visits at 63 instructions per query. The independent baseline A/A p95 was **+0.108583 ms** and triangle median **0.095625 ms**. Its first six signed pairs were `[-0.001918, -0.010041, -0.025000, -0.030167, -0.058625, -0.331458]` ms.

The archived VM on the fresh nested fixture scored **10.841166 ms source to upload** and **+20.997708 ms paired p95**. Per-pass paired p95 was `[0.000209, 16.891916, 0.267334, 4.380749, 0.296833]` ms. The old compiler reported 41 wire instructions and eight logical live registers. Both baseline images exactly matched the promoted image. Baseline field error was zero on 2,197 samples.

| Archived independent 1280×800 control | 120-pair A/A p95, ms | Triangle median, ms | First six signed A/A samples, ms |
| --- | ---: | ---: | --- |
| 1 | +0.108583 | 0.095625 | −0.001918, −0.010041, −0.025000, −0.030167, −0.058625, −0.331458 |
| 2 | +0.146750 | 0.095542 | −0.001666, −0.306375, −0.074584, −0.050792, +0.008125, −0.020125 |
| 3 | +0.169875 | 0.027958 | +0.009751, −0.003708, +0.007333, −0.048667, −0.063541, +0.440292 |

These controls used the archived executable during the compile-lock wait. All raw A/A endpoints are in `baseline-control*.json`. They bound contemporaneous measurement noise but carry no D2b timings.

## D2b build and score

The single prescribed build exited **101** at `crates/loft-passes/src/tier0/session.rs:466` with E0282 on the `xyz` closure's array pattern. It waited 860.3 s on the shared Keel lock. The branch includes an explicit `[f64; 4]` annotation added after the failed build. The single-build rule leaves that correction uncompiled. Cargo produced no D2b focused executable. All three D2b runs, promotion and reference comparisons, GPU spill evidence and after measurements remain unscored.

The first planned residual is the 63-op full-screen fixture. Its staged schedule reserves the first 16 instructions for invariant loads, then opens residual PCs **16..63** with `LOAD_INPUT(position)`. It predicts 47 instructions per sample and 18 live logical vec4s at peak. On the archived diagnostic's 16,000 tiles and 17,517,676 visits, this predicts **823,330,772 residual instruction visits** plus setup invocations. The A/A and triangle observations above are controls from the archived build. D2b correctness, edit latency, paired whole-frame p95 and generation checks require an integration build and the locked runs.

[Build log](../../../out/meta/d2b/build.log) · [Baseline full-frame log](../../../out/meta/d2b/baseline-full.log) · [Nested baseline](../../../out/meta/d2b/baseline-nested.log) · [Raw control 1](../../../out/meta/d2b/baseline-control.json) · [Raw control 2](../../../out/meta/d2b/baseline-control-2.json) · [Raw control 3](../../../out/meta/d2b/baseline-control-3.json)
