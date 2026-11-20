# Open targets for v1.0-alpha

## Legacy retirement

One locked Apple M4 Metal run on `33cdcbf` scored Pictures **0/18**, Twin **36/36**,
Speed **12/12**, Load **31/33** and Truth **34/34**, with zero contended verdicts.
All 18 gate RGBA hashes and all nine bead renders matched archived main bit for bit. The two
Load misses were Hermit's 14 pipelines against a budget of 13. All 1,510 historical beauty
hashes verified. The reference approval queue is unchanged. [Gate measurements](../../out/meta/del2/five-gates.json) · [Exact frame comparisons](../../out/meta/del2/pixels.json)

The single native build produced 25 test executables. **152/153** tests passed, including all
six mutants and the three transferred image-comparison tests. The Tier 0 promotion test measured
**0.581 ms** p95 against **0.500 ms**, with zero of 140,000 pixels over 1/255. Isolated checks
measured **0.667 ms** and **0.796 ms**. The same test executable (identical SHA-256) measured
**0.787 ms** on concurrent unchanged main. This timing miss remains open. The wasm32 runner
checked with web-sys, doctests passed, and Earley and llguidance each returned all 280 expected
outcomes. [Native and main evidence](../../out/meta/del2/gate.json)

The root Cargo workspace owns the native and WebAssembly builds. The 18 approved images in
`tests/fixtures/gate-refs/` carry the same RGBA hashes pinned in `engines/gates.json`.
`loft gate` records each rendered RGBA hash in `out/meta/gate.json` for exact comparisons with
a previously scored revision. Picture approval remains a separate owner decision.

| Retired check or tool | Disposition |
| --- | --- |
| Bun suite, typecheck and loader-import scan | Their compiler, renderer and loader were deleted. The Phase 5 manifest partition check survives in `tools/check_corpus.py`. Native Cargo tests cover the current compiler and runner, and the dependency check keeps the runner independent of compiler passes. |
| Nested Cargo workspace checks, lab binaries and single-test-binary layout | The Bun serve tests launched the retired `loft-node` and `loft-draw` binaries and exercised their child-process protocol. Those binaries and the old bundle loader were deleted. `tools/dev.sh` builds the root members. Per-crate lint is available through `tools/dev.sh lint`. |
| R0 regeneration, live cross-engine replay and TypeScript seed sampling | They require executing the retired engine. The original inputs, hashes and observations remain historical evidence. New comparisons use the checked-in corpus and frozen images. |
| Standalone bead comparator | The same pixel, edge and cluster rules and their three tests now run in the root CLI as `loft diff`. The bead gate renders all nine observations with the workspace’s `phase5_corpus` example. |

The [Phase 5 corpus](../../corpus/README.md) feeds the grammar and geometry checks.
[Examples](../../examples/README.md) remain read-only migration sources. The browser viewer depended on
the retired engine. The unused training directory had no tracked files. The [historical design](../../docs/legacy/DESIGN.md)
and manuals are archived.

## v1–v3 integration on 20 November

All three lane heads stay on their branches. The single locked Apple M4 Metal run scored the retained engine at `843adfe`. Its executable matched the SHA-256 of the archived main build at `9878c6e`. Twin held 36/36, Truth 34/34 and Speed 12/12, with zero contended verdicts. All six mutants fired, the wasm32 runner checked with web-sys, and Earley and llguidance each returned 280/280 expected results. Native passed 150/150 and Bun passed 315/315. [Gate report](../../out/meta/iv/gate.json) · [Contact sheet](../../out/meta/iv/sheet.webp)

All 18 fresh 1280×800 frames matched the archived main RGBA byte for byte. Pictures held 0/18 against the frozen references. All 18 misses belong to the existing owner queue, with at most eight pixels per frame more than two codes from the queued proposal. Load held 31/33. Hermit created 14 pipelines against a budget of 13 at both instance counts. Before and after hashes matched for 18 frozen gate images, nine bead geometry frames and 1,510 legacy beauty images. All nine bead geometry checks passed, and their renders matched archived main RGBA. [Pixel counts](../../out/meta/iv/pixels.json) · [Owner queue](../../out/meta/i6/approval_needed.md)

| Blocked lane | Measurement and reason |
| --- | --- |
| v1 · Tier 0 edit speed · `f5bea03` | Staged fields removed most interpreter overhead. The general clipped sphere took **0.486 ms** to upload and **+0.110 ms** paired GPU p95 on the first run. Confirmation measured **0.448 ms / +2.478 ms** against the **2.000 / +0.500 ms** caps. The two-sided clip missed both at **2.280 / +3.622 ms**. Literal GPU overhead was +0.531 and +2.327 ms. The marker probe was repaired, and every recorded positive edit matched the independently compiled image. [v1 lane note](https://github.com/nbardy/loft/blob/f5bea03ee6d983a4aa103a464a4435d083578a56/agent_notes/2026-11-20_v1/lanes/v1.md) · [General confirmation](../../out/meta/v1/confirm/general.log) |
| v2 · Authored solvers · `6776d49` | Packed CPU/Metal rows differed on **371/481** Crab ticks, **177/481** Heron ticks and **447/481** Boss ticks. Verdict words differed on **481/481** for each. The three final 0.5 s frames matched main. The branch adds a Heron plan and solver event support, but playback still executes the laws. Full solver execution, bit-equal twins and the E201/E308 emitters and snapshots remain open. [v2 lane note](https://github.com/nbardy/loft/blob/6776d494ee727d3440cb9f5550cbf7915e9abb2e/agent_notes/2026-11-20_v1/lanes/v2.md) · [Tick readbacks](../../out/meta/v2/twin-final.log) |
| v3 · Shared pass preparation · `d0e6a79` | All **18/18** beauty frames changed. The owner queue allows 1,024 pixels per frame to differ from the proposal by more than two codes. Every v3 frame had at least **35,063** such pixels. Load fell from **31/33 to 28/33**. The archived Classify shader failed resource binding, and the bake and spectral hosts failed storage-texture validation. Functional L0 fell **5,447 → 5,209** against **2,500**. [v3 lane note](https://github.com/nbardy/loft/blob/d0e6a7962b21c4a572759eea06e8a559207b067e/agent_notes/2026-11-20_v1/lanes/v3.md) · [Compatibility and frame results](../../out/meta/v3/verification.json) |

### Retained Speed measurements

Each case has 300 timed frames. The prior main measurements came from the byte-identical executable.

| Scene | Prior main p95, ms | This run p95, ms | Cap, ms | GPU commands |
| --- | ---: | ---: | ---: | ---: |
| capsules-1 | 0.460 | 0.443 | 0.8 | 5 |
| capsules-20 | 0.720 | 0.749 | 1.1 | 5 |
| blobs-1 | 0.512 | 0.520 | 1.1 | 5 |
| blobs-20 | 1.489 | 1.498 | 2.4 | 5 |
| hermit-1 | 3.820 | 3.814 | 5.0 | 17 |
| hermit-20 | 5.330 | 5.271 | 5.6 | 17 |

### Source lines by audited head

The counts include tracked Rust and WGSL under each crate’s `src/`, with blanks and inline tests. Functional L0 counts executor code across crate boundaries.

| Crate | Retained main | v1 | v2 | v3 |
| --- | ---: | ---: | ---: | ---: |
| loft-abi | 498 | 498 | 498 | 498 |
| loft-cli | 5,066 | 5,182 | 5,066 | 4,307 |
| loft-format | 815 | 815 | 815 | 815 |
| loft-ir | 548 | 548 | 548 | 548 |
| loft-parse | 2,723 | 2,723 | 2,723 | 2,723 |
| loft-passes | 16,456 | 16,670 | 16,519 | 16,734 |
| loft-plan | 949 | 949 | 949 | 949 |
| loft-runner | 1,630 | 1,630 | 1,630 | 1,998 |
| loft-std | 4,321 | 4,321 | 4,321 | 4,321 |

## Wave integration on 20 November

The Apple M4 Metal gate on `9878c6e` passed the merge rule for w1–w4. Twin held 36/36, Truth 34/34 and Speed 12/12, with zero contention. All six mutants fired. The runner passed wasm32 with web-sys, and Earley and llguidance each returned 280/280 expected outcomes, including the negative sentinel. Frozen hashes matched before and after for 18 gate frames, nine bead frames and 1,510 legacy beauty images. All nine bead geometry checks passed and their RGBA matched the archived release. [Gate evidence](../../out/meta/iw/gate.json) · [Contact sheet](../../out/meta/iw/sheet.webp)

Pictures held 0/18 against the frozen references. All 18 frames belong to the existing layered-material review queue. At most 8 pixels per frame differed from its queued proposal by more than two codes. The separate comparison against freshly rendered main frames found exact RGBA on 15/18. Capsules-20 accounted for all remaining changes. Load held 31/33, with Hermit at 14 pipelines against 13 at both counts. [Pixel comparisons and hashes](../../out/meta/iw/pixels.json) · [Owner review queue](../../out/meta/i6/approval_needed.md)

| Lane | Measured delivery and work still open |
| --- | --- |
| w1 · Authored edits and motion | All four positive edit probes matched independently compiled images at every pixel and had zero foreground pipeline creations. The rejected name returned E011. The general tape and literal edit exceeded the paired GPU budget in this run. The table below gives the individual measurements. The 18 emitted diagnostics in E101–E503 have snapshots. E201 and E308 still need emitters and fixtures. The bound HeronHop crouch reached 0.59348 m in the lane probe. The three shipped characters still execute laws. Their 0.5 s images matched main, and full authored solver/plan execution and bit-equal twins for all three remain open. [w1 audit](lanes/w1.md) |
| w2 · Shrine and capsule ties | Shrine measured **3.904 ms p95** against 4.000 ms at 1280×800, after 30 warmups over 100 frames. Median Shade was 0.313 ms. The unmodified main and candidate beauty renders differed at 65 inherited variable pixels. Every candidate value occurred at that coordinate in a baseline replay. The separate full-size comparisons at identical ascending and descending cull orders were byte-identical. Depth matched exactly. Capsules-20 selected the lower row at every witnessed tie and reproduced the independently recorded beauty and ID hashes at 0, 4 and 9 s. [w2 audit](lanes/w2.md) |
| w3 · Native tests and executor | All 38 inherited failures now pass. The combined workspace ran 150 native tests across 25 executables, with zero failures or ignores. Bun passed 315/315. Native and embedded paths share raster preparation and command recording. `Loaded::prepare_passes` still selects commands from its source schedule. Full shared preparation requires portable per-draw bindings, dirty bakes and path resources. The embedded host drew two frames and passed both depth conventions. [w3 audit](lanes/w3.md) |
| w4 · Core declarations | Four archived bundles round-tripped byte-for-byte. Removing the manifest and table schema mirrors reduced the four owned crates by 17 lines, from 2,827 to 2,810. Functional L0 alone measures **5,447** on the combined tree. Including IR gives at least **5,995**, against the 1,500-line L0+L1 target. [w4 audit](lanes/w4.md) · [Functional line accounting](../../out/meta/iw/l0.json) |

| Authored edit | Source to final upload, ms · cap 2 | Paired GPU overhead p95, ms · cap 0.5 |
| --- | ---: | ---: |
| Literal | 0.480 | +0.592 |
| Structural | 0.479 | +0.252 |
| Loop | 0.481 | -0.021 |
| General | 2.133 | +4.554 |

| Speed case | Main p95, ms | Integrated p95, ms | Cap, ms | GPU commands |
| --- | ---: | ---: | ---: | ---: |
| capsules-1 | 0.526 | 0.460 | 0.8 | 5 |
| capsules-20 | 0.643 | 0.720 | 1.1 | 5 |
| blobs-1 | 0.501 | 0.512 | 1.1 | 5 |
| blobs-20 | 1.506 | 1.489 | 2.4 | 5 |
| hermit-1 | 3.921 | 3.820 | 5.0 | 17 |
| hermit-20 | 5.451 | 5.330 | 5.6 | 17 |

Physical tracked Rust and WGSL lines under each workspace crate’s `src/` include blanks and inline tests. The separate functional L0 count also charges execution in Std and CLI.

| Crate | Main | Integrated | Change |
| --- | ---: | ---: | ---: |
| loft-abi | 498 | 498 | +0 |
| loft-cli | 4,948 | 5,066 | +118 |
| loft-format | 867 | 815 | -52 |
| loft-ir | 513 | 548 | +35 |
| loft-parse | 2,723 | 2,723 | +0 |
| loft-passes | 15,185 | 16,456 | +1,271 |
| loft-plan | 949 | 949 | +0 |
| loft-runner | 1,585 | 1,630 | +45 |
| loft-std | 4,321 | 4,321 | +0 |

## Earlier alpha and runner-cut measurements

The alpha uses the lead’s release rule. The locked Apple M4 Metal run on `07ac562` held Twin 36/36, Truth 34/34 and Speed 12/12, with zero contention. All six mutants fired. The runner typechecked for wasm32 with web-sys. Both grammar recognizers gave 280/280 expected outcomes, including the negative sentinel. The 18 gate hashes, nine bead pins and 1,510 legacy beauty hashes matched. The six scenes used 5/5, 5/5 and 17/17 commands at counts 1 and 20. Their 300-frame p95 times were 0.5005/0.653 ms for capsules, 0.456/1.506 ms for blobs and 3.911/5.453 ms for hermits. [Release measurements](../../out/meta/rel/gate.json) · [Five-gate log](../../out/meta/rel/baseline.log)

### Runner and edits

| Target | Alpha measurement and work remaining |
| --- | --- |
| Runner size and shared execution | 3,758 physical lines against 1,000 before counting the separate native executor. The 16 runner tests passed and an external host drew two frames. Native source and embed still need shared bake/path resources, cross-caller pixel and state parity, and browser frames. Three- and four-section persisted layouts both report ABI 1. |
| Tier 0 authored edits | Literal, structural, bounded-loop and rejected edits all exited 6 at WGSL line 140 on the U+FDD0 source marker. The three independently compiled controls rendered. The manual GPU fixture uploaded in 0.021 ms, added 0.490 ms p95 and had 0/140,000 pixels above 1/255. The public path still needs source-to-upload ≤2 ms, zero foreground pipeline creations, overhead ≤0.5 ms, field and image parity, typed rejection and browser promotion. |

#### Runner cut

The workspace build and external host passed. All five locked gate scores and individual verdicts matched alpha. The
strict release-frame comparison held **16/18**. Capsules-20 differed at two pixels at 0 s and one at 9 s. The 4 s frame
matched in the gate. Both archived binaries reproduced both variants at 4 s and 9 s in four fresh renders per time.
The alpha binary also reproduced the exact two-pixel variant at 0 s in three of 16 replays. The alpha ID AOV
alternated between rows 63 and 64 at 4 s, and between rows 63 and 69 at 9 s. The observed row switches are consistent
with the unchanged cull shader's atomic survivor append and the strict Less depth test, which keeps the first hit
when stored depths tie. W2 subsequently established the deterministic lower-row policy and the hashes reported above.
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

### Motion

| Target | Alpha measurement and work remaining |
| --- | --- |
| Authored solvers and plans | Scheduled solver and open HeronHop both exited 1 with E405. The closed prior clip ran at 0.553 ms p95. Stage 0 asset binding for `near`, `clear` and `land_on`, colored constraints, solver events, world-space pinning and support hulls remain open. |
| Full motion model | Scalar Crab replay matched all 481 ticks. Its 1,000-row kernel measured 0.011 ms p95 against 0.150 ms, with a 0.047 ms control. Full BODY/STATE/POSE execution remains open. The earlier Boss measurement was 76 bytes of active state against a 2,928-byte state and 3,072-byte pose requirement. |

### Rendering

| Target | Alpha measurement and work remaining |
| --- | --- |
| Coral Shrine close view | Restoring the shadow map reduced p95 from 19.240 to **4.210 ms** against **4.000 ms**, at 1280×800 after 30 warmups over 100 frames. Median Shade fell from 16.019 to 0.544 ms. Control was 0.039 ms. The woodblock comparison had 73 pixels above 2/255 out of 1,024,000. The per-hit shadow proposal is excluded from this alpha. |
| Hermit load pipelines | Load held 31/33. Hermit at both counts created 14 pipelines against 13. Its indexed shell uses a load-only bake. All 6,369 UV vertices and 36,864 corners produced matching procedural and indexed images for beauty, depth and ID. |
| Bake completeness | This run completed the 20 load dispatches in 1.371 ms against 2.000 ms. The scored integration took 3.748 ms. Dirty dispatches were 5/0/0/5/0. Every-mip texel parity, vector parameter invalidation and portable bake descriptors still need verification. |
| Transport and viewer | The four-sample spectral study rendered at 128² on the eight-slot device. Dielectric transmission, dipole/random-walk SSS, Gaussian media, prior-frame reservoir reuse, reprojection and scheduled bloom mips remain open. |

### Corpus and interoperability

| Target | Alpha measurement and work remaining |
| --- | --- |
| Corpus certification | **0/295 certified**, 281 required. All 187 candidate hashes matched and all 187 clips met 50 ms, with a maximum p95 of 14.100 ms. There are 108 missing candidates and 186 candidates without pinned geometry. The 107 scratch observations cover 15 identities. Source checks, 14 reduced previews and between-key seed motion still need evidence. |
| Bead export | Bead passed 9/9 frozen geometry frames with at most eight isolated pixels above one code, zero off-edge pixels and zero depth failures. Bundle export exited 1 because the screen-path resource needs a portable descriptor. |
| glTF import time and coverage | The optimized 100,000-triangle import took **22.135 ms** against **100 ms**. D1d’s debug runs took 331.740 and 557.989 ms. Two-primitive packing succeeded, but D1d’s primitive-color probe failed. Textures, skinning and animation remain open. |
| Retargeting | The positive walk exited 1 with E601. Planted-toe drift remains unmeasured against 0.1 mm. |
| Hardware rays | The portable probe returned 1 m and −1 for hit and miss. The hardware probe exited 1 with one acceleration structure requested against a per-stage limit of zero. Compiled World traversal still refuses mesh geometry. |
| Embedding and replay | The standalone host drew two frames and passed both depth conventions. Fresh replay reproduced the recorded image. Pinned replay exited 5 on bundle identity. Live slots, post-start input, cross-device replay, Keel/browser frames, dynamic host viewport and portable ASSETS remain open. |

### Workspace tests and appearance review

Bun passed all 315 tests. Native tests recorded 108 passes, 38 failures and zero ignored. Of those failures, 36 read `lib/migration/` as a file. The Shrine test prelude lacks `loft_user_paper_grain` (E701). The custom Radiance medium fixture returns E405. [Named failures](../../out/meta/rel/gate.json) · [Native logs](../../out/meta/rel/tests/)

The full TypeScript suite passed **315/315** after provisioning the legacy `loft-node` and `loft-draw` lab binaries.
The runner-cut recheck ran all 25 native test executables and reproduced **108 passed, 38 failed, 0 ignored**, with
the same failing test names. The 36 directory-read failures comprise ten tests in `bakeoff`, 25 in `diagnostics`
and `tier0::tests::l23a_sphere_tape_matches_the_e_plus_twin`. The two other failures are
`coral_shrine_has_analytic_branches_and_two_lantern_snapshots` (E701) and
`checked_contract_reaches_lowering` (E405). W3 subsequently repaired all 38 cases.
[Named cut failures](../../out/meta/fxcut/comparison.json) · [Cut test logs](../../out/meta/fxcut/tests/)

All 18 layered frames exceeded frozen picture tolerance. Each independent still was within one pixel above 2/255 of its queued Phase 4 proposal, against an allowance of 1,024. The woodblock look and 187 corpus beauty pairs across 17 batches, including the revised bead proposal, remain queued for owner review. Texture bundle serialization still needs a format decision. [Release image comparisons](../../out/meta/rel/picture-diffs.json) · [Original appearance queue](../../out/meta/i6/approval_needed.md) · [Format request](../../out/meta/i4-r2/approval_needed.md)

## Performance carry-overs from the 18 Nov cook (lead, 20 Nov)

- **Pipeline cache plus warm-up during loading** (spec flag `PIPELINE_CACHE`). Metal compiles about 70 ms per pipeline. The old lane (agent_notes/2026-11-18_cook/pipecache.final.md) activated retained pipelines in 0.08–1.28 ms with byte-identical frames. Target: cold load of hermit-20 dominated by the cache import, and warm load unchanged.
- **Solve cache:** keep the 128 most recently used verified `solve` results (agent_notes/2026-11-18_cook/small.final.md).
- **Volume march budget** before real-time media: the old dust volume cost 20–61 ms with shadow marches (volume.final.md). Bound the step and shadow counts, and add them to the Speed budget.
- **Not carried:** f16 gave no gain on M4 (f16.final.md); f32 stays the default.
