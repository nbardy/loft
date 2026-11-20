# Std1a · Nested authored modules

**Held for exact stock image parity.** Base is `fa09876d33392faadf2bce084f06111da4c0ec1d`. The proposed implementation on `loft/std1a` is `3c9cf678348ebded0c7e14e36142d59424c4b347`. The lead owns integration and picture approval. B–F depend on the lead's integration.

The baseline gate and bead captures completed before relocation. Later controls used the retained base source. The prediction called for one source parse per reachable file, identical row words and commands, 187 corpus builds, and possible Shrine replay variance. Evidence is under the primary checkout's `out/meta/std1a/`. [Prediction](../../../out/meta/std1a/prediction.md) · [Sorted base inputs and SHA-256](../../../out/meta/std1a/base/source-sha256.tsv) · [Path audit](../../../out/meta/std1a/base/path-inventory.txt) · [Candidate changed paths](../../../out/meta/std1a/candidate/changed-paths.tsv)

## Import and ABI inventory for B–F

The base parsed 16 sorted top-level library inputs totaling 607 lines through the symlinks. In order they were Act, analytic, BSDF, core, frames, layout, light, mesh, motion, noise, PointLamp, post, ReSTIR, Roam, SDF and world. The two migration files entered through the bead observer. The base also parsed an embedded recipe for partial and absent schedules. Each canonical source moved once, with the light and woodblock dependencies separated. Both symlinks were removed. [Caller inventory](../../../out/meta/std1a/base/input-inventory.txt)

| Canonical modules | ABI |
| --- | --- |
| `core` | `TAU`, `Sky`, `Sun` in original row order. |
| `sdf/solid`, `geometry/analytic` | Sphere, capsule, box, smooth operations and existing ray/chain helpers with unchanged spaces and distances. |
| `frames/transport`, `geometry/layout`, `geometry/mesh` | Frame seeding and transport, layout's stage-0 helpers, and `MeshAsset` plus `Mesh(pose, asset, lod = 0)`. Mesh binds its default GGX by identity. |
| `noise/value`, `materials/bsdf` | Original value noise and seven staged lobes. GGX energy uses the existing intrinsic and table. |
| `lighting/world`, `lighting/emitters` | `World`, `Ray`, `Hit`, `RngKey`, `Scatter`, `Lit`, `Sigma`, `Vol`, `FieldMedium`, `PointLamp`, and `DEFAULT_SUN_COLOR`. |
| `sky/environment`, `integrators/direct` | `DEFAULT_ZENITH`, `DEFAULT_HORIZON`, `sky_dome`, `sky_patch`, `sky_color`, and the same ordered 12-direction `sun_sky` integration. |
| `integrators/restir`, `post/display`, `styles/woodblock` | Existing reservoir add/reuse/scale, paper and AGX functions, and the six-argument Woodblock pigment. These callbacks bind in the declaring or consuming recipe's scope. |
| `motion/solve`, `engines/act`, `engines/roam` | Original motion functions and their signatures. Act and Roam each expose four policies and import `quintic_plan`. |
| `engines/{realtime,woodblock,reference,realtime_matched,spectral_caustic}` | One public schedule per recipe. Spectral exposes `spectral_sample` and `spectral_dielectric`. The callback scope binds its photon kernel and wavelength helpers. |
| `engines/{capsules,blobs,hermit,coral_shrine}` | Runnable entries with the original rows, cameras, overrides and schedules. |
| `migration/{bead_observation,trace_coverage}` | The harness supplies the original fragment and ink to `observe_bead(fragment, ink)`. The observer depends on the coverage encoders. |

The provider returns canonical paths and bytes. The graph records each file once with ordered import edges, binding forms and use spans. Source and expression IDs address the current load. Link order preserves Sky, Sun, Mesh, PointLamp and FieldMedium ahead of scene kinds whenever those declarations are reachable. Only the entry's anonymous build executes. A complete schedule takes precedence. Partial schedules retain camera and two-world behavior. An absent selection depends on Realtime. [Import guide](../../../stdlib/README.md) · [Tagged legacy index](../../../stdlib/REFERENCE.md)

For stdaot1, content keys can use the bytes and ordered import forms and dependency keys, including core and the selected recipe. Cached content must be rebound to the load's canonical declaration identities and spans. Independently copied modules retain their own self-relative imports. Compiled bundles carry the linked program. [Cache contract](../design/std1.md#5-aot-caching)

D owns the typed Surface and Lighting inputs, transport and style callbacks, optional surface preparation and compiled visibility, path, forward and post schedule fragments. The design's two structs and callback signatures describe the D contract. [Shared inputs and callbacks](../design/std1.md#module-interfaces)

## Controls

All 11 relocated engines, six live scenes and 187 corpus ports built. The old standalone Act and Roam collided with their symlinks, and five standalone render recipes required drawable kinds. The loader probes exercised a five-file diamond, independent private shade functions, public-name collisions with both spans, missing/private imports and a full cycle. The diamond's linked value was 13. The editor probes covered an imported trace helper and entry provenance on a parse failure. The final CLI also built a qualified kind and a qualified derivative. An imported anonymous build contributed zero rows, and an unknown engine failed at its use span.

The seven gate configurations and all 187 corpus ports matched packed row data and order, camera, parameters, pass order and other non-shader program metadata. Reflected shader binding identities and order matched across 39 gate modules and 1,891 corpus modules. The base opened 17 source files per workload, plus its embedded recipe when selected. Capsules opens 9, Blobs 8, Hermit 12 and Shrine 10. The ports open 5–11 sources with a median of 9. Median corpus build wall time including source loading was 182.951 → 185.256 ms during the GPU inventory. The loader portion was 1.393 → 30.558 ms because parsing now occurs during source loading. [Exact program and source ledgers](../../../out/meta/std1a/comparison.json) · [Reflected bindings](../../../out/meta/std1a/bindings-comparison.json)

At the authored cameras, 920/935 corpus frame pairs matched decoded RGBA. The probe used all four 512² times and the 1280×800 detail time. Fifteen frames across ten ports varied. The first recorded differing pair was `crab-actions-v1__r38retry-before__before-3__3` at 0.5 s and 512². Its hashes were `25fd2745e565ac90bcd8e0c49189a5c4d52b201b0d8388358f24ec42d152d61f` and `c3743d3b562707b0173b2b27766755214d55741d0cc05d92d27e0daecb1ea555`. Three base replays and one candidate replay of that individual frame matched. All 50 captures of the ten affected workloads matched with fixed row-order culling as a diagnostic. The stock result stays open. [Corpus hash pairs](../../../out/meta/std1a/comparison.json) · [Diagnostic replays](../../../out/meta/std1a/diagnostic-comparison.json)

The frozen Pictures rule held 19/19. All 18 Capsules, Blobs and Hermit frames matched decoded RGBA. Shrine differed in the official hash pair. Three base captures had distinct hashes and 47–57 differing pixels per pair. The earliest sampled difference was at (646, 0), from [204, 159, 159, 255] to [219, 183, 182, 255]. The ID and normal probes first diverged in visibility at (646, 1). One ID decodes to CoralBranch row 115. Depth and albedo AOVs stayed stable. Multi-kind culling appends visible instances atomically in arrival order. The diagnostic used fixed row order for both revisions and all six Shrine captures matched `9190b5bd708c9637dfb507153b7b8f9a4a67cd0235420ec5798059ca3f75125d`. Official base and candidate hashes were `2711fce9e32768004185f816f8be4b418e77d3065f5506a24c091776b0cdefa2` and `e4223d3bd36b1b75d2fe61af5142034910387820e7bee9d0ccab58cac1e4bb00`. [Base lossless replays](../../../out/meta/std1a/base/raw-rgba.json) · [First differing attachment](../../../out/meta/std1a/base/shrine-aov-diff.json)

All nine bead captures matched decoded RGBA. Spectral Study matched `655772ddcc0a0ae0d6b41c3b787a382f3624d13ce7afa7c22fc1d751dafdfb31`. The l5c CPU proofs passed. The l5e CPU probe measured 22,867 strip samples with 4.7683716e-7 m maximum error. Crab, Heron and Boss recorded identical native state hashes at 480 ticks and replayed their logs. Workspace motion tests exercised Act and Roam and tick grouping. The independent bundle client re-encoded Capsules byte for byte. The host-owned device probe passed. The 100,000-triangle glTF fixture imported successfully. The first timed candidate build was 112.561 ms against 100 ms. Three paired builds measured median 133.845 → 193.595 ms, with six successful builds. [RGBA comparisons](../../../out/meta/std1a/diagnostic-comparison.json) · [Bundle and motion evidence](../../../out/meta/std1a/candidate/) · [Paired asset timing](../../../out/meta/std1a/asset-timing-pairs.json)

The copied Woodblock engine selected its style beside the scene. The unedited copy matched stock. The ink edit changed 101,176 pixels. Stock and edited fork each matched their repeat. Stock SHA-256 was `0828563144930235a121144bdb10b16a8c8007b6c200cfc1c8f4d557c85f938f`. The edited fork SHA-256 was `e4f2fb2065b1c5bad5a32b60109720713820b880254a433c086188f443f13416`. Show evidence attributes the edited code to its copied file. [Fork inputs and lossless captures](../../../out/meta/std1a/candidate/fork/)

## Timing and checks

The Apple M4 Metal run used 1280×800, 30 warmups and 300 timed frames per configuration. Base held Pictures 19/19, Twin 36/36, Speed 6/12, Load 24/33 and Truth 34/34. Candidate held 19/19, 36/36, 6/12, 23/33 and 34/34. Load returned exit 4. All six candidate triangle controls exceeded 0.350 ms. Command, module and pipeline counts matched.

| Workload | Base → candidate p95 ms | Triangle p50 ms | Commands / modules / pipelines | Cold s | Warm s |
| --- | ---: | ---: | ---: | ---: | ---: |
| Capsules 1 | 4.500 → 6.925 | 0.887 → 1.182 | 5 / 4 / 5 | 0.701 → 1.600 | 0.009 → 0.021 |
| Capsules 20 | 4.412 → 10.136 | 0.551 → 0.709 | 5 / 4 / 5 | 0.871 → 2.010 | 0.009 → 0.025 |
| Blobs 1 | 3.763 → 7.247 | 0.036 → 0.992 | 5 / 4 / 5 | 0.451 → 1.588 | 0.009 → 0.017 |
| Blobs 20 | 6.139 → 8.772 | 0.171 → 1.008 | 5 / 4 / 5 | 0.711 → 2.149 | 0.008 → 0.012 |
| Hermit 1 | 9.840 → 12.019 | 0.590 → 0.602 | 13 / 8 / 14 | 2.480 → 4.359 | 0.069 → 0.084 |
| Hermit 20 | 12.692 → 15.264 | 0.117 → 0.599 | 13 / 8 / 14 | 2.637 → 4.732 | 0.081 → 0.081 |

[Base gate](../../../out/meta/std1a/base/gate.json) · [Locked candidate gate](../../../out/meta/std1a/candidate/gate-2.json)

The second native workspace sweep held 157/159 across 27 executables. A focused rerun passed all five Phase 4 tests after correcting entry provenance. The Tier 0 promotion timing test exceeded 0.500 ms on both revisions. The independent base replay measured 0.553749 ms. Candidate runs measured 0.506083 and 0.518334 ms, with zero pixels over 1/255 in all runs. Workspace doctests and the wasm32 web-sys check passed. The scored native binaries preceded the final diagnostic and fixture fixes. The before/after checks verified 19 frozen pictures, nine bead references and 1,510 archived Phase 5 beauty files. [Native sweep](../../../out/meta/std1a/candidate/native-2.log) · [Phase 4 rerun](../../../out/meta/std1a/candidate/phase4.log) · [Base promotion](../../../out/meta/std1a/base/runner-promotion.log) · [Frozen references](../../../out/meta/std1a/candidate/frozen-references.json)

Static shell, dependency-pin and corpus-provenance checks passed. Earley and llguidance agreed on the full 283-input inventory across six shards and the focused recheck of the sole failing fixture after moving its leading comment. The sharded run replaced the redundant serial recognizer pass. The native parser and all seven focused linker and Phase 4 tests passed on the final source. [Grammar shards](../../../out/meta/std1a/candidate/grammar-shards.log) · [Fixture recheck](../../../out/meta/std1a/candidate/grammar-fixtures-verified.log) · [Focused linker checks](../../../out/meta/std1a/candidate/link-tests-verified.log)
