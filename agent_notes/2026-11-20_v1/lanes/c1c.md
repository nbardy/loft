# C1c · Verify deterministic motion WGSL

Base `173d6e9a2b4fa36fcb45c0957f75e9c44f8e6dfa` from fetched `origin/main`.
The D2a experiment supplied typed-IR and provenance examples. The finite-loop
proof here uses the integer counter, guard and continuing update in Naga IR.

## Prediction

For each of law and solver, the archived typed identity, marked identity and
literal three-trip loop will exit 0. The unmarked identity, data-dependent while
and runtime pow will exit 1 with E504. This changes four of twelve verdicts.
At ticks 0, 1 and 3, the archived bounded law will store exactly 0, 4 and 12
and the solver will store exactly 1, 4 and 10 with default drive and one substep.
Declared bounds, local stores and scalar fixed transcendental calls will pass.
A transitive unbounded helper, indirect writes to motion storage and loop
counter overflow or mutation will return E504 at the author's operation.
Scheduled checks use the same verifier. Render-only raw WGSL keeps its normal
validation. Expect emitted motion shader bytes for Crab, Heron and Boss to match
the base and no added GPU commands. The verifier adds compile-time traversal of the
reachable shader graph and inspection of authored IR. Scalar wrapping adds one
reparse only for modules that use it. Frame p95 and full CPU/Metal rounding parity are
outside this probe.

## Verification boundary

The law/check and solver lowerers supply the declarations they emit. The same
verifier walks typed Naga calls from the compute entry and checks raw islands
and their authored callees. It checks the original marker and reports body
failures at the author's operation, with the raw island's existing provenance
as the fallback. Generated dispatch, binding and outbox code runs through
ordinary Naga validation. A raw island cannot call that scaffolding or reach
shader resources. Typed law and solver writes retain their declared ownership.

The loop proof recognizes a live integer counter, a compile-time entry value,
a constant ordered comparison, and a single nonzero constant update in
continuing. It rejects a counter write in the body or a terminal update that
would wrap. The proof covers nesting and early exit. More general finite
loops, runtime bounds and pointer arguments have no proof in this lane.
Runtime scalar `sin`, `cos`, `atan2`, `acos`, `exp` and `log` call the
existing fixed functions. Vector overloads have no fixed implementation here.
The verifier sees Naga's runtime expressions after constant folding. A folded
constant has no runtime math operation to check.

Full bit-equal CPU/Metal rounding, arithmetic contraction and authored
BODY/POSE execution remain with D2.

## Measurements

The archived CLI reproduced the original 12 observations. Its source revision
`a4f3e75` and the lane base have identical compiler, library and scene sources.
The candidate scored 12/12 against the prediction.

| Probe on each of law and solver | Base exit | Candidate exit |
| --- | ---: | ---: |
| Typed identity | 0 | 0 |
| Unmarked raw identity | 1 · E504 | 1 · E504 |
| Marked raw identity | 0 | 0 |
| Literal three-trip loop | 0 | 0 |
| Data-dependent while | 0 | 1 · E504 |
| Raw `pow(x,x)` | 0 | 1 · E504 |

All six legal `show` outputs matched byte for byte. Both unmarked identities
retained their original author spans. The four new refusals point to `while` or
`pow` in the raw body. [Before and after logs](../../../out/meta/c1c/) ·
[Archived input and original instrument](../../../out/meta/c1/det.py)

The all-targets check passed. The two-job optimized build took 84 seconds after
465.8 seconds waiting for the shared compile lock. All 72 tests passed across
nine Cargo-emitted executables. The new tests cover the law, solver and scheduled
check routes, transitive Loft and raw calls, storage writes, wrapper emission,
render-only math and generated outbox atomics. [Build artifacts](../../../out/meta/c1c/passes-build.jsonl) ·
[Test summaries](../../../out/meta/c1c/tests-summary.log)

On Apple M4 Metal, both literal and declared trip bounds gave `[0,4,12]` for
the law and `[1,4,10]` for the solver at ticks 0, 1 and 3. The default parameters
and one row were used in each of four cases. All twelve CPU/Metal readback pairs matched their
predicted bits. Each nonzero tick sample used one dispatch
from fresh initial rows and measured 0.008375–0.010292 ms with GPU timestamps.
Tick-zero and frame p95 timing were unmeasured. The readback harness initially
hit a Rust path type mismatch, then a log-directory collision before dispatch.
The measured readback and all six stills ran with the GPU lock.
[Readback and timing](../../../out/meta/c1c/gpu-readback.log) ·
[Packed-word hashes](../../../out/meta/c1c/readback-sha256.json)

Crab, Heron and Boss each retained one fixed-step dispatch and emitted 6, 7
and 6 byte-identical shader modules respectively. Their 0.5 s stills at
1280 × 800 with defaults matched all 3,072,000 RGBA pixels. Lossless PNG and
decoded RGBA files, elapsed wall times, exact commands and exits are in
`out/meta/c1c/`. [Shader hashes](../../../out/meta/c1c/shipped-shaders.json) ·
[Pixel comparisons](../../../out/meta/c1c/pixels.json)

[Static check output](../../../out/meta/c1c/static.log) ·
[Command ledger](../../../out/meta/c1c/commands.txt)
