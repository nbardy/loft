# Phase 1.5 — E+ parser swap

Base 389da80 contains the syntax consolidation, proposed IR changes and the parser manifest. Its parser source directory is empty. The scientist's addendum and the three lead rulings govern the implementation.

## Predictions before implementation

| Instrument | Prediction |
|---|---|
| Phase 1 pictures, original approved RGBA hashes, three orbits × six cases | 18/18 |
| CPU/GPU twin, 60° off peak, material tolerance 2e-5 from ABI §5 | 36/36 |
| Frame p95 and recorded commands, original budgets | 12/12 on a quiet M4; 5 commands for blobs/capsules, 17 for hermit |
| Load, including identical module bytes at 1 and 20 | 33/33; hermit 8 modules, 13 pipelines, 11 distinct shapes at either count |
| Truth | 34/34 |
| Hermit warm front end + device load, cache primed | 45 ms at 1, 55 ms at 20 (budget 60 ms) |
| Hermit whole-file o200k_base tokens against the Phase 1 source | −36% (budget ≥35%); same tokenizer and treatment of comments for both |
| Normative Lark acceptance | 100% of v1 library, engine and bakeoff files |
| Lossless CST and semantic IR codec | byte-identical CST replay, canonical encode/decode/encode equality |
| Spec-only tasks, first successful compiler invocation | crab 3, reeds 2, heron 3 (cap 5); all 3 compile |
| Independent geometry checks | two distinct rays from reflected origins converge on the target; opposite signed turns on mirrored fish input |

The cache removes interval subdivision from the warm path. A placement changes the world bounds but preserves local derivations. A change to a called geometry helper, scan table, or a shape column must invalidate the corresponding derivation. The port must preserve arithmetic order, orbit parameters, row order, and layer order for exact reference hashes. Speed is measured on the device and all reference approvals remain those from Phase 1.

## Compiler feedback log

The first drafts are in commit a411e96. Each attempt below was a fresh invocation of the real Rust compiler with the shared library and Naga validation. Lark acceptance does not count as a compile.

| Attempt | Crab | Reeds | Heron |
|---|---|---|---|
| 1 | E001 at 28:25, a unit consumed the start of the next function name | E001 at 17:33, conditional expression in an argument | E001 at 4:19, parameter refinement |
| 2 | E405 at 16:13, AD through the named coil scan | E011 at 26:8, undeclared Sky | E003 at 7:10, angle parameter inference |
| 3 | Compiled and Naga-validated; 1 surface, 2 claws, 12 leg rows, 1 eye row, 1 sand row, 8 modules | Compiled and Naga-validated; 200 stalks plus water, 5 modules | E520 at 52:3, unlowered run_laws schedule entry |

Between attempts 2 and 3 the compiler acquired direct scan sampling, frame field names, contextual vectors and environment rows. The crab uses one lobe layout on both pupil branches and the prelude's fine value noise. The reed draft supplies its hash helper. The heron explicitly types its bearing, angle, eye and fish helpers.

The heron first compiled in round 4. At 132 GPU ticks the two fish paths gave opposite yaw. A behavioral test then caught an error that type checking, Naga and the signed-turn test all missed. Turning immediately on takeoff removed the neck error before the check could ever report a miss. In round 5 the law waits for 4 cm of lift to rotate. The real GPU now reports a positive missed angle during takeoff, then held lock and posture after turning. The compiler also cannot infer the intended sign of the beak pitch; the scene uses the inverse of its desired pitch in the domain map. The still path samples one fused row-local GPU kernel at 120 Hz and re-derives the sampled geometry bounds before drawing. Persistent live motion, checkpoints and cross-table solvers belong to Phase 3.

## Token counts

The numbers include the complete UTF-8 file, comments and final newline. The encoder was tiktoken `o200k_base` with `encode_ordinary`. Library and engine baselines are the Phase 1 sources at `81330c5`. The bakeoff baselines are the spec-only drafts at `a411e96`.

| File | Before | After |
|---|---:|---:|
| lib/analytic.loft | 640 | 942 |
| lib/bsdf.loft | 1,511 | 1,375 |
| lib/core.loft | 52 | 82 |
| lib/frames.loft | 304 | 301 |
| lib/layout.loft | 261 | 241 |
| lib/light.loft | 727 | 649 |
| lib/noise.loft | 437 | 421 |
| lib/sdf.loft | 474 | 345 |
| **Library** | **4,406** | **4,356** |
| engines/blobs.loft | 966 | 727 |
| engines/capsules.loft | 835 | 621 |
| engines/hermit.loft | 4,651 | 3,021 |
| **Engines** | **6,452** | **4,369** |
| Bakeoff crab | 1,288 | 1,428 |
| Bakeoff reeds | 637 | 679 |
| Bakeoff heron | 1,048 | 1,251 |

The Hermit reduction was 35.05% against a 36% prediction and a 35% gate. Three-segment capsule geometry shared by Hermit's leg query and ray test lives in the analytic library. The library and three ported engines together fell from 10,858 to 8,725 tokens (19.64%).

## Parser and bakeoff verification

- The independent Earley parser accepts all 14 files (eight libraries, three engines, three bakeoffs). The Rust CST replays all 14 byte for byte. Its raw-island fixture also covers Unicode, CRLF, same-line comments, nested WGSL comments and unmatched braces inside comments.
- The executable Hermit S-expression encodes, decodes through Naga validation and re-encodes byte for byte. The CLI-built file hashes to SHA-256 `569edb95a92a7779e6f9e7d78cc409c5e898c3299ef57a975d303e4231ba8048` when the scene is named `engines/hermit.loft`. The independent crab test rebuilds the shader modules from S-expression, checks author locations and compares every RGBA byte rendered at 160 × 100.
- The crab probe sampled both material branches at 141 angles and two times. All 564 results matched the independently computed rays from the reflected eye centers. Substituting the right origin on the left changes 62 decisions. The heron probe read GPU state and verdicts after 132 ticks. Positive and negative fish paths produced yaw +0.52000040 and −0.52000040 radians, a positive missed angle during takeoff, then held gaze and posture.
- Milestone renders are WebP quality 95. [Crab](v1_syntax/crab.webp), [reeds](v1_syntax/reeds.webp), [positive fish](v1_syntax/heron-side-1.webp), [negative fish](v1_syntax/heron-side--1.webp).
- Physical source lines, including comments and blank lines, against the row's estimates: `loft-parse/src` 1,631 vs 1,390, `loft-passes/src` net +540 vs +450, `lib/*.loft` 288 vs 280. The passes delta includes deleting all 741 lines of the Syntax A parser. The new parser count includes the lossless CST and its regressions. The passes also lower the executable bakeoff motion and preserve SSA author locations.

## Scored revision and merge decision

Implementation `ca21dbd`, Apple M4 (Metal), 1,280 × 800, 30 warm-up frames and 300 measured frames per scene. [Scored report, including every timestamp](v1_syntax/gate.json). Original reference PNGs were recovered from the Phase 1 compiler and verified against all 18 already-approved RGBA hashes. The Twin material tolerance is the ABI's 2e-5, the claw measured 1.115e-5.

| Gate | Predicted | Scored |
|---|---:|---:|
| Pictures | 18/18 | 18/18. 17 frames have exact RGB. Two of 1,024,000 pixels exceeded 2/255 on capsules-20 at t=0, within the specified 0.1%. |
| Twin | 36/36 | 36/36 |
| Speed | 12/12 | 6/12. All six command counts held. |
| Load | 33/33 | 33/33. Hermit had identical module text at 1 and 20, 8 modules, 13 pipelines and 11 distinct shapes at either count. |
| Truth | 34/34 | 34/34 |
| Hermit warm, 1 / 20 | 45 / 55 ms, cap 60 | 31.56 / 23.42 ms |
| Whole-file Hermit token reduction | 36%, gate 35% | 35.05% |
| Lark / lossless CST | 100% / byte exact | 14/14 / 14/14 |
| Typed IR encode/decode/encode | exact | exact; crab pixels also match after decoding |
| First successful compiler round: crab / reeds / heron | 3 / 2 / 3 | 3 / 3 / 4; heron behavior corrected in round 5 |
| Mirrored pupils / mirrored turn | independent origins / opposite sign | 564/564 samples, 62 discriminate / ±0.52000040 rad |
| Full Rust workspace | all pass | 24/24 in seven test binaries |

| Speed case | p95 budget (ms) | Measured p50 (ms) | Measured p95 (ms) | GPU commands |
|---|---:|---:|---:|---:|
| Capsules ×1 | 0.8 | 0.343 | 4.149 | 5 |
| Capsules ×20 | 1.1 | 0.588 | 4.400 | 5 |
| Blobs ×1 | 1.1 | 0.402 | 4.329 | 5 |
| Blobs ×20 | 2.4 | 1.311 | 6.223 | 5 |
| Hermit ×1 | 5.0 | 3.847 | 8.426 | 17 |
| Hermit ×20 | 5.6 | 10.469 | 12.785 | 17 |

**Branch retained, merge pending.** The shared M4 was also running a 3D game. A separate run of the original Phase 1 renderer on the same GPU missed both control budgets: capsules ×1 at 3.485 ms vs 0.8 and Hermit ×20 at 7.820 ms vs 5.6. An earlier unscored E+ run passed 11 of 12 Speed checks, missing capsules ×1 by 0.064 ms. The measurements establish sensitivity to machine load; they do not establish a passing quiet-device score. Acceptance still requires a green 300-frame gate on a quiet M4, then an upstream integration and repeat of the gates. Use `tools/dev.sh run loft-cli --fast -- gate` in this workspace. The verified PNG references are in `out/meta/refs/`. No reference approval was issued.
