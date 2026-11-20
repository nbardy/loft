# PF1b · Group the long SDF march tail

## Prediction before implementation

Execute iterations 1–24 with the original scalar loop, then iterations 25–192 in 42 groups of four. Emit each sample from one source. A hit takes precedence over the advance and keeps the current distance bits, crossing the far bound ends the march after the advance, and exhausting iteration 192 discards. The groups eliminate 126 of 192 loop iterations for an exhausted ray. A hit on iteration 24 skips the tail. A hit on iteration 192 still wins. The diagnostic counts the evaluated samples, including the hit sample. Both cameras use the same fragment entry and checked module.

PF1 attributed 81.501% of successful Shrine steps and 3.585% of successful Hermit steps at 0 s to the tail. Scaling the fully grouped measurements predicts 0.313 ms saved in Shrine Vis and 0.012 ms saved in Hermit Vis plus Shadow before transition costs. This excludes misses and hidden fragments. The Shrine target is at least 0.250 ms Vis p50 improvement and at most 3.800 ms frame p95. Hermit should stay near the matched control, within 2% and at or below 5.600 ms frame p95. PF1 observed Hermit p95 drift from 5.585 to 6.775 ms between runs, so score the absolute and matched limits independently.

Predict zero decoded changes in beauty, depth, ID and SDF-step images at 0/4/9 s, with matched ascending and descending Shrine rows. Expect 12 Shrine and 17 Hermit frame commands and the matching main pipeline counts. Measure natural cull order with separate resident A/B scenes on the same queue, per-arm triangle controls, 30 warmups, and 100 fixed Shrine frames or 300 orbiting Hermit frames at 1280 × 800. Use the latest GPU pass end for frame completion. Score the tail in isolation from sun-map reuse.

## Implementation and first run

The fragment emitter writes one step body into the scalar loop and the four slots of the tail group. The explicit exit reason carries an interval exit across the prefix boundary and distinguishes it from the cap. A hit retains the distance before the advance. The step AOV increments before each field evaluation and packs the count into the same low byte. The main and sun views share the checked fragment entry. Module creation, pass/row assignments, author inputs, and the Tier 0 edit march follow their existing paths.

The first A/B/B/A was on Apple M4 Metal, macOS 15.5, one queue. The instrument averages the two blocks' quantiles per arm. Pass times use consecutive ends for attribution, and raw intervals and frame ticks are retained. Present was the latest end in all 1,600 timed frames.

| Scene | A1 frame p95 | B1 frame p95 | B2 frame p95 | A2 frame p95 | A/B Vis p50 | A/B frame p95 |
|---|---:|---:|---:|---:|---:|---:|
| Shrine | 3.979 | 3.579 | 3.593 | 3.797 | 2.109 / 1.812 | 3.888 / 3.586 |
| Hermit-20 | 5.472 | 5.313 | 5.119 | 5.181 | 1.402 / 1.334 | 5.327 / 5.216 |

All times are milliseconds. Shrine saved 0.297 ms on attributed Vis p50 and 0.309 ms on the pass's own timestamp interval. Hermit stayed below 5.600 ms and was 2.08% faster than control. This falls 0.08 percentage points outside a symmetric 2% band. Triangle medians in A/B/B/A order were 0.029/0.031/0.028/0.028 ms on Shrine and 0.096/0.031/0.030/0.031 ms on Hermit. All were under the 0.350 ms contention threshold. The different first Hermit triangle median and the 0.292 ms spread between Hermit control tails motivated confirmation.

All 36 pairs of decoded 1,024,000-pixel images matched. This covers four AOVs at three cameras for each fixed Shrine order and Hermit's natural order. Raw counts held at 12/17 frame commands on Shrine/Hermit, with 7/8 shader modules, 5/6 render pipelines and 6/8 compute pipelines. [First-run timings, all pass quantiles, comparison counts and RGBA hashes](../../../out/meta/pf1b/first/summary.json)

Comparing main's ascending order with main's descending order changed 252/386/301 beauty pixels and 54/84/65 ID pixels at 0/4/9 s. The paired comparisons use the same order on both arms. [Shrine AOV sheet](../../../out/meta/pf1b/first/shrine-ascending.webp)

## Confirmation and boundary readback

All 36 requested lossless image pairs and 27 raw visibility readback pairs had zero differing pixels at 1280 × 800. The raw pairs exposed the kind/row word, the full hit-distance word and the diagnostic distance/count word through RGBA8Unorm. Shrine passed both fixed-order comparisons before its natural-cull timing. Shadow work used the same checked fragment entry and was included in each timed arm. [Decoded comparisons](../../../out/meta/pf1b/confirm/summary.json)

| Scene and row order | Beauty | Depth | ID | SDF steps |
|---|---:|---:|---:|---:|
| Shrine ascending | 0/0/0 | 0/0/0 | 0/0/0 | 0/0/0 |
| Shrine descending | 0/0/0 | 0/0/0 | 0/0/0 | 0/0/0 |
| Hermit natural | 0/0/0 | 0/0/0 | 0/0/0 | 0/0/0 |

A locked Metal compute witness ran the emitted march bodies against scripted binary-exact distance samples. The control, candidate and independently predicted outputs matched in all ten cases.

| Witness | Evaluations | Distance bits | Exit |
|---|---:|---|---|
| Early hit | 1 | `00000000` | Hit |
| Early interval exit | 1 | `3e000000` | Interval |
| Prefix's last sample hits | 24 | `40380000` | Hit |
| Tail's first sample hits | 25 | `40400000` | Hit |
| Prefix ends beyond the interval | 24 | `40400000` | Interval |
| Tail advances beyond the interval after equality | 25 | `40480000` | Interval |
| Last allowed sample hits | 192 | `41bf0000` | Hit |
| Hit wins before interval test | 1 | `3f800000` | Hit |
| Exhaustion | 192 | `41c00000` | Cap |
| Negative near clamps to zero | 1 | `00000000` | Hit |

[GPU boundary results](../../../out/meta/pf1b/confirm/boundaries.json) · [Witness source](../../../out/meta/pf1b/boundaries.rs)

### Confirmation on one queue

All values are milliseconds. Triangle controls in A/B/B/A order were 0.029/0.032/0.031/0.030 on Shrine and 0.102/0.032/0.032/0.029 on Hermit. Present ended last in all 1,600 timed frames. The candidate saved 0.308 ms attributed Vis p50 on Shrine and 0.304 ms by raw Vis interval. Command, module and pipeline counts matched the first run.

| Scene | Pass | Main p50 / p95 | Candidate p50 / p95 |
|---|---|---:|---:|
| Shrine | Cull | 0.008 / 0.009 | 0.008 / 0.009 |
| Shrine | Vis | 2.104 / 2.469 | 1.796 / 2.108 |
| Shrine | Shadow | 0.447 / 0.713 | 0.453 / 0.755 |
| Shrine | Classify | 0.000 / 0.123 | 0.000 / 0.052 |
| Shrine | Shade | 0.369 / 0.744 | 0.362 / 0.762 |
| Shrine | Present | 0.171 / 0.201 | 0.171 / 0.247 |
| Shrine | Frame | 3.032 / 3.556 | 2.732 / 3.177 |
| Hermit | Cull | 0.004 / 0.006 | 0.004 / 0.007 |
| Hermit | Vis | 1.395 / 1.651 | 1.321 / 1.522 |
| Hermit | Shadow | 0.636 / 1.049 | 0.593 / 0.967 |
| Hermit | Classify | 0.000 / 0.241 | 0.001 / 0.186 |
| Hermit | Shade | 2.125 / 2.849 | 2.082 / 2.904 |
| Hermit | Present | 0.147 / 0.443 | 0.147 / 0.380 |
| Hermit | Frame | 4.481 / 4.944 | 4.316 / 4.836 |

[Hermit per-frame ticks and input bits](../../../out/meta/pf1b/confirm/hermit-timing.json) · [Shrine per-frame ticks and input bits](../../../out/meta/pf1b/confirm/shrine-timing.json)

The confirmation repeated both Shrine passes and the Hermit absolute pass. Hermit was 2.19% faster than control, outside a symmetric 2% band. Both timed trials started Hermit A1 with a triangle median near 0.10 ms before dropping to about 0.03 ms on later blocks. The final permitted replay preconditions the queue with one additional untimed triangle-control batch before Hermit's first timed block. Each arm retains its own triangle control and 30 warmups. This should remove the first-triangle disparity. Expect roughly 0.30 ms saved in Shrine Vis and a candidate Hermit p95 below 5.600 ms. Score the 2% band separately.

## Final replay and verdict

The extra triangle batch measured 0.102 ms and Hermit A1 still measured 0.102 ms. Subsequent Hermit triangle controls were 0.031/0.031/0.028 ms. Shrine's were 0.030/0.030/0.033/0.029 ms. This replay exhausted the timing rerun budget. All controls were below 0.350 ms. The first-triangle disparity persisted and its effect on frame tails remains unknown.

| Run | Shrine Vis p50 saved | Shrine frame p95 A / B | Hermit frame p95 A / B | Hermit change |
|---|---:|---:|---:|---:|
| First | 0.297 | 3.888 / 3.586 | 5.327 / 5.216 | −2.08% |
| Confirmation | 0.308 | 3.556 / 3.177 | 4.944 / 4.836 | −2.19% |
| Preconditioned | 0.236 | 4.154 / 3.826 | 5.458 / 5.191 | −4.89% |
| Mean of the three matched runs | 0.280 | 3.866 / 3.530 | 5.243 / 5.081 | −3.09% |

Times are milliseconds. Both Shrine targets held twice and missed on the third run by 0.014 ms in Vis and 0.026 ms in frame p95. Both candidate blocks in every Hermit replay held the 5.600 ms budget. The candidate mean was faster in all three Hermit runs, outside the literal ±2% band each time. The lane therefore has exact picture and visibility parity, stable command and pipeline counts, and mixed timing acceptance. [Final replay pass quantiles and controls](../../../out/meta/pf1b/preconditioned/summary.json) · [Hermit per-frame ticks](../../../out/meta/pf1b/preconditioned/hermit-timing.json) · [Shrine per-frame ticks](../../../out/meta/pf1b/preconditioned/shrine-timing.json)

## Checks and reproduction

The pinned lab check of `loft-passes` passed. The full native workspace suite passed 153/153 across 25 executables, including the renderer and Tier 0 tests. Doctests and the wasm32 `web-sys` check passed. Static checks verified the corpus, vocabulary, shell, GPU pins, and 281/281 grammar outcomes in both recognizers. The wasm check reported the existing unused `Kernel::layout` warning. [Check log](../../../out/meta/pf1b/candidate-check.log) · [Native suite](../../../out/meta/pf1b/suite-native.log) · [Static checks](../../../out/meta/pf1b/static.log)

The main control was compiled at `5527cba` from the same authored scene sources and inputs. The serialized program audit matched all non-module fields, including camera, schedules, instances and packed rows. Only CrabFace, CrusherClaw and FoldedLantern changed. Each changed WGSL module matched byte for byte outside the march block, for all four AOVs. The evidence retains the compiled main/candidate programs, probe sources, build logs, lossless PNGs, complete per-frame timestamp pairs, and decoded RGBA hashes. The pair probe loads both programs on one queue and uses natural culling for timing. The sun-map lane was scored separately. [Pair source](../../../out/meta/pf1b/pair.rs) · [Quantile and comparison script](../../../out/meta/pf1b/score.py)
