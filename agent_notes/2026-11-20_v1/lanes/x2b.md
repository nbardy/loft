# X2b · Ordered binary32 motion

Main started at `1194779`. The lane merged `origin/loft/d2a` and used its Naga lowering and integer binary32 helpers. The lane fixed the build, verified bounded raw loops on both operand orders, and kept the shipped Heron score on its original trajectory. The separate plan-driven Heron fixture exercises the inherited packed-prior sampler. The branch uses the normal drawable law entry for the three shipped characters and the same arithmetic pass for dedicated state solvers.

## Prediction

Each ordinary operation rounds to nearest, ties to even. Explicit FMA rounds the exact product and sum once. Both the CPU interpreter and Metal use the emitted integer operations and scalar-lane reduction order. This predicts zero differing state and verdict ticks and split-rounding words `3c162fca`, `3f51b2fb`, and `3c593be3` at the three prior first-divergence sites. The 30 and 60 Hz checkpoint and six-tick stall probes should reach the same words as 120 Hz. The 1,000-row test budget is 0.150 ms p95 against a prediction of 0.18 ms for the software divide and square root path. The shading program and effective Metal contraction policy should match main, with 0 ms structural delta in GPU frame time.

## Metal replay

Apple M4 Metal. Each trace includes tick 0 and 480 fixed steps at 120 Hz. Table 0 is the entire drawable state row. Table 2 contains the full packed verdict records. The probes use `Runner::step_bundle_cpu` and the emitted GPU entry from the same bundle.

| Scene | Row / verdict bytes per tick | Main differing state / verdict ticks | Candidate differing state / verdict ticks |
| --- | ---: | ---: | ---: |
| Crab Scramble | 60 / 64 | 371 / 481 | 0 / 0 |
| Heron Hop | 60 / 128 | 177 / 481 | 0 / 0 |
| Boss Hit 1 | 76 / 64 | 447 / 481 | 0 / 0 |

All three GPU traces also matched all 481 ticks at 30/60 Hz with seeks and a six-tick stall. Boss emitted the same two 32-byte records at tick 60 and zero drops. Crab lifted at tick 10 and its maximum planted drift fell from 0.000211901 to 0.000000000 mm. The 20 mm toe and shortened-reach mutants both missed at tick 0. Heron kept the 0.59 m knot and had zero rear-gaze misses. In the separate prior fixture, perturbing a baked knot moved the tick-1 word from `3f599965` to `3f5aed36` on both evaluators.

The dedicated raw solver matched all 32 output words against the Rust binary32 oracle and Metal, including the separated `00000000` versus explicit fused `a8800000` result, subnormals, NaNs, infinities and negative zero. The compiler accepted both comparison orders of a fixed raw loop and rejected the tested dynamic loop, counter mutation, wrapping loop, float atomic, raw global access, f16 and native `pow` with E504.

## Rendering

At 1280 × 800 and 0.5 s, Crab, Heron and Boss each differed from main at **0 / 1,024,000 pixels**. Their decoded RGBA SHA-256 values match the retained main captures. The earlier split-rounding-only revision differed at 0, 13 and 55 pixels, respectively. Tick-60 readbacks isolated the Heron hip and gaze and the Boss post-impact position and velocity. Explicit Horner rounding for the crouch and explicit binary32 reciprocals for the fish score and Boss mass restored the reference poses. The frozen references were untouched.

| Frame | Final decoded RGBA SHA-256 |
| --- | --- |
| Crab | `9d18ab5033df547173114e67b8bee097523b66a308212c7c524ff5a8e66c5879` |
| Heron | `401553ba707cfd617ff085bfa346c272b40e8611160298ee206e0c24160c3858` |
| Boss | `a5d8f485d67c9a53068f7167656aefd42afc2bf7b16822c246a4e6644863590f` |

Hermit-20 kept 8/8 WGSL modules and Shrine kept 7/7. Naga 30.0.1 emitted byte-identical MSL for HermitShell, CoralBranch and FoldedLantern Shade. The Hermit loop still emits the contractible accumulator `c = c + (sample * weight)`. On both builds the ordinary multiply-add witness matched explicit FMA on all 4,096 pixels and differed from forced separate rounding at 1,804.

| Apple M4 Metal profile | Timed frames per arm | Base → candidate p95, first pair (ms) | Base → candidate p95, reverse pair (ms) | Commands |
| --- | ---: | ---: | ---: | ---: |
| Hermit-20 orbit | 300 | 4.666 → 4.959 | 4.552 → 4.427 | 13 |
| Shrine, distance 6.4 at t=0 | 100 | 3.569 → 3.438 | 3.205 → 3.172 | 9 |

Each profile used 30 warmups, ring timestamps, and the shared GPU lock. Triangle medians were 0.028–0.053 ms in the final comparisons. Shade medians moved by −0.043/−0.005 ms for Hermit and −0.003/−0.0003 ms for Shrine. In the previous profile the Hermit frame p95 increased in both orders by 0.558/0.170 ms while the GPU programs and command counts were already identical. The direction of the final Hermit p95 changed with the order. These samples establish shader and command identity, with a variable frame-time tail.

## Tests and motion cost

The scoped 1,000-Crab dispatch measured 0.024 ms p95 on main, 0.207 ms on the initial strict lowering, 0.234 ms on the integer-kernel revision, and **0.180 ms** on the final contact revision against **0.150 ms**. Triangle medians were 0.096, 0.045, 0.097 and 0.030 ms, respectively. The second suite measured 0.154 ms, and the final workspace run passed that timing assertion. The standalone gate remains above budget. Crab now returns the authored toe coordinates exactly when the pin lies in the reachable annulus. Outside the annulus the checked projection still applies. The normalized 14-bit division digits and two-bit square-root steps matched the integer oracle on 100,000 and 200,008 cases, respectively.

The final workspace run passed 155/156 tests. The one failure was the source evaluator's missing scalar `fma` handler exposed by the explicit Heron expression. The final fix adds single-rounding `f32::mul_add` there. The focused source-motion rerun passed all 3 tests. The workspace check, workspace doctests, host GPU pin check and both 281/281 grammar recognizers passed.

[Raw traces, shader captures, profile samples and test logs](../../../out/meta/x2b/).
