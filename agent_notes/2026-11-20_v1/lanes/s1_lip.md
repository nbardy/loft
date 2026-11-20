# S1 · Preserve Euclidean bounds through rotations

Base `bca209d15a760bab2623957c13ef257e06fe46cf`.

## Prediction recorded before implementation

An interval vector with bound L supplies a common coordinate system to its projections.
For coordinates folded independently with abs, scaled by 1.45 and offset, the fold Jacobian
is a diagonal matrix followed by the signed Y-rotation. Each fold multiplies L by at most
`1.45 * max(1, hypot(c, s))`. The quoted coefficient pair is evaluated as stored f32
literals. Four folds and the stored divisor give approximately `1.000000188`.
The old bound is 22.871492. A Gram-matrix row-sum bound retains the cancellation of
the signed rotation and also bounds shear, repeated axes and non-unit matrices.
Nonlinear coupled expressions retain the scalar interval rules.

Predict Shrine's t=0 visible-hit mean at or below 25 steps and at least 0.65 ms saved
at the Vis median. A proportional estimate from PF1's 129.7 steps and 1.066 ms
lantern cost gives 0.86 ms before the cost of additional hits and misses. Predict changed
Shrine pictures at all three times because PF1 already saw 11,705 / 37,019 / 20,858 changed
pixels at half L. The larger step distance also changes the final accepted sample.
Predict 18/18 existing gate hashes match, since the manifest covers Capsules, Blobs and
Hermit. Predict matching Hermit-20 shader and frame hashes and no systematic timing delta.

Measure 1280 × 800 on Apple Metal. Shrine uses 100 fixed t=0 frames per ABBA arm and
Hermit-20 uses 300 orbit frames per arm, with 30 warmups, ring timestamps and per-arm
triangle controls. Compare lossless RGBA at 0, 4 and 9 s and count successful march
iterations from the SDF steps AOV. Run all six gate scenes on base and candidate.

## Bound

Each tracked output component is a signed row of a common matrix M plus a spatially
constant offset. Each column can have one function hⱼ of its source coordinate vⱼ
with scalar Lipschitz constant at most one. Aliases retain the identity of that
function. For example, `abs(k * h(x) + b)` is `abs(k) * abs(h(x) + b/k)`.
Distinguishing these functions prevents treating two differently shifted folds
of the same axis as a shared input to a rotation.

For any two inputs in the analysis domain, each coordinate satisfies
`|Δhⱼ| ≤ |Δvⱼ|`. Consequently
`||ΔF||₂ = ||M Δh||₂ ≤ ||M||₂ ||Δh||₂ ≤ ||M||₂ L_v ||Δp||₂`.
This applies across the abs seams and gives the corresponding Jacobian bound
where derivatives exist. A diagonal scale, including permuted axes, has norm
`max |sⱼ|`. For the Givens block the signed Gram matrix has zero off-diagonal
and equal diagonal entries `c²+s²`, so the containing 3D map has norm
`max(1, hypot(c,s))`. The implemented Gram bound uses the largest absolute
row sum of MᵀM, an upper bound on its largest eigenvalue, with outward
rounding on the dot products. It also handles a general signed shear.
The rule requires exact tracked coefficients and a shared basis and fold in
each active column. Other scalar expressions use the established component
bound. Repeated swizzle axes scale the vector bound by the square root of
the largest multiplicity. New vector certificates round up when packed as
f32.

Loft's existing `rot_x`, `rot_y` and `rot_z` map stages preserve L for an angle
independent of the traced point. Their outputs now carry a fresh coordinate
basis through scalar projection, so rebuilding or permuting those coordinates
preserves the joint bound. Authored signed rotations use the same matrix
proof as the lantern. The tracked scene needs no source edit.

## Measured result

Apple M4 (Metal), 1280 × 800, 30 warmups per arm with ring timestamps.
The stock CLI ran A, B, B, A in separate processes while holding the GPU lock.
Shrine used 100 frames per arm at t=0, `shrine_distance=6.4` and `shrine_solid=0`.
Hermit used `count=20` and 300 orbit frames per arm. Triangle medians ranged
from 0.028 to 0.095 ms against the 0.350 ms threshold. Every Shrine frame
issued 12 GPU commands and every Hermit frame issued 17. Shader hashes matched
for both scenes. Gains are main minus candidate and average the two
order-reversed differences of arm quantiles.

| Scene and metric (ms) | A1 p50 / p95 | B1 p50 / p95 | B2 p50 / p95 | A2 p50 / p95 | Gain p50 / p95 |
| --- | ---: | ---: | ---: | ---: | ---: |
| Shrine Vis | 2.104 / 2.479 | 1.233 / 1.531 | 1.161 / 2.414 | 2.161 / 2.649 | +0.935 / +0.592 |
| Shrine frame | 3.020 / 3.921 | 2.173 / 3.175 | 2.083 / 5.788 | 3.236 / 4.240 | +1.000 / −0.401 |
| Hermit-20 Vis | 1.476 / 1.801 | 1.461 / 1.758 | 1.584 / 1.997 | 1.583 / 1.873 | +0.007 / −0.041 |
| Hermit-20 frame | 4.781 / 5.446 | 4.729 / 5.532 | 5.081 / 5.731 | 5.161 / 5.825 | +0.066 / +0.004 |

The Vis median exceeded the 0.650 ms target in both Shrine pairs. Shrine Vis
p95 gains were 0.949 and 0.235 ms. Shrine frame p95 reversed sign by order.
The first run, before the f32 round-up change and using the same packed Shrine
bound, saved 0.969 / 1.187 ms of Vis and 0.982 / 1.151 ms overall. The Hermit
frame p95 pairs changed by +0.086 and −0.094 ms (candidate minus main).
The matched shaders, three bit-identical Hermit-20 stills and order reversal
support frame-time parity at the resolution of this run.

The lantern's packed L is **22.871492386 → 1.000000238**. Main and candidate
pack the same FoldedLantern AABB. The step AOV counts winning camera hits.

| Time | Main → candidate hits | Mean steps | p95 steps | New hits recovered by raising only the main cap | Changed beauty pixels |
| ---: | ---: | ---: | ---: | ---: | ---: |
| 0 s | 13,131 → 23,303 | 129.7 → 7.5 | 182 → 15 | 10,172 / 10,172 | 12,408 |
| 4 s | 49,381 → 79,631 | 135.3 → 7.2 | 184 → 14 | 30,254 / 30,254 | 38,252 |
| 9 s | 35,567 → 53,197 | 141.5 → 7.5 | 184 → 15 | 17,634 / 17,634 | 23,049 |

The scratch control changed only the old lantern cap from 192 to 4,096 in its
compiled WGSL. It recovered every new candidate hit. Its unmodified arm
differed from the stock main render at 81 / 136 / 93 beauty pixels and
12 / 33 / 16 ID pixels. The cap comparison uses the two arms from the same
probe, then checks their recovered-hit mask against the stock candidate.
No winning traced pixels went red in the secant/L AOV at any of the three
times for either revision.

**Merging requires owner picture approval for Coral Shrine.** The lane derives
the tighter L for the tracked scene as written. The scene source, gate manifest
and all 18 frozen reference files passed the before/after content hash check.
All **18/18** actual gate digests match fetched main. The gate manifest covers
Capsules, Blobs and Hermit. Against the existing approved references both
runs scored 0/18 Pictures, 36/36 Twin and 34/34 Truth. The final Speed and Load
scores were 11/12 and 28/33 for main and 10/12 and 25/33 for the candidate.
The first pair had the opposite Speed and Load ordering in the raw gate logs.

Review evidence includes lossless before/after beauty, depth, ID, steps and
secant/L PNGs at 0, 4 and 9 s. The local archive also contains the cap-only
control, per-frame profile data, actual gate digests and source digests.
[Review sheet](../../../out/meta/s1_lip/shrine-review.webp) ·
[Measurements](../../../out/meta/s1_lip/score.json)

## Verification

All 156 native tests passed across 26 executables, including all six gate
and compiler mutants and the separate `lipschitz_bound_mutant.loft` numeric
violation test. The all-targets check for Loft Passes and Loft CLI passed.
Static checks passed on 295 corpus identities, both grammar recognizers
(281/281 cases) and the host GPU pins.

Each of the seven new cases used 10,000 seeded CPU twin secants inside the
derived AABB. The four-fold trace's maximum sampled secant was 0.997958367
against L = 1.000000238. The repeated shear rows gave 2.134749022 against
2.236068010. The coupled sine gave 0.999143229 against 1.752349973, and
the distinct folded axes gave 1.357791873 against 2.218107462.
The tests also cover repeated swizzles, permuted anisotropic scales through
all three rotation stages and a rotation whose coefficients give a gain
strictly above one. Hermit-20's full serialized program had the same
SHA-256 on both revisions, including its row data and shader IR.

Reproduction uses `run-gpu.sh` under `tools/gpu_lock.sh` and `score.py` on
the saved profiles and readbacks. Build the base CLI and the cap probe
against the base lab artifacts before building the candidate.
[Secant log](../../../out/meta/s1_lip/secants.log) ·
[Native suite](../../../out/meta/s1_lip/workspace-tests.log)
