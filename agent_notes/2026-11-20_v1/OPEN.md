# Open targets after the D1 redesign gate

**Phases 2, 3, 4 and 5 are blocked.** [Gate and raw evidence](../../out/meta/i6/gate.json) · [Contact sheet](../../out/meta/i6/sheet.webp)

The scored candidate is `10f77b51626b5be3f20b130a306ad7c78c37ff78` on `loft/int6`. It contains `loft/int23-fix` at `498c0f3`, the Phase 4 import at `d3faf43` and the Phase 5 import at `637401f`. The combined tree was built once with the optimized lab profile. One Apple M4 Metal GPU lock covered the full gate, phase probes, mutants, platform checks and suites.

| Gate | Held | Release consequence |
| --- | ---: | --- |
| Pictures | 0/18 | Each combined frame differed from its Phase 4 proposal by at most 1 pixel above 2/255. All 18 qualify for the requested exception. |
| Twin | 36/36 | Passed. |
| Speed | 12/12 | All six command counts and 300-frame p95 caps held. Controls ranged from 0.039 to 0.099 ms against 0.35 ms. |
| Load | 31/33 | Hermit at both counts created 14 pipelines. The manifest budgets 13. The indexed shell adds one load-time bake pipeline. |
| Truth | 34/34 | Passed. |

All five decision mutants and the inline-BSDF-growth mutant fired. The runner compiled for wasm32 with web-sys. Both grammar recognizers accepted the 279 tracked scenes and rejected the negative sentinel. The native suite recorded 107 passed, 39 failed and 0 ignored. Bun recorded 312 passed and 3 failed.

## Phases 2 and 3

| Lane | Evidence on the combined candidate | Remaining acceptance criteria |
| --- | --- | --- |
| D1a · `b1a4c26` | Runner tests passed 16/16, including rebind, swap, host depth and native manual promotion. A source-built hermit bundle drew two standalone frames. Fresh 481-tick replay matched state and pixels. | Runner counts 3,758 physical lines against 1,000 before the separate native executor. Native source and embed still use different rendering paths. Shared bake/path resources, cross-caller pixel/state parity and browser frames remain open. Three- and four-section persisted layouts still share ABI 1. |
| D1c · `561df77` | The CPU test confirmed the six-op sphere tape, two live registers, one dirty row and a structural tape using five registers. The manual GPU edit fixture passed at +0.378 ms. | Public literal, structural, loop and invalid edits all exited 6 while creating the editable shader. WGSL source markers reached Naga at line 140. Source-to-upload ≤2 ms, zero foreground creations, GPU overhead ≤0.5 ms, field and image parity, typed rejection and browser async promotion still need a working authored path. |
| Phase 3 motion | The timed prior clip passed at 0.553 ms. Boss body syntax checked. The scalar Crab matched 481/481 ticks. | Scheduled solver and open HeronHop returned E405. The 1,000-row Crab ran in 0.168 ms p95 against 0.150 ms, control 0.061 ms. Full BODY/STATE/POSE, Stage 0 near/clear/land_on asset binding, colored constraints, solver events, world-space pinning and support-hull coverage remain open. |

## Phase 4

D1b (`8156b11`) scored 0.487, 0.733, 0.466, 1.471, 3.922, 5.432 ms p95 for capsules, blobs and hermits at 1 and 20 instances respectively. Hermit-20’s 6,369 indexed UV vertices and 36,864 corners produced identical beauty, depth AOV and ID pixels against the procedural draw. Hermit-20’s median Vis, Shadow and Shade times were 1.399, 0.630 and 2.180 ms. The earlier paired timing experiment also met all six budgets with immediate reads. It leaves the historical tail unexplained.

| Check | Result |
| --- | --- |
| Coral Shrine | 19.240 ms p95 against 4.0 ms over 100 close frames after 30 warmups. Shade took 16.019 ms median, control 0.053 ms. The 64-pixel bore probe distinguished the carved and solid worlds. |
| Bake | 3.748 ms against 2.0 ms. All 20 load dispatches and 5/0/0/5/0 frame dispatches held. Vector dirty clocks, every-mip texel comparison and portable bake descriptors remain open. |
| Spectral | The four-sample 128² study rendered on the eight-slot device. |
| Transport and viewer | Dielectric transmission, dipole/random-walk SSS, Gaussian media, prior-frame reservoir reuse, reprojection and scheduled bloom mips remain open. |

## Phase 5

D1d (`93e0888`) supplied an identity index for 295 frozen entries. All 187 candidate source hashes matched and all 187 clips met their 50 ms cap. The bead passed 9/9 frozen RGBA frames with at most eight isolated pixels above one code, zero off-edge pixels and zero depth failures. Its portable bundle export still refuses the screen-path resource descriptor.

| Check | Result |
| --- | --- |
| Corpus certification | 0/295 certified, 281 required. There are 108 missing candidates and 186 candidates without pinned geometry. The 107 scratch observations cover 15 identities. Source checks, 14 reduced previews and between-key seed motion still need evidence. |
| glTF and retarget | The optimized 100,000-triangle import took 22.042 ms against 100 ms. The two-primitive pack succeeded. D1d’s primitive-color probe failed. Textures, skinning and animated clips remain open. Retarget returned E601 and the positive planted-toe drift (≤0.1 mm) is unmeasured. |
| Hardware rays | Portable per-row hit/miss returned 1 m and −1. The native probe exited 1 with one acceleration structure bound against a per-stage device limit of zero. Compiled World paths still refuse mesh geometry. |
| Embedding and replay | Standalone host submitted two frames and both depth conventions passed. Pinned timed replay exited 5 on bundle identity. Live slots and post-start input, cross-device replay, Keel/browser frames, dynamic host viewport and portable ASSETS remain open. |

## Workspace regressions

36 native tests tried to read `lib/migration/` as a file. The remaining failures were the 1,000-row Crab budget, the Shrine test prelude missing `loft_user_paper_grain` (E701) and the custom Radiance medium refusal (E405).

The full Bun run lacked the legacy native add-on and draw executable in the disposable checkout. After restoring the binaries from the identical legacy source tree, the three affected server tests passed in a focused check. [Full run](../../out/meta/i6/bun.log) · [Focused check](../../out/meta/i6/legacy-recheck.log)

## Owner review

The appearance queue covers 18 layered frames, Coral Shrine’s woodblock treatment and revised exact shadows, and 187 corpus beauty pairs in 17 batches, including the bead’s revised proposal. All frozen reference hashes held before and after measurement. The texture bundle serial contract needs a separate format decision. [Owner queue](../../out/meta/i6/approval_needed.md) · [Phase 4 format request](../../out/meta/i4-r2/approval_needed.md)
