# S1 · Whole-pass controls

Base: `bca209d15a760bab2623957c13ef257e06fe46cf`.

## Prediction before implementation

On Apple M4 Metal I expect identical packed visibility, HDR and RGBA on the
paired replay. The control shaders preserve the authored arithmetic order,
12 sky samples and the lantern's 192-step bound. PF1 measured Hermit Shade
p50/p95 at 2.188/3.096 ms and Shrine Vis at 2.150/2.684 ms (pass-end
increments). For complete timestamp intervals I expect both p95 ratios in
0.90–1.10, with approximately 1.0 as the point estimate and a lower 95%
bound below 1.05. With the same complete pass issued twice on the Loft arm,
I expect Hermit Shade's ratio in 1.8–2.2, with the lower bound above 1.05.
Each fixture gets 30 warmups per block, then 1,200 Hermit or 400 Shrine
measured frames over four blocks. The paired bootstrap resamples consecutive
replay times jointly across all four blocks. I expect triangle medians below
0.350 ms and zero contraction-off or safe-math directives.

## Implementation and score

The hand WGSL reads the same scene tables and renders the complete five-dispatch
Hermit Shade and three-draw Shrine Vis. Loft's frame scheduler and eight-slot
timestamp ring execute both arms on one Apple M4 Metal device and queue
(macOS 15.5, 24F74). Each block starts with a triangle control and 30 warmups.
The replay keeps 300/100 measured frames per block. Parity captures use
indexed cull in both arms to fix equal-depth draw order. Hermit captures
frames 0, 150 and 299 and Shrine captures its constant t=0 view. In timed
Shrine B, a pair of shadow-only kind aliases keeps the emitted caster
pipelines and sun frustum. Two cull bounds records account for the aliases.

| Fixture | Loft p50 / p95 ms | Hand p50 / p95 ms | Pass p95 ratio, 95% CI | Frame p95 ratio |
| --- | ---: | ---: | ---: | ---: |
| Hermit Shade | 2.187 / 3.747 | 2.275 / 3.801 | 0.986 [0.978, 0.992] | 0.977 |
| Shrine Vis | 2.073 / 2.542 | 2.004 / 2.408 | 1.056 [0.967, 1.120] | 1.025 |
| Hermit Shade mutant | 5.017 / 6.744 | 2.420 / 3.857 | 1.748 [1.658, 1.821] | 1.407 |

The four blocks use begin-to-end pass durations and earliest-start to
latest-end frame durations. The paired circular bootstrap samples ten
adjacent replay indices at a time and reuses those indices across all
four blocks in each of 4,096 draws. Triangle medians were 0.0277–0.0530 ms
on the final controls, below the 0.350 ms cutoff. Shrine's point estimate
exceeded 1.05 and its interval crossed the limit. Neither regular control
earns a Speed verdict because both failed attachment parity.

Hermit's visibility IDs, packed payload and D32 values matched at every
pixel in all three captured frames. RGBA differed at **183 / 158 / 162**
pixels, all exactly one code value. HDR RG differed at **2,618 / 2,171 / 2,231**
pixels, with **248 / 227 / 242** over one half ULP. HDR BA differed at
**2,118 / 1,750 / 1,831**, with **249 / 230 / 252** over one half ULP.
The largest HDR difference was 73 half ULP. The second revision removed
the Ground RGBA differences and reduced overall differences. The existing
one-ULP HDR threshold still fails.

Shrine had **20** differing IDs, **153,390** differing hit-distance words
(**25,737** over one f32 ULP), **76,908** differing depth values (**13,570**
over one f32 ULP), and **99** differing RGBA pixels (**90** over one code
value). All differences were on the analytic branch draw. The floor and
192-step folded lantern matched in all compared outputs. The follow-up
matched the first run's Shrine diff counts after restoring the source-level
ray and cone evaluation order. The rounding source for the branch
differences remains unresolved.

The mutant repeated all five emitted Shade dispatches, raising the frame
command count from 17 to 22 while retaining the same draw output. The
harness exited 7 with a slowdown diagnostic from the CI. The p50 ratio was
**2.073×** and the p95 ratio **1.748×**, below the predicted 1.8–2.2 p95
range. The synthetic bootstrap unit case returned exactly 2.000 for the
point estimate and both bounds.

Naga validated standalone control sources and linked pipelines. Across
five emitted Hermit compute entries and six emitted Shrine vertex and
fragment entries, WGSL and the offline Naga MSL translation had zero
contraction-off and safe-math directives. The Metal device's buffer-fed
default compilation witness returned `a8800000`, the fused result.

The evidence includes the preflight readback capability failure, both
scored runs, exact attachment digests, raw stamps, bootstrap results and
mutant output. The preflight diagnostic was changed to recover stored half
bits with integer operations. The lane used its allowed measurement
reruns. [Raw evidence](../../../out/meta/s1_controls/)

## Regression checks

`CARGO_BUILD_JOBS=2 tools/dev.sh check loft-cli --all-targets` passed.
The static suite verified 295 corpus identities and 281 grammar fixtures
in both recognizers. The workspace passed 153 of 154 tests, including all
CLI tests. The existing runner Tier 0 promotion timing test measured
0.537 ms against its 0.500 ms cap, with zero image differences over
1/255. The default gate exited 3 for its frozen picture comparison.
All 18 actual RGBA hashes matched retained main's i1 evidence. The five
gate counts were Pictures 0/18, Twin 36/36, Speed 8/12, Load 27/33, and
Truth 34/34.
