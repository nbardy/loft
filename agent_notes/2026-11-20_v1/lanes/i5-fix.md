# Phase 5 fix wave

## Predictions before compilation

- A fixed perspective camera should return eye [4.5, 4.5, 8], target [0, 0.7, 0], and FOV 0.62 at all four corpus times, through both default scheduling and explicit `use_camera`. Existing orbit scenes should keep their frame transforms.
- The frozen anchor should parse as Stage 0 import plus a named mesh lookup. The 20 rows should share a single asset and the CPU ray distances should be 1 m within the existing fixture tolerance.
- The standalone bundle should allocate distinct viewport visibility, internal depth and HDR, fixed-size alternate view targets, and caller presentation. Two encoded frames should reset writable indirect arguments and keep internal clears away from caller attachments.
- All seed-generated geometry should match the sampled legacy rings within the generator's existing 1e-5 m CPU constraint. Contact sheets are owner observations. The denominator and pinned references stay fixed.

## Result

**Release held.** Main `5d98414` was merged into `loft/i5-scored-fix` at `5607156`. The branch is pushed. No release merge or reference approval ran. All 18 approved RGBA hashes matched before and after, and the nine pinned R0g files, gate manifest, corpus manifest, and tolerances match the scored branch. Scratch logs are under `out/meta/i5-fix/`.

| Finding | Disposition and evidence |
| --- | --- |
| L5a import and BLAS | The original Stage 0 string and receiver-call refusal is fixed in the parser and grammar. Static GLB and external binary glTF lower to shared triangle storage and CPU Bvh2. A scratch copy of the frozen anchor with an explicit `f32(i)` cast built four modules and produced a bundle. A single CPU invocation importing and compiling a generated 100,000-triangle grid took 27.49 ms end to end. **ESCALATE** the unchanged anchor fixture failed E003 at `i * 2m` in the last permitted build. Its enabled 20-instance proof stopped there. Commit `8f1183e` fixes the CPU-stage multiplication case and still needs a new compiled proof. Imported textures, skins, animations, and multi-primitive meshes are also outstanding. |
| L5a retarget | `retarget Walk from Source to Target { scale: 1 }` passes syntax and gives E601 for the unavailable chain. **ESCALATE** glTF skin/clip lowering, the named chain mapping and planted walk proof. No drift measurement. |
| L5a/L5g pose inputs and replay | Added typed slot layout and a timed packet entry point on Runner group 0. **ESCALATE** authored slot lowering, resumable motion dispatch and live packet execution evidence. Same-device native replay produced identical 32-byte state identities for the 481-tick trace. Cross-device replay still needs a numeric oracle. Metal's driver fields remain empty. |
| L5b/L5g standalone hermit | Hermit builds and emits a four-section .loftb with distinct color/depth texture declarations, sampler binding, viewport dimensions, frame templates and writable indirect reset tables. The portable codec reports distinct container errors. **ESCALATE** the new compiler-free device executable and wasm32 checks were not run within the build cap. Runner still needs the full Keel consumer, viewport-dependent camera updates, authored sample counts, full asset payload handling and browser encoding. The bridge packs a 1280×800 time-zero camera and advances its time word. |
| L5c bead | The port now uses the exact fixed camera for whole and through. **ESCALATE** the legacy four-sample coverage, ink and nearest-miss distance still have no equivalent in this port. The scored branch failed all nine R0g frames. No fresh R0g numbers are claimed. The old and new beauty pair is included for owner review. |
| L5c missing scenes | **ESCALATE** only three of the 75 identities have candidate source. The former quarantines, legacy modules without scene exports and absent drawable ports remain unverified. |
| L5d corpus | Recovered 14 preview sources and generated all 25 seeds. Four preview grammar repairs produced identical emitted WGSL. **ESCALATE** 36 remaining identities, original views and projection, omitted effects and checks, and the direct CPU Twin proof. |
| L5e corpus | All 75 existing sources pass the standalone CPU CLI check. Their archived geometry sampling still uses seven motion keys and simplified materials and water. Generator can publish complete new batches by rename. **OWNER APPROVAL NEEDED** R0g references are absent for this cohort. |
| L5f corpus | All 70 pinned seeds were generated with the fixed eye, target, and FOV. The default schedule resolves a camera through `use_camera`, also accepted in an explicit schedule. All 70 pass CPU CLI check. **ESCALATE** the direct compiled CPU Twin proof and exact legacy look. |
| Hardware ray queries | Added native triangle BLAS/TLAS construction and a hardware WGSL query plus an imported-triangle hit/miss comparison example. **ESCALATE** execution on the M4 and dispatch integration into the World queries. No hardware timing or parity claim. |
| Core constraints | The prescribed core word scan passes. File and replay serialization moved into the compiler-free format crate. **ESCALATE** Runner remains 1,992 physical lines against 1,000 by the integration count. No Phase 5.5 code added. |
| Compiler and runner diagnostics | The runner preserves .loftb decode error kinds. **ESCALATE** compiler artifact Naga lowering still transports failures as formatted strings. |

## Corpus and approvals

All 187 available sources returned zero from `loft check` and rendered one 512² frame at time zero. They comprise 78 scored-branch candidates, 14 recovered L5d examples, 25 new L5d seeds and 70 new L5f seeds. The new generator sampled both pinned JSONL sets and obtained a maximum legacy ring reconstruction error of 5.2463e-12 m with zero swatch variation. This measures the source sampling step. The compiled CPU Twin example was built in test-harness mode only, so the per-scene Twin proof is outstanding. The CLI check does not prove source checks omitted by the ports.

**OWNER APPROVAL NEEDED.** `out/meta/i5-fix/approval/` holds 187 WebP q95 pairs and a contact sheet for each of the 17 batches containing candidates. The old side uses the existing archived 512² beauty observation at time zero. Where an archive has several views, the first sorted view was used except that bead explicitly uses whole. A beauty archive is independent of the pinned R0g geometry/depth gate. Batches 2–4 had no candidates to render. The 77 prior unpinned candidate identities are still unverified. The full corpus remains at zero certified out of 295 with 108 lacking candidates.

## Build and gates

- The first two permitted compile invocations found independent Rust integration errors. The last invocation compiled the workspace and all target sources, then nextest stopped with 73 passed and one failed at the frozen anchor. Eleven enabled tests remained after fail-fast. Examples compiled as test harnesses, which do not execute their main entry points.
- The normative grammar and llguidance accepted the anchor and all generated seeds. The compositional Earley instrument covered 170 generated files and 51,234 row statements. The four initial L5d grammar refusals were repaired and retested with both parsers.
- Locked baseline Pictures 18/18, Twin 36/36, Load 33/33 and Truth 34/34 held. The first Speed run passed 6/12 with no control above the contention threshold. Blobs, capsules and hermit emitted byte-identical shader text to the scored branch.

- **ESCALATE Speed.** A quiet repeat still held 6/12. The untouched scored binary, run on the same device, missed the same six p95 rows. Quiet fix-wave p95 ranged from 1.660 ms to 10.559 ms on the misses. The scored control ranged from 1.873 ms to 10.581 ms. No threshold or contention rule changed.
- The follow-up run of the existing Runner test binary passed 8/9. The remaining test expected `Unsupported` for an external image with no adopted binding. Commit `8f1183e` restores that named refusal while retaining internal-image binding. It shares the anchor follow-up's unbuilt status.
- Bun passed 312/315 in the full invocation. The three serve children could not find the legacy native addon in the new worktree. The focused rerun with the existing addon passed 3/3. The locked dependency install resolved TypeScript's missing `wgsl_reflect` and the typecheck passed.
- A named explicit `use_camera(Fixed)` schedule built, and a fixed seed's IR successfully round-tripped with eye [4.5, 4.5, 8], target [0, 0.7, 0], and FOV 0.62 in f32.
