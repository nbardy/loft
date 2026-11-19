# Open targets after the D1 integration gate

**Phases 2–5 are blocked.** All four D1 lanes stopped at the same prerequisite. The scored Phase 2–3 commit `d4b7646` had not reached `main` at `3124c86`. Local and remote branch inventories contained zero D1 implementation tips. Each lane reported its escalation before creating a workspace, so none supplied a lane note or an implementation to audit. [D1 design](design/d1.md) · [m23 decision](../../out/meta/m23/BLOCKED)

| Lane | Deliverable awaiting implementation |
| --- | --- |
| d1a | The shared Phase 4 then Phase 5 import, followed by one executor for the CLI and embedder. The design predicts 940 L0 lines. |
| d1b | Frame-time work for shells, Coral Shrine and the Phase 5 timestamp ring, plus bake and spectral binding repairs. |
| d1c | Incremental checked-graph editing, the frozen opcode path, foreground timing and latest-generation promotion. |
| d1d | Per-identity corpus evidence, bead parity, skinned retargeting and compiled World hardware traversal. |

The single i6 five-gate run scored `cd4b140` on Apple M4 Metal. That candidate combines `main` and `loft/merge23`. Its engine matches the m23 candidate. The same lock covered independent probes of the submitted Phase 4 and 5 binaries against their own sources. Both branches add only their review after the measured engine revision. A combined four-phase execution still requires the d1a import. [Gate and provenance](../../out/meta/i6/gate.json)

## Phases 2 and 3

Pictures held 18/18, Twin 35/36, Speed 11/12, Load 31/33 and Truth 34/34. All six scenes ran 300 timed frames. Every triangle control was below 0.35 ms. [Five-gate measurements](../../out/meta/i6/baseline.json)

| Owner | Open target | Measurement or missing proof |
| --- | --- | --- |
| 2–3 | Twin visibility error ≤1e−5 | Hermit-20 CrabLeg reached 1.4961e−5 over 640 samples. |
| 2–3 | Hermit-1 GPU p95 ≤5.0 ms | 7.429 ms, with a 0.102 ms control. All six command counts held. |
| 2–3 | Cold load ≤0.6 s for 20 capsules and ≤0.5 s for 20 blobs | 0.680 s and 0.958 s. |
| 2 / d1a | L0 ≤1,000 lines, one executor across callers | 2,779 local physical lines plus 11 included lines. Fused tick, rebind and state-preserving swap across both callers still need the shared executor. |
| 2 / d1c | Source receipt through upload ≤2 ms, zero foreground pipeline creates, GPU overhead ≤0.5 ms | Authored radius edit took 9.148 ms p95 on Tier 0 and 1.593 ms on Tier 1, a 7.555 ms difference. The 0.056 ms upload timer omits parse, check and lowering. The CLI prints a literal zero for creations. The manual fixture measured +0.331 ms. |
| 2 / d1c | Frozen 56-op lowering, at most 32 live registers, native and browser promotion after 400 ms quiet | The authored shader handles 28 opcodes. Structural edits, generation ordering, browser completion and full E101–E503 diagnostic snapshots lack acceptance evidence. |
| 3 | Full authored BODY/STATE/POSE, scheduled colored solver and Stage 0 plan binding | Scheduled solver and open HeronHop return E405. The published Boss expression returns E003. Near, clear and land_on asset binding, solver events, surface pinning, general support hulls and secondary constraints remain open. |
| 3 | Full motion execution for Crab, Heron and Boss | Scalar Crab matched 481/481 ticks and ran 1,000 rows in 0.00925 ms p95. Scalar Heron and Boss specify their target heights directly. The previously measured Boss state row is 76 bytes against 2,928. Full-model pose and drift remain unmeasured. |
| 3 | Timed motion clip through the shared executor | The closed-prior clip at 0.5 s exited 6 because GPU timestamps were unavailable. |
| 2–3 / 5 | Normative grammar accepts every tracked scene and rejects the negative sentinel | 63/64 expected outcomes. Both parsers rejected the frozen glTF anchor at byte 32. |

## Phase 4

The phase-specific probes used the `6b59b38` binaries and source. The previous full baseline on that engine scored Speed 11/12. [i6 observations](../../out/meta/i6/phase4/) · [Phase 4 re-score](../../out/meta/i4-r2/gate.json)

| Owner | Open target | Measurement or missing proof |
| --- | --- | --- |
| 4 / d1b | Twenty hermits ≤5.6 ms GPU p95 | 5.989 ms in the Phase 4 baseline. The indexed shell and invariant-hoisting changes have no submission. |
| 4 / d1b | Coral Shrine close ≤4.0 ms GPU p95 and per-hit visibility against the solid lantern | 4.504 ms over 100 frames after 30 warmups, control 0.052 ms. The woodblock renderer uses a raster shadow map. |
| 4 / d1a, d1b | Bake ≤2.0 ms with vector dirty clocks and identical mip levels | Native bake measured 3.729 ms. All 20 load dispatches and the 5/0/0/5/0 dirty sequence held. Shared bundles still need texture extents, formats and bake clocks. |
| 4 / d1b | Four-sample 128² spectral study within eight storage buffers | Device validation counted nine and exited 101. |
| 4 | Material, transport and post acceptance | Dielectric transmission, dipole/random-walk SSS, Gaussian media, prior-frame reservoir reuse, viewer reprojection and scheduled bloom mips remain open. The existing medium sampler covers homogeneous boxes. |
| 4 | Green native workspace on the Phase 4 branch | Its re-score failed the Shrine prelude test (E701) and the custom Radiance medium test (E405). |

## Phase 5

The phase-specific probes used the `c9a3350` binaries and source. [i6 observations](../../out/meta/i6/phase5/) · [Phase 5 re-score](../../out/meta/i5-r2/gate.json)

| Owner | Open target | Measurement or missing proof |
| --- | --- | --- |
| 5 / d1b | All six baseline p95s within budget | Prior full baseline scored Speed 6/12. Capsules at 1/20 measured 2.141/3.055 ms against 0.8/1.1, blobs 2.309/3.773 against 1.1/2.4, hermits 8.052/10.392 against 5.0/5.6. |
| 5 / d1d | Certify ≥281 of 295 frozen identities | 0/295. Of 187 candidates, 186 lack pinned geometry/depth and the bead fails. Another 108 identities lack candidates. Fourteen previews omit original behavior. Seed motion, view coverage and source checks remain incomplete. All 187 candidate clips met the speed limit in i6. |
| 5 / d1d | Bead 9/9 frozen frames with correct coverage and depth | 0/9. Whole/through at 512² have 5,344/4,720 pixels over one code against 26 allowed. Whole at 1280 × 800 has 8,080 against 102. |
| 5 / d1d | Import skinned, textured, multi-primitive animated meshes and retarget a planted walk with ≤0.1 mm drift | Static 100k-triangle import took 43.1 ms against 100 ms. Retarget returned E601. A positive two-rig drift measurement is missing. |
| 5 / d1d | Compiled World hardware and portable ray hit at 1 m ±10 µm and miss at −1 | Hardware probe exited 101. The device granted zero BLAS geometries. Compiled World traversal still lacks the hardware route. |
| 5 | Live slots, post-start motion input and replay across devices | Compiler does not populate slots. Prior native post-start input exited 6. The pinned timed replay exited 5 again on bundle identity. |
| 5 / d1a | Keel and browser frames with host-owned attachments and portable ASSETS payloads | Standalone host drew two frames at 1280 × 800. Keel/browser frames, dynamic viewport and camera, authored multisampling and portable asset loading remain unverified. |
| 5 / d1a | Versioned persisted bundle and L0 ≤1,000 lines | Three- and four-section bundles both declare ABI 1. The old reader rejects the new file. Phase 5 runner measured 1,993 lines. |

## Owner decisions

The visual queue contains the 18 Phase 4 layered frames, the Coral Shrine woodblock comparison and 187 Phase 5 beauty pairs across 17 batches. Technical certification still requires independent geometry, depth and source behavior. The shared texture bundle format has a separate pending serial-contract decision. [Picture pairs](../../out/meta/i6/approval_needed.md) · [Contact sheet](../../out/meta/i6/sheet.webp)
