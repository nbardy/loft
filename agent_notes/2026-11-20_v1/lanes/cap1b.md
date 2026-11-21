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

The v15_exec baseline `5e252da` exported all four authored scenes. The exact
consumer fixture returned E504 with the native `pass p2g` producer span.
Earley and llguidance both accepted all four scenes. The typed native scatter
reads storage row fields individually. A whole-row local currently refers to
an undeclared nominal WGSL type in the v15b emitter. This lane uses the existing
field access and pass ABI. The frozen references were untouched.
