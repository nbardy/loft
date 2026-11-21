# cap1b · Particle transfer

## Predictions before execution

Every fixture has 65,536 unit-mass particles and a periodic 32³ transfer grid. An
axis visits distinct nodes, so each particle writes a node at most once. The
global bound K is 65,536 and is attained by coincident trilinear particles.
Both channels use 2¹² counts per unit. Mass is nonnegative in an i32 word.
The maximum absolute contribution is 4,096 counts, so every partial sum is
bounded by 2²⁸, even when signed terms cancel. The degree-two stencil uses
integer axis weights over 512, whose maxima of 384 bound its product by
56,623,104. The largest momentum numerator including the rounding bias fits
u32 (3,567,255,552 + 1,032,192). Round nearest, ties away from zero. The
per-contribution error is ≤1/8192. The trilinear fixture has exact mass weights.
The fixed atomic and sorted exact routes should match every i64-twin word.
A count of 65,537 should return Capacity and leave the published grid intact.

The 8/27-tap transfers issue 524,288/1,769,472 node visits. I expect fixed P2G
medians of 0.5–1.0/1.6–3.4 ms on M4. The 27-tap frame budget is 16 ms p95 at
1280 × 800, including grid update, G2P and particle drawing. Native f32 uses
four partial grids with a fixed pairwise finish. I expect 0.5–1.2 ms median
P2G, normalized conservation ≤1e−5 and max cell error ≤1e−4 across all
seeds and both orders. Compare 120-step velocity and energy with a separate
f64 FLIP twin. The browser path targets 3–9 ms median for clear, stable radix
sort, range creation and gather. Its work includes 8,323,072 local rank
comparisons for the two digit passes. Sort scratch is 1,836,032 B, including
the two key tables, tile histograms, digit offsets and ranges.

## Authored contracts and reproduction

- `scenes/particle_flip.loft` is the 27-tap quadratic FLIP showcase.
  The linear, native and sorted controls use the same authored compute ABI.
  The 27-tap kernel uses the tensor product of quadratic B-spline weights
  with the same FLIP/PIC update. It has no MPM affine or stress term.
- The integer envelope is `abs(v) ≤ 63` with velocities encoded as `v/63`,
  unsigned sixteenth-cell positions, and a one-word host count. The host
  fixtures construct valid rows. The solver constructs packed rows by
  projecting positions and velocities into that envelope. The periodic
  stencil wraps across the transfer grid. Grid and particle boundary conditions
  then contain the splash in the tank.
- Exactness here covers the integer deposition boundary. Raw WGSL carries
  conservative NonDet source provenance under the existing v15b contract.
  Grid normalization, 92% FLIP / 8% PIC and positional integration use
  floating point. The step applies gravity and wall response without a pressure
  projection. The 120-step test measures velocity and kinetic-energy drift.
- The native scatter uses the landed v15b typed kernel and chooses
  `FLOAT32_ATOMICS` per pass from the grant on the actual device. The native
  fixture has one status check, scatter, pairwise publication and scratch clear.
  The browser sorts by wrapped base cell, with stable upload row order for IDs
  within a cell, then uses one integer writer per destination. The browser host
  projects its bind layouts and dispatch extents from the decoded `.loftb`.
  No shared link, checker or runner change is required.
- Reproduce with `tools/dev.sh build loft-cli --fast --example particle_flip`,
  then `tools/gpu_lock.sh target/lab/examples/particle_flip EVIDENCE_DIR`.
  Serve that directory with `uv run --no-project python web/particle_flip/serve.py EVIDENCE_DIR`.
  Open the given URL for the live splash, or append `?bench` to force sorted
  gather and record WebGPU readback and timing. Run browser GPU work under the
  same lock. Timing uses alternating paired order, eight warmup pairs, 64
  measured pairs (300 for the native showcase), and a paired bootstrap 95%
  interval for the p95 difference. Fixture transfer timing excludes upload
  and readback. Full step timing includes packing, the grid update and G2P.

## Source checks

The checkout was created from `origin/main` at `5e252da`, the unified
v15_exec merge. The subsequent main updates through `78ef244` changed
orchestration and notes. The baseline exported all four authored scenes. The exact
consumer fixture returned E504 with the native `pass p2g` producer span.
Earley and llguidance both accepted all four scenes. The typed native scatter
reads storage row fields individually. A whole-row local currently refers to
an undeclared nominal WGSL type in the v15b emitter. This lane uses the existing
field access and pass ABI. The frozen references were untouched.

## Native readbacks and timing

On Apple M4 Metal, both fixed stencils matched all 3,145,728 words of the
i64 twin across the two orders and four fixtures, including each capacity
rejection. Count 65,537 produced Capacity and left each published grid
intact. The native device granted `SHADER_FLOAT32_ATOMIC` and
`TIMESTAMP_QUERY`. All 128 f32 readbacks were finite and held the statistical
budgets. The maximum normalized global error was 8.22e−10 spread and
5.18e−9 clumped. The maximum absolute cell error was 5.00e−7 spread and
3.69e−5 clumped, against 1e−4. The maximum cell RMS error across the suite
was 2.68e−7.

The ordinary native FLIP scene advanced 120 steps against the independent f64
twin. At step 120, velocity RMS error was 0.00415, maximum particle velocity
error was 0.19840 and relative total kinetic-energy error was 2.80e−6.
Across all 120 checkpoints, the maxima were 0.00415, 0.51753 and 6.40e−6.
The largest component of the mean-velocity difference at step 120 was
1.03e−4. Velocity uses scene units per second and energy uses unit particle
mass. These are drift observations for the trilinear f32 solver.

Both managed compile slots were held for the GPU run. The 1280 × 800
triangle p95 was 0.099 ms before and 0.110 ms after. GPU timing spans the
first to the last recorded pass, with query resolve and readback outside
the interval. Values in the table are p50/p95 in ms.

| Fixture and route | P2G | Grid update + G2P | Full GPU interval |
| --- | ---: | ---: | ---: |
| Spread, fixed8 | 0.994/1.000 | — | 1.056/1.062 |
| Spread, native f32 | 0.988/0.992 | — | 1.065/1.069 |
| Clumped, fixed8 | 1.074/1.098 | — | 1.137/1.194 |
| Clumped, native f32 | 0.952/0.974 | — | 1.045/1.228 |
| Splash, fixed8 | 0.980/1.151 | 0.126/0.188 | 4.043/4.743 |
| Splash, fixed27 | 3.170/3.316 | 0.321/0.378 | 6.438/7.054 |

The 27-tap splash met the 16 ms frame budget. Its p95 penalty versus 8 taps
had a paired 95% interval of [2.212, 2.416] ms. Its six compute pipelines
and one render pipeline used 524,288 B of transfer scratch. The P2G
median represents 20.7 M particles/s and 558 M node contributions/s.
CPU encode, submit and completion-wait medians were 0.445, 0.081 and
7.067 ms. The 8-tap showcase deposited 66.9 M particles/s and 535 M
node contributions/s.

For the transfer fixtures, fixed clear took 0.036/0.037 ms at the spread/clumped
medians. Native float clear took 0.029/0.029 ms. The float route had four compute
pipelines and 2,097,152 B of transfer scratch. Fixed8 had three pipelines and
524,288 B. Float P2G deposited 66.3/68.8 M particles/s and 531/551 M node
contributions/s spread/clumped, compared with 66.0/61.0 M particles/s and
528/488 M contributions/s for fixed8. The authors' 8-tap P2G medians were
about 2× the proposal's 0.46–0.48 ms direct-dispatch medians. The float-minus-fixed
paired p95 intervals for the complete deposit lists were [0.004, 0.013] ms
spread and [−0.014, 0.055] ms clumped.

## Chrome WebGPU

Chrome selected Apple's Metal-3 adapter and granted timestamps. All 1,572,864
grid words matched the wide integer CPU reference across spread, clumped, boundary and
cancellation fixtures in both orders and after capacity rejection. All eight
accepted runs had unique IDs and stable order within each cell. Each rejected
run returned Capacity and preserved the preceding grid. The quadratic splash
also ran in the interactive host at 1280 × 800, and Pause stopped it.

The sorted route allocates 1,836,032 B of scratch and creates 16 compute pipelines.
Deposit-only runs record 13 of those. Apple Metal-3 GPU measurements below are
p50/p95 in ms, over 64 paired samples per arm.

| Fixture | Clear + capacity | Sort + ranges | Gather | Grid update + G2P | Full measured dispatch list |
| --- | ---: | ---: | ---: | ---: | ---: |
| Spread deposit | 0.000/3.015 | 1.638/5.046 | 0.328/0.328 | — | 1.901/5.374 |
| Clumped deposit | 0.000/2.359 | 0.852/3.670 | 5.112/10.355 | — | 6.160/13.238 |
| Splash step | 0.066/1.901 | 1.573/1.769 | 0.721/0.852 | 0.131/0.197 | 2.556/4.981 |

Chrome timestamps quantized at 65.536 µs. Full step CPU encode, submit and
completion-wait medians were 0.10, 0.00 and 7.10 ms respectively. The sorted
deposit list processed 34.5 M particles/s and 275.9 M node contributions/s
spread, and 10.6 M particles/s and 85.1 M node contributions/s clumped.
The proposal's native scatter medians were 0.46–0.48 ms. The observed browser
sorted deposit lists took roughly 4× spread and 13× clumped as long, including
clear, radix sort and ranges. The clumped gather dominates its median because
a destination thread serially reduces long ranges.

The fixed-atomic browser deposit controls measured 1.049/3.801 ms spread and
0.590/1.376 ms clumped. The sorted-minus-fixed p95 95% intervals were
[1.049, 2.294] ms for spread, [10.551, 12.976] ms for clumped, and
[−0.131, 2.163] ms for the full splash step. Both managed build slots were
held. Triangle p95 was 0.131 ms before and 2.753 ms after, above the quiet
limit. The speed verdict is Unknown. Raw timings, pass layouts, fixture inputs
and GPU grids are under `out/meta/cap1b/`. Browser transport for this run used
the prebuilt v15_exec CLI's exported bundles and a projection of the same
pass ABI while the Rust harness was queued. Naga appends an underscore to
entry names ending in digits, so the authored entries now end in letters.

## Workspace verification

The lab-profile workspace suite ran once under the GPU lock. All 180 tests
passed, including both particle twin tests, and one test was skipped. Full
static checks verified corpus provenance, shell syntax, core vocabulary and
the Keel dependency pins. The two grammar recognizers initially agreed on
309/310 outcomes. Moving the rejected comment outside the kernel body restored
agreement on all four changed scenes in the focused rerun. Full and focused
logs are retained together.
