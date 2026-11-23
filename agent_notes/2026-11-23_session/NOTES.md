# 23 Nov 2026 session: performance, edges, N-feet gait, meta-engine limits

This note was written by the coordinating agent. The lane notes hold the measurements. Read them in
`agent_notes/2026-11-23_perf/`: perf-instrument, perf-load, perf-frame, perf-precision,
perf-integrate, perf-edges, motion-nfeet-2 and merge. The design notes are
`agent_notes/2026-11-23_image_pass/DESIGN.md` and
`agent_notes/2026-11-23_motion_theory/{THEORY,PROOFS}.md`.

## What landed (branch `merge-2026-11-23`, fast-forwarded to main when its gates hold; see merge.md)

| Branch | Change |
|---|---|
| perf-instrument | The Speed gate brackets each block with a ~1 ms ALU control. Contention now scores Unknown, never Miss. |
| perf-load | Surfaces are index-only, with no bake pipeline. One derivation pool. `loft load` reports per-stage and per-pipeline timings. Hermit drops from 14 to 13 pipelines and cold load drops about 0.16 s. |
| perf-frame | Present reads HDR only where something was shaded. The samples:4 path uses an alpha test. BVH traversal visits the nearer child first. Present is 14–28% faster on sky scenes. |
| perf-precision | Precision is decided by role (Radiance, Integer, Depth). The default changes no pixels. `--precision f32` is the opt-out. |
| perf-edges | `samples:` becomes `edges: none \| coverage \| taps`. The default stays `none`. |
| motion-nfeet(-2) | The Walker gait supports up to 8 sites. Compile-time kind parameters (`kind Walker[sites: u in 1..9]`) now lower to constants, so 4-site walkers cost what they did on main and render byte-identical. |

Gates on perf-integrate: Pictures 19/19, Twin 36/36, Speed 9/12 (3 contended, 0 missed),
Load 26/33, Truth 34/34. Main scored 6/12 and 23/33.

## Corrected beliefs

- **The Spore-style adaptive gait and the springs were never dropped.** They live in
  `stdlib/motion/{gait,body,springs,solve,mood}.loft`. They were hard-wired to 4 feet, and that
  is now lifted.
- **"Crab Live is slower" was contention.** Main's binary renders it in 1.5–2.3 ms. Earlier Speed
  "misses" were mostly a blind control.
- **"Heavy lighting is 2× slower than legacy" no longer holds.** At 20 groups the two engines are
  level. At 1 group the cost of the edge refinement is the whole remaining gap.
- **Legacy's edge AA cost nothing.** It turned the closest approach of the single ray into
  coverage, written to the hardware MSAA sample mask. Native `taps` traces 4 new BVH rays per edge
  pixel, which is 48–69% of Shade. `coverage` is native's equivalent of legacy's method.

## Owner decisions still open

1. **Make `edges: coverage` the default?** The look is clearly better and costs about 0.6× of
   `taps`, but all 19 frozen pictures would have to be re-pinned. Paper presents would need a
   fallback. The switch is the `None =>` arm in `crates/loft-passes/src/shade_schedule.rs`.
2. **A 768² shadow map for Crab Live** would cut Shadow by about a third, with slightly softer
   edges.
3. **A cheaper sky quadrature.** It is 58–59% of Shade on Hermit and Crab Live, and changing it
   changes the look.
4. **The image-pass design** has 8 open questions, listed in DESIGN.md. The coordinator's
   recommendations are in the session transcript: reuse `screen_pass`; zero-initialised history;
   tonemap moves to stdlib first; à-trous only in the preview and reference engines.

## Known limits (meta-engine)

- **Pass verbs are a closed Rust list** (`pipeline/schedule.rs`). Authors write per-pixel,
  single-frame functions only. TAA, ReSTIR, denoisers, SSAO, bloom and DoF are therefore not
  expressible in `.loft`. The image-pass design is the fix. It shares Lane A with K1.
- **Laws cannot read another kind's table** (E405, `loft-passes/src/motion.rs:69`). The adaptive
  gait cannot drive the StageCrab body, and two bodies cannot interact. That blocks fights.
- **The trace refuses point-dependent branches** (E701). Exact per-leg culling is impossible, and
  that limit caps 8-leg Crab Live at about 11 ms. The legs are 85% of the trace.
- **Kind parameters are one value per kind per scene** (E402 otherwise). A host `upsert_rows`
  that changes the value is not refused.
- **The gait swings one foot at a time.** Ties go to the lower ID, so eight legs sweep 0 → 7.
  The multi-swing design is in THEORY.md §3 and in motion-nfeet-2.md.
- **`loft controls` still uses the old triangle control.** The precision opt-out is not wired
  into `gate`, `controls` or web. coral_shrine is nondeterministic on main (38–149 px).
  blobs-20 and capsules-20 still miss their cold-load caps. The Rust core is 34k lines against
  an 18k budget.

## Next work, proposed order

1. **K1 plus image Lane A:** cross-table reads for laws and checks, sharing one resolver with
   image passes. This unblocks StageCrab on the adaptive gait, Link, and duels.
2. **Body test set:** biped, three-legged, hexapod, heron, and a quadruped with a flexing spine.
   Each probes one gait assumption, and each needs numeric checks (planted, COP inside the hull,
   no fall, cost).
3. **Stride:** several simultaneous swings, wave order alternating sides, and a lateral pole for
   crabs. Then the rest of THEORY: centroid, effort, strike, link, duel.
4. **Image passes:** TAA first, written as stdlib.
5. **Trace culling:** accept a branch when its skipped side provably cannot lower a `min`, or
   give each leg its own bound. Crab Live needs this to reach 5 ms.
