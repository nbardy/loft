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

The first drafts are in commit a411e96. The 3/3/4 counts are first compilations during compiler and scene co-development. Each attempt below was a fresh invocation of the real Rust compiler with the shared library and Naga validation. Lark acceptance does not count as a compile.

| Attempt | Crab | Reeds | Heron |
|---|---|---|---|
| 1 | E001 at 28:25, a unit consumed the start of the next function name | E001 at 17:33, conditional expression in an argument | E001 at 4:19, parameter refinement |
| 2 | E405 at 16:13, AD through the named coil scan | E011 at 26:8, undeclared Sky | E003 at 7:10, angle parameter inference |
| 3 | Compiled and Naga-validated; 1 surface, 2 claws, 12 leg rows, 1 eye row, 1 sand row, 8 modules | Compiled and Naga-validated; 200 stalks plus water, 5 modules | E520 at 52:3, unlowered run_laws schedule entry |

Between attempts 2 and 3 the compiler acquired direct scan sampling, frame field names, contextual vectors and environment rows. The crab uses one lobe layout on both pupil branches and the prelude's fine value noise. The reed draft supplies its hash helper. The heron explicitly types its bearing, angle, eye and fish helpers.

The heron first compiled in round 4 and its behavior was corrected in round 5. At 132 GPU ticks the two fish paths gave opposite yaw. A behavioral test then caught an error that type checking, Naga and the signed-turn test all missed. Turning immediately on takeoff removed the neck error before the check could ever report a miss. In round 5 the law waits for 4 cm of lift to rotate. The real GPU now reports a positive missed angle during takeoff, then held lock and posture after turning. The compiler also cannot infer the intended sign of the beak pitch; the scene uses the inverse of its desired pitch in the domain map. The still path samples one fused row-local GPU kernel at 120 Hz and re-derives the sampled geometry bounds before drawing. Persistent live motion, checkpoints and cross-table solvers belong to Phase 3.

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
- Physical source lines, including comments and blank lines, against the phase forecasts: Runner 942/1,000, IR plus gates 1,419/1,500 (1,473 including the frame-budget helper), `loft-parse/src` 1,631 vs 1,390, `loft-passes/src` net +540 vs +450, `lib/*.loft` 288 vs 280. The passes delta includes deleting all 741 lines of the Syntax A parser. The new parser count includes the lossless CST and its regressions. The passes also lower the executable bakeoff motion and preserve SSA author locations.

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

**Historical Speed verdict — CONTENDED.** The lead ruling at bc241a7 treats the timing misses as warnings. The original Phase 1 control on the same shared M4 also missed (capsules ×1 at 3.485 ms against 0.8 and Hermit ×20 at 7.820 ms against 5.6). All six command counts held. An earlier E+ run scored 11/12.

**Historical Load measurements.** The phase instrument at ca21dbd recorded 33/33. The independent review of 4fd93bf held 25/33, with six cold misses and Hermit warm at 72.17 and 80.19 ms. The corrected Load instrument starts before reading files and uses a new derivation directory and pipeline key per case, then reuses both for warm.

## Review corrections

The [review Outcome](../2026-11-20_v1/reviews/p15.md#outcome) records the finding-by-finding dispositions. Source counts after the review are Runner 998/1,000, IR plus gates 1,681/1,500 (1,737 with the budget helper), parser 1,729 against the 1,390 forecast, passes 6,573 and net +973 against +450, and library 292 against 280. The parser, passes and library row figures are forecasts. IR plus gates exceeds its ceiling by 181 lines, or 237 including the budget helper.

### Corrected gate run

Apple M4 Metal, 1280 × 800, 30 warm-up frames and 300 measured frames per case. The cold and warm measurements finish at device load. These run before the frame batches. The approved reference images and hashes were preserved.

| Gate | Result |
| --- | --- |
| Pictures | 18/18 |
| Twin | 36/36 |
| Speed | 8/12 raw, all 6 command counts held. **CONTENDED** under the lead's Phase 1 control ruling. |
| Load | 33/33, including every module, pipeline, shape and identity check |
| Truth | 34/34 sampled geometry checks. Separate authored-check tests exercised GPU Held, Missed and Unknown. |
| Workspace | 43/43 |
| Decoder and Rust parser | All 14 original files, the default recipe, and both shared fixtures gave the expected result |
| Whole-file Hermit tokens | 4,651 → 3,021, −35.05% |
| Hermit S-expression | Byte-identical encode/decode/encode, SHA-256 1b921f53ca3280c121b3198019c4fa5754f779d514191e0a580f7ed3179ca829 |

| Case | Commands | p95 ms | Triangle control p50 ms | Cold ms | Warm ms | Derivation hits cold / warm |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| Capsules ×1 | 5 | 1.394 | 0.096 | 332.65 | 4.29 | 0 / 7 |
| Capsules ×20 | 5 | 2.489 | 0.096 | 362.31 | 5.08 | 0 / 140 |
| Blobs ×1 | 5 | 1.061 | 0.053 | 332.70 | 4.34 | 0 / 1 |
| Blobs ×20 | 5 | 3.337 | 0.180 | 433.74 | 4.67 | 0 / 20 |
| Hermit ×1 | 17 | 3.716 | 0.031 | 904.68 | 15.55 | 0 / 11 |
| Hermit ×20 | 17 | 10.044 | 0.045 | 964.78 | 16.32 | 0 / 11 |

The built-in triangle control held, so the raw CLI run reports four Speed misses and exits 7. In the immediately following Phase 1 control, capsules ×1 measured 1.011 ms against 0.8 and Hermit ×20 measured 10.265 ms against 5.6. The lead's ruling covers those Speed misses. Two earlier measurements with the corrected cache isolation held 31/33 and 32/33 on Load. The scored device-load run held 33/33 without a Load exception.
