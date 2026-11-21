# v15_bloat

The implementation started at the unified v15 execution merge `5e252da`.
The branch includes current `origin/main` at `78ef244`. The intervening
main commits changed orchestration and notes. The baseline compiler, renderer,
and corpus are identical between those revisions.

Prediction before regeneration. Each seed keeps its original strip count, declaration order,
f32 row words, camera, sun, grid and keyframe arithmetic. The source becomes one
indexed build loop per sweep kind while the seven legacy CPU samples become
binary tables. The corpus median should be below 4,000 o200k tokens. The
deterministic scorer should hold for RGBA, ID and D32 at the four manifest times
at 512² and the 1.37 s detail plate at 1280×800, plus four between-knot samples
on train-16/2. Every generated port has one camera.

The inherited sampler is rooted at `a19a5ae`. The new encoding uses an ordered
typed header, explicit row count, and a per-row sparse mask. Paint and exponent
use per-kind modes, and absent delta columns use the existing zero defaults.
AD specializes JVPs to active arguments, so the sweep helper receives exponent
and sampled ring axes as primal values with no tangent arguments.

## Measurements

The updated base has 187 live scenes. Its median is 96,240 tokens and its
nearest-rank p95 is 198,888. TOK1 measured 96,204 at an earlier revision. Counts
include all source text, using tiktoken 0.14.0 and `o200k_base`.

Ignored verification evidence is under `out/meta/v15_bloat/` in the primary checkout.

| Live corpus, 187 scenes | Before | After |
| --- | ---: | ---: |
| Median tokens | 96,240 | 3,983 |
| Nearest-rank p95 tokens | 198,888 | 7,340 |
| Total tokens | 18,868,887 | 786,454 |

Keeping the sweep arithmetic inline in otherwise identical generated source
costs 898,246 tokens. Inactive-argument AD enables the shared helper and saves
111,792 tokens across the corpus, including its imports. The helper lives once
in `stdlib/geometry/sampled_sweep.loft`.

The 170 sampled ports went from 39,057,541 source bytes to 1,545,811 source
bytes and 7,939,163 binary bytes across 1,121 tables. Both Creative JSONL
checksums held. All 51,234 sampled rows match the original f32 words,
including modal defaults and zero deltas. The legacy CPU sampler checked
3,927,476 vertices with maximum error 5.25 × 10⁻¹² m.

The three compilation commands were the CLI all-targets check, the focused
passes suite, and the optimized workspace verification build. The first
focused run passed the binary decoder regression and stopped on the new AD
fixture's point/vector mismatch. The corrected AD fixture passed from the
final build. The normal CLI built a regenerated port into a portable bundle.

All 170 regenerated sources passed strict lint. The full check enforces lint
on these ports and reports one warning on the retained Shatter example.
Standalone lint exits 0 by default and exits 7 with `--strict` on an original
unrolled seed. The token cap and repeated-block check run only in lint.

The final CLI compiled all 187 live sources through `loft show --plan`.
Static checks passed, including expected results from Earley and llguidance
on 307/307 files.

The deterministic v15c gate passed all 854 paired frames across all 170
regenerated scenes on Apple M4 Metal under the GPU lock. ID and D32 matched
exactly in every frame. RGBA matched exactly in 556 frames. The maximum
channel drift was one code, with at most 0.00191% of pixels changed in any
frame. The stochastic gate accepted the candidate's reference-sphere sequence
against the v15-exec reference at 41.73–42.07 dB and temporal variance
4.4914 × 10⁻⁵. It rejected the gain mutant at 6.74 dB.

The optimized native workspace suite had one timing failure. The runner's
Tier 0 edit-gate overhead measured 0.830 ms p95 against a 0.500 ms cap,
with zero pixels over 1/255. The v15-exec final suite also recorded a timing
miss in this test.
