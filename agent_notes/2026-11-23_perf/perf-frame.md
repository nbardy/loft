# perf-frame: Shade and Classify frame cost (23 Nov 2026)

Branch `perf-frame`, base `34bd927`. Lane owner of Shade/Classify shaders, `stdlib/integrators`, `stdlib/lighting`, `stdlib/engines`.

## Measurement conditions

The M4 was never quiet. Other lanes held the GPU lock back to back, load average 5-22, and the same variant's Shade median moved 2.5x
between repetitions of one run (4.8 to 10 ms for Hermit-20). Absolute milliseconds below are therefore not comparable with PF1's
(Hermit-20 Shade 2.19 ms, Vis 1.42 ms). The Vis pass is untouched by this lane, so every candidate is scored by the ratio
`Shade / Vis` of the same process, interleaved and shuffled across variants with a warm-up run before each timed run
(`scratchpad/ab.py`, `ratio.py`). Ratio on Hermit-20 baseline: 1.52-1.68 against PF1's 2.188/1.417 = 1.54, so the ratio transfers.
Predicted milliseconds are the ratio change times PF1's quiet Vis.

## Where Shade goes (omission probes, Hermit-20 ratio, baseline 1.569; Crab Live idle baseline 7.04)

| probe | change | Hermit-20 Shade/Vis | share of Shade |
| --- | --- | ---: | ---: |
| P1 | sky loop off (sun + emission only) | 0.639 | sky quadrature is 59% (Crab idle: 58%) |
| P4 | `sky_dome(l)` replaced by a constant | 1.361 | sky weights are 13% |
| P2 | `ggx_energy(rough, nl)` replaced by 0.5 | 1.365 | per-sample energy LUT is 13% |
| P3 | film `airy` at the half vector replaced by `fv` | 1.466 | per-sample Airy is 6.6% |
| P5 | pixel-invariant GGX terms hoisted into lobe lets | 1.462 | the compiler was NOT hoisting them: -6.8% median, -9.5% min |

So the cost is ALU and loads inside 14 BSDF evaluations (about 7 live per pixel; the 6 patches below the horizon return early),
not transcendentals alone, not shadow or classification.

(The P2/P3/P4/P5 percentages above came from the first, noisy instrument and are order-of-magnitude only. The paired instrument
below measured the lighting changes more tightly; P1 is robust under both.)

## Instrument

`ab2.py` (scratchpad): per round, every variant runs once in shuffled order (80-100 frames after 15 warm-up frames), and each
variant's median pass time is divided by the mean of two identical baseline runs from the same round. Null check (baseline vs
baseline): Hermit-20 Shade IQR 0.997..1.003, Crab Live idle 0.96..1.04, matched B 0.994..1.017. A change needs to clear
roughly 2x that band to count.

## Heavy-lighting gap against legacy, apples to apples

`tools/ab_legacy.sh --matched` (prebuilt legacy `loft-draw` from the 21 Nov rescue, tag `legacy/2026-11-19` at `acb7e8b`),
Apple M4, 1280x800, t = 0, 300 timed frames, triangle control p95 0.044 / 0.103 ms (limit 0.15). A = both engines reduced to
the native workload, B = native raised to the legacy workload (`realtime_matched`). Legacy numbers are its "gpu frame" median.
Legacy itself swings 2-4x between windows (B-20: 5.3 to 21.9 ms inside one 300-frame run), native B-20 does not.

| pair | legacy median | native p50 / p95 on base `34bd927` (window 1) | native p50 / p95 with this lane (window 2, legacy re-run beside it) |
| --- | ---: | ---: | ---: |
| A, 1 group | 0.320 / 0.319 | 0.346 / 0.426 | 0.345 / 0.440 |
| A, 20 groups | 0.707 / 0.658 | 0.802 / 2.519 | 0.641 / 0.675 |
| B, 1 group | 0.832 / 0.560 | 1.385 / 4.421 | 0.910 / 1.002 |
| B, 20 groups | 10.615 / 5.643 | 9.251 / 10.014 | 5.287 / 6.464 |

The 20 November "2x slower on heavy lighting" (58 against 27 ms at 20 groups) is gone on this base: native B-20 is already at
parity (0.87x of legacy's median in window 1, 0.94x in window 2). What is left is a fixed cost at small counts: B-1 is
1.6-1.7x legacy in both windows. Omission probes (paired, B 1 / 20 groups, Shade ratio to baseline): `samples: 4` off (center-only
shading) 0.52 / 0.31 (the 4-tap edge refinement is 48% / 69% of Shade); no bounce trace 0.63 / 0.69; no sun visibility ray
0.77 / 0.86. Frame ratio of the same probes: 0.68 / 0.47, 0.90 / 0.74, 0.94 / 0.89. So B is edge-refinement-bound, not BSDF-bound,
and the edge refinement is what buys legacy's antialiasing, so it stays.

## Changes (all pixel-exact) and what they were predicted to do

Pixels: 18 stills (Hermit-1/20, Capsules-20, Blobs-20, matched B-1/20 at t = 0, 4, 9; Shrine; Crab Live) are byte-identical to base,
except the two scenes that are already nondeterministic on base (Shrine 46-87 pixels, Crab Live 3 pixels differ between three
runs of the unmodified binary). `loft gate --only` over the seven gate scenes: Pictures 19/19, Twin 36/36, Truth 34/34. Speed and
Load fail as on base under load (14 pipelines against 13 on Hermit is also on base). `tools/dev.sh test --workspace`: 187 passed.

1. **Present reads color only where something was shaded** (`stdlib/core/present.wgsl` `{{LOAD}}`, `emit_present`). A sky pixel
   now costs one visibility read instead of that plus a discarded 8-byte color read. Predicted Present -25% on sky-heavy scenes.
   Measured (paired, same window, Present ms base -> new): Capsules-1 0.230 -> 0.201, Capsules-20 0.179 -> 0.144, Blobs-1 0.132 ->
   0.107; frame -6.3% / -5.4% / -5.7%. Hermit-1 and Crab Live (no sky) unchanged. Predicted-vs-measured: -12..-20%, a miss on size.
2. **Subpixel-edge path (`samples: 4`): present tests HDR alpha instead of the 3x3 visibility neighborhood.** Halo classify
   zeroes the HDR alpha of every pixel (it already visits them all); shade stores 1.0 at exactly the pixels it owns (the
   neighborhood rule is the same, so the set is identical); present reads `alpha == 0` for sky. `bundle.rs` binds `hdr` to
   classify only when the module declares it. Predicted B-1 frame -14%, B-20 -2.7%. Measured: Present 1.063 -> 0.291 ms (B-1)
   and 0.546 -> 0.307 ms (B-20) in one window; classify +0.01 ms. A hit and larger than predicted.
3. **BVH traversal keeps the nearer child in hand, pushes only the farther one and drops the entry-distance stack**
   (`crates/loft-emit/src/bvh.wgsl`; bit-identical visit order, a popped node is retested against the current `t1`, which
   equals the old `entry > t1` skip). Predicted B-20 Shade -3%. Measured Shade 8.907 -> 7.275 ms (-18%, paired IQR 0.79..0.83)
   and B-1 -11%; no effect on A scenes (they use no BVH). A hit, six times the prediction (the stack arrays live in thread
   memory, so halving them and the reload matters more than the arithmetic).
   Together B: frame B-1 4.57 -> 3.58 ms (-21%), B-20 10.01 -> 8.18 ms (-18%) in the contended window; 1.744 -> 1.466 (-16%)
   and 13.16 -> 11.45 (-10%) in the next one.

Tried and reverted, no robust gain on the quiet-enough windows:

- Hoisting the pixel-invariant GGX terms (`sqrt(a2 + (1-a2) nv^2)`, the multiple-scattering weight, Airy constants) into lobe lets:
  Hermit-20 Shade -3.3% (IQR 0.92..0.98) in one window, 0 in another. The compiler already hoists most of it.
- `sun_sky` skipping the sun when `shadow == 0` and the sky weight when the lobe sum is exactly zero: -4.9% in two windows, -0.4% and
  -2.8% (Vis moved -1.9% the same way) in the others. Exact, but not demonstrably faster, so not landed.
- Tabulating the sky-dome weight per patch (`sky_patch_dome`): no gain over the previous item.
- Shadow map: replacing the nine bilinear compares by four gathers is capped by the shadow cost itself. Disabling the shadow lookup
  entirely moved Hermit-20 Shade only -2.1% and Crab Live -1..-4%, so it was not written.

## Where Shade time goes (answer to the profiling question)

Sky quadrature is 58-59% of Shade on both Hermit-20 and Crab Live (sun-only Shade/Vis 0.639 against 1.569; Crab 2.85 against
9.45). Inside it: the per-sample energy-table lookup (`ggx_energy(rough, nl)`, 4 storage loads and a sqrt per lobe per sample)
is 9-13% of Shade; the film lobe's per-sample Airy term (3 cos, sqrt, divides) 6-8%; the nine shadow compares 2%; the sky weight
`sky_dome(l)` 9-13% by omission but not recoverable by table; per-pixel invariants are already hoisted by the Metal compiler
(a hand hoist moved nothing reliable). About half of the twelve patches are behind the surface and return early, so roughly
seven BSDF evaluations per pixel stay live. Classify is 0.04-0.15 ms and Present 0.10-0.35 ms of frames that are 0.35-1.0 ms
in the light scenes, i.e. 30-55% of Capsules-1's frame is Classify + Present, not Shade. No counter for occupancy, registers or
DRAM bytes exists here (no Metal toolchain, timestamps only), so those stay unmeasured.

## What this lane did not get

- Shade on the A path did not get a verified exact win; the only large lever left is a look change (fewer or cheaper sky patches
  for rough lobes, or split-sum image-based sky), which was not built because it would break the 0.1% picture rule and no owner
  approval exists. Hermit-20 Shade stays near 2.2 ms quiet.
- B-1 is still 1.6x legacy; the 4-tap edge refinement is the cost and is also the look.
- Measurement: every window here had other lanes on the GPU (Hermit-20 Shade 3.1 to 11.4 ms for the same binary). Only paired,
  shuffled, same-round ratios with a baseline-against-baseline null (IQR 0.997..1.003 on Hermit-20 Shade) were used for verdicts.
  Absolute milliseconds in this note are for orientation only.

## Files touched outside the stated ownership (lowering for Classify/Present/Shade)

`crates/loft-emit/src/bvh.wgsl`, `crates/loft-emit/src/emit.rs`, `crates/loft-emit/src/emit/shade_tiles.rs`,
`crates/loft-passes/src/bundle.rs`; inside: `stdlib/core/classify.wgsl`, `stdlib/core/classify_halo.wgsl`, `stdlib/core/present.wgsl`.
