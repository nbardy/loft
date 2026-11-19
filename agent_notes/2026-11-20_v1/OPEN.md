# Open targets for v1.0-alpha

The alpha uses the lead’s release rule. The locked Apple M4 Metal run on `07ac562` held Twin 36/36, Truth 34/34 and Speed 12/12, with zero contention. All six mutants fired. The runner typechecked for wasm32 with web-sys. Both grammar recognizers gave 280/280 expected outcomes, including the negative sentinel. The 18 gate hashes, nine bead pins and 1,510 legacy beauty hashes matched. The six scenes used 5/5, 5/5 and 17/17 commands at counts 1 and 20. Their 300-frame p95 times were 0.5005/0.653 ms for capsules, 0.456/1.506 ms for blobs and 3.911/5.453 ms for hermits. [Release measurements](../../out/meta/rel/gate.json) · [Five-gate log](../../out/meta/rel/baseline.log)

## Runner and edits

| Target | Alpha measurement and work remaining |
| --- | --- |
| Runner size and shared execution | 3,758 physical lines against 1,000 before counting the separate native executor. The 16 runner tests passed and an external host drew two frames. Native source and embed still need shared bake/path resources, cross-caller pixel and state parity, and browser frames. Three- and four-section persisted layouts both report ABI 1. |
| Tier 0 authored edits | Literal, structural, bounded-loop and rejected edits all exited 6 at WGSL line 140 on the U+FDD0 source marker. The three independently compiled controls rendered. The manual GPU fixture uploaded in 0.021 ms, added 0.490 ms p95 and had 0/140,000 pixels above 1/255. The public path still needs source-to-upload ≤2 ms, zero foreground pipeline creations, overhead ≤0.5 ms, field and image parity, typed rejection and browser promotion. |

### Runner cut

The workspace build and external host passed. All five locked gate scores and individual verdicts matched alpha. The
strict release-frame comparison held **16/18**. Capsules-20 differed at two pixels at 0 s and one at 9 s. The 4 s frame
matched in the gate. Both archived binaries reproduced both variants at 4 s and 9 s in four fresh renders per time.
The alpha binary also reproduced the exact two-pixel variant at 0 s in three of 16 replays. The alpha ID AOV
alternated between rows 63 and 64 at 4 s, and between rows 63 and 69 at 9 s. The observed row switches are consistent
with the unchanged cull shader's atomic survivor append and the strict Less depth test, which keeps the first hit
when stored depths tie. A deterministic tie policy and a reproducible reference are still open.
[Strict comparison](../../out/meta/fxcut/comparison.json) · [Replay evidence](../../out/meta/fxcut/probe/comparison.json)

`Loaded::prepare_passes` still translates source-program passes while the embedded runner prepares persisted
`PassDecl` records. Both record through `command::Pass::record`. A single preparation path must carry native
load and dirty bakes, the path/photon resources and edit-time bundle refresh. Portable lowering still refuses programs
with bakes or path resources. The native load and pass preparation span over 400 lines and the shared route requires
changes to the resource contract and edit path, beyond the 150-line cut.

D1 charges device setup, binding and attachment validation, pipeline preparation, and command recording and transfer
where they live. Count physical tracked Rust/WGSL lines including blank lines and inline tests, following runner
source inclusions and excluding its two standalone test modules. The Std subtotal includes the device-facing parts
of mixed modules, with CPU-only pick decoding and the bake dirty clock excluded. Native adds device execution
including the separate source renderer, bakes, path, edits, motion readback and hardware ray setup. The camera
matrix helpers, instance packing and CPU motion decoding are outside that count.

| L0 source | Physical lines |
| --- | ---: |
| Runner | 983 |
| Execution and preparation moved into Std | 2,429 |
| Pre-existing Std pipeline worker | 67 |
| Native CLI device execution | 1,938 |
| **Total** | **5,417** |

Runner plus displaced Std work alone is **3,412**, or **2,412 over** the 1,000-line ceiling. Including the worker
and native execution, L0 is **4,417 over**. Std grew from 1,772 to 4,321 lines and the CLI shrank from 5,246 to
4,948 lines under the crate-wide count. [Per-file source spans](../../out/meta/fxcut/l0.json)

## Motion

| Target | Alpha measurement and work remaining |
| --- | --- |
| Authored solvers and plans | Scheduled solver and open HeronHop both exited 1 with E405. The closed prior clip ran at 0.553 ms p95. Stage 0 asset binding for `near`, `clear` and `land_on`, colored constraints, solver events, world-space pinning and support hulls remain open. |
| Full motion model | Scalar Crab replay matched all 481 ticks. Its 1,000-row kernel measured 0.011 ms p95 against 0.150 ms, with a 0.047 ms control. Full BODY/STATE/POSE execution remains open. The earlier Boss measurement was 76 bytes of active state against a 2,928-byte state and 3,072-byte pose requirement. |

## Rendering

| Target | Alpha measurement and work remaining |
| --- | --- |
| Coral Shrine close view | Restoring the shadow map reduced p95 from 19.240 to **4.210 ms** against **4.000 ms**, at 1280×800 after 30 warmups over 100 frames. Median Shade fell from 16.019 to 0.544 ms. Control was 0.039 ms. The woodblock comparison had 73 pixels above 2/255 out of 1,024,000. The per-hit shadow proposal is excluded from this alpha. |
| Hermit load pipelines | Load held 31/33. Hermit at both counts created 14 pipelines against 13. Its indexed shell uses a load-only bake. All 6,369 UV vertices and 36,864 corners produced matching procedural and indexed images for beauty, depth and ID. |
| Bake completeness | This run completed the 20 load dispatches in 1.371 ms against 2.000 ms. The scored integration took 3.748 ms. Dirty dispatches were 5/0/0/5/0. Every-mip texel parity, vector parameter invalidation and portable bake descriptors still need verification. |
| Transport and viewer | The four-sample spectral study rendered at 128² on the eight-slot device. Dielectric transmission, dipole/random-walk SSS, Gaussian media, prior-frame reservoir reuse, reprojection and scheduled bloom mips remain open. |

## Corpus and interoperability

| Target | Alpha measurement and work remaining |
| --- | --- |
| Corpus certification | **0/295 certified**, 281 required. All 187 candidate hashes matched and all 187 clips met 50 ms, with a maximum p95 of 14.100 ms. There are 108 missing candidates and 186 candidates without pinned geometry. The 107 scratch observations cover 15 identities. Source checks, 14 reduced previews and between-key seed motion still need evidence. |
| Bead export | Bead passed 9/9 frozen geometry frames with at most eight isolated pixels above one code, zero off-edge pixels and zero depth failures. Bundle export exited 1 because the screen-path resource needs a portable descriptor. |
| glTF import time and coverage | The optimized 100,000-triangle import took **22.135 ms** against **100 ms**. D1d’s debug runs took 331.740 and 557.989 ms. Two-primitive packing succeeded, but D1d’s primitive-color probe failed. Textures, skinning and animation remain open. |
| Retargeting | The positive walk exited 1 with E601. Planted-toe drift remains unmeasured against 0.1 mm. |
| Hardware rays | The portable probe returned 1 m and −1 for hit and miss. The hardware probe exited 1 with one acceleration structure requested against a per-stage limit of zero. Compiled World traversal still refuses mesh geometry. |
| Embedding and replay | The standalone host drew two frames and passed both depth conventions. Fresh replay reproduced the recorded image. Pinned replay exited 5 on bundle identity. Live slots, post-start input, cross-device replay, Keel/browser frames, dynamic host viewport and portable ASSETS remain open. |

## Workspace tests and appearance review

Bun passed all 315 tests. Native tests recorded 108 passes, 38 failures and zero ignored. Of those failures, 36 read `lib/migration/` as a file. The Shrine test prelude lacks `loft_user_paper_grain` (E701). The custom Radiance medium fixture returns E405. [Named failures](../../out/meta/rel/gate.json) · [Native logs](../../out/meta/rel/tests/)

The full TypeScript suite passed **315/315** after provisioning the legacy `loft-node` and `loft-draw` lab binaries.
The runner-cut recheck ran all 25 native test executables and reproduced **108 passed, 38 failed, 0 ignored**, with
the same failing test names. The 36 directory-read failures comprise ten tests in `bakeoff`, 25 in `diagnostics`
and `tier0::tests::l23a_sphere_tape_matches_the_e_plus_twin`. The two other failures are
`coral_shrine_has_analytic_branches_and_two_lantern_snapshots` (E701) and
`checked_contract_reaches_lowering` (E405). All 38 remain open.
[Named cut failures](../../out/meta/fxcut/comparison.json) · [Cut test logs](../../out/meta/fxcut/tests/)

All 18 layered frames exceeded frozen picture tolerance. Each independent still was within one pixel above 2/255 of its queued Phase 4 proposal, against an allowance of 1,024. The woodblock look and 187 corpus beauty pairs across 17 batches, including the revised bead proposal, remain queued for owner review. Texture bundle serialization still needs a format decision. [Release image comparisons](../../out/meta/rel/picture-diffs.json) · [Original appearance queue](../../out/meta/i6/approval_needed.md) · [Format request](../../out/meta/i4-r2/approval_needed.md)
