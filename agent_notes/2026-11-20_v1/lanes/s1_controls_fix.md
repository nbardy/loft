# S1 control-pass parity

## Prediction before GPU measurement

Compare the Hermit-20 orbit at frames 0, 150 and 299 and Coral Shrine at
distance 6.4, solid 0, t=0, both at 1280 × 800. On Apple M4 Metal, aligning
the branch proxy, typed ray transform and cone/cap evaluation should take
Shrine from 20 differing IDs, 153,390 hit distances and 76,908 depths to zero.
The floor and lantern already match. Hermit should retain exact visibility
and depth. Its finite-difference field normals and lighting can amplify small
changes in floating-point evaluation into multiple stored HDR half ULPs.
Keep the one-half-ULP threshold and exact display comparison.

Expect Hermit near 0.99× and Shrine near 1.05× Loft/hand pass p95 based on
the inherited evidence. A clean interval below 1.05 earns a pass. A crossing
calls for reversed order. The Hermit double-dispatch mutant should yield a
lower 95% bound over 1.05 and exit 7, with a point estimate near 1.75×.

## Changes

Cambium branched from `aa1087e` and merged `origin/main` at `8e8bf19`
before implementation. The handwritten branch draw decodes the row once,
uses scalar proxy corner components and retains the separate inverse
translation and point transform. The cone and cap tests use the same axis
evaluation form. Loft's emitter and runtime have no lane edits.

`tools/check.sh parity` builds the optimized CLI and takes the GPU lock.
The timing comparison holds when the upper 95% bound is at most 1.05.
A lower bound above 1.05 returns exit 7. A crossing or contended measurement
with matching attachments retries in reversed order. A persistent crossing
or contention returns exit 6. An output mismatch cannot earn a pass.

## Output comparison

On Apple M4 Metal (macOS 15.5) at 1280 × 800, Shrine's indexed-cull readback returned
zero differences in IDs, hit distances, mirrored D32, HDR, and RGBA in
both launch orders. The inherited report had 20 differing IDs, 153,390
differing hit distances, and 76,908 differing depths. This compares the
whole branch arithmetic change as a unit.

Hermit reproduced the inherited attachment report byte for byte. IDs,
visibility payload, and depth matched in all three frames. HDR RG had
248 / 227 / 242 pixels over one half ULP and HDR BA had 249 / 230 / 252,
with a maximum of 73 half ULP. The display differed by one code value at
183 / 158 / 162 pixels. The half-ULP and exact RGBA checks still apply.

A diagnostic kept the RGB lighting computation and wrote the reconstructed
world normal's x component into HDR alpha. The half-float normal differed at
3,974 / 3,331 / 2,635 pixels. Of the 287 / 266 / 290 RGB pixels exceeding one
half ULP in that capture, 9 / 6 / 6 had a differing stored normal x. Ground
had no normal differences. The probe locates a source of drift in Shade's
surface reconstruction despite exact visibility. Hermit uses spline
derivatives for the shell and tetrahedral finite differences for the other
surfaces. Small changes in those results feed the normal-biased shadow
comparisons and the GGX lobes. The Metal contraction witness returned
`a8800000`. The capture retains one normal component at half precision,
so the individual rounding operations and the remaining HDR outliers
are unresolved.

## Timing and mutant

Each block ran 30 warmups and used the eight-slot timestamp ring. Hermit
timed 300 orbit frames per block and Shrine timed 100 fixed-camera frames.
The first locked run was contended. Its triangle medians ranged from 0.373
to 1.641 ms for Hermit and from 0.176 to 1.742 ms across both Shrine launch
orders, against a 0.350 ms cutoff. Its process exit was 6.

| First run | Loft / hand pass p95, ms | Loft / hand pass p95 ratio, 95% CI | Frame p95 ratio |
| --- | ---: | ---: | ---: |
| Hermit ABBA | 5.362 / 5.500 | 0.975 [0.898, 1.062] | 1.000 |
| Shrine ABBA | 8.794 / 6.957 | 1.264 [0.945, 1.512] | 1.000 |
| Shrine BAAB | 9.394 / 8.036 | 1.169 [1.084, 1.294] | 1.000 |

The first Hermit mutant repeated the five Shade dispatches and measured
2.074× pass p95, with a 95% interval of 1.947–2.137×. Its triangle medians
were 0.377–0.703 ms, so the measured interval exceeded the limit and the
process returned the contention exit 6. The historical uncontended mutant
returned exit 7 at 1.748×. The one mutant retry started BAAB and measured
1.637× (95% interval 1.573–1.844×). Its 0.865–1.482 ms triangle medians
again gave exit 6. An uncontended GPU exit 7 remains unverified on this lane.

## Checks

`tools/dev.sh check --workspace --all-targets` and the final focused
`tools/dev.sh check loft-cli --all-targets` passed. Naga validated both
whole-pass fixtures and the linked shader entries. The 21 CLI unit tests
passed, including the predicted 2.000× bootstrap and a case that checks
exit 7 for a lower bound above the 5% threshold. The latter also checks the
contended, inconclusive, and attachment-mismatch paths.

The shared build lock was occupied, so the CLI tests used the lane's
source and pinned Rust toolchain with its optimized dependencies.
The remaining 24 native test executables came from the primary checkout's
completed workspace build against the same renderer and library sources.
Across both sets, 154 of 156 tests passed. The retained CLI clip contract
exceeded its 50 ms GPU p95 cap with one frame over budget. The retained runner edit test
measured 0.572 ms promotion overhead against its 0.500 ms cap, with zero
of 140,000 pixels over 1/255.

The static check passed shell syntax, host pins and all 295 corpus
identities. I stopped the grammar run after checking that its source,
grammar, fixtures, author card and every tracked `.loft` file matched the
prototype's 281/281 Earley and llguidance run.

[Raw timing, attachment, diagnostic and test evidence](../../../out/meta/s1_controls_fix/)
