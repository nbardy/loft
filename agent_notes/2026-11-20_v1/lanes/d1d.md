# D1d · Phase 5 migration and device proofs

## Base and ownership

The lead ruling takes Phase 2 and 3 from `loft/int23-fix`. Local main and origin/main were `8fd710db37d4b1d701f6b0d4cd97accf9b84e434`, and `d4b76463824104af280c127650e38f39026e44e5` was absent from main. The m23 report at `out/meta/m23/BLOCKED` records Speed 6/12. The branch started at `498c0f30acbdbb71fde5f237177f96847e3feecd`, which contains the requested score and its disposition note, then used the shared merge with origin/main at `d8ca52e489e82a0b3e39258d52c3e103fe229de6`.

The published ownership base is `b92f8c057585a476d9b1d9fec82d94f114f6de0b` (“Prepare D1 lane base”). Both scored tips, Phase 4 `23562541072a6cf1e6f949ae2d5444d54e04f7ec` and Phase 5 `e8ffd8cb634ae994e89fdf8edefa9291e3c49295`, are ancestors. Published import repairs through `7fb88f900507added7ed40ac197bff4d8625af04` precede d1d changes. Focused binaries used the additional published d1a runner revision `6bc9ab32eac289b9be9e0863e537e538feb6943a` in the lane workspace. The integrator owns that merge.

## Predictions recorded before the native build

| Probe | Prediction |
| --- | --- |
| Corpus | Exactly 295 frozen identities and 187 inherited candidate paths. At least 94 more candidates even if all 187 certify. Look approval alone adds zero. At most one geometry qualification from current pins. |
| Bead | 9/9 RGBA frames. At most 26 or 102 pixels above one code, zero off-edge and maximum cluster four. The resolved color varies with sample coverage. Depth uses the nearest covered sample and the positive plane distance. |
| Import and retarget | Static 100,000 triangles under 100 ms, 20 placements sharing one anchor asset. Planted toe ≤0.08 mm against a 0.1 mm cap. E601 for an unmapped chain and E602 for excluded extensions. |
| Hardware rays | Transformed hit at 1 m ±10 µm, miss −1, identical distance and row identity between GPU backends, zero panics. |

## Corpus · ESCALATE · 0/295 fully certified

The frozen example source is `d3674dbdb918b604a5e82f9736db84780a6894a8`. Creative seed source is `b7f79a82f380bc61c18f8349d3ecf3e91fc07ea7`. Both pinned JSONL digests and all 1,510 archived beauty file digests matched. The evidence index joins all 295 identities with source and R0 shot provenance, check records, candidate source hashes, CPU observations, timing, repacking and look dispositions. It records source import chains to drawn consumers for 51 absent identities. Those chains establish source usage candidates for investigation. They supply no standalone acceptance contracts.

| Candidate coverage | L5c | L5d | L5e | L5f | Total | Additional candidates needed for 281 |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| Inherited | 3/75 | 39/75 | 75/75 | 70/70 | 187/295 | 94 |
| After lantern source-witness batch, 3 identities and 27 frames | 3/75 | 39/75 | 75/75 | 70/70 | 187/295 | 94 |
| After the remaining planar/empty-ground witnesses, 12 identities and 80 frames | 3/75 | 39/75 | 75/75 | 70/70 | 187/295 | 94 |

The 107 new scratch observations cover 14 of the 186 unpinned candidates and the still-absent swinging lantern. They came from the frozen compiler and its Dawn executor at the archived cameras and times, including the lantern’s continuous clock. The derived alpha observes resolved opaque depth and includes the analytic floor where applicable. Pearls and Shell retain the named sea-depth blocker. Scratch observations await reference promotion. Seventeen owner batches still hold 187 beauty decisions.

Current CPU validation covered all 187 candidates and 51,234 pinned rest samples across 170 seeds. The maximum rest error was 4.92 × 10⁻⁷ m. Zero candidate authored checks ran. Six inherited candidates have R0 source verdicts. The inherited speed and repack evidence remains linked per row. The unfinished rows include 108 absent candidates, the 14 incomplete previews, between-key seed motion, and geometry/depth observation and acceptance coverage. Geometry-qualified identities are 1/295. Release requires 281/295.

Evidence under `out/meta/d1d` includes `evidence.json`, `candidate-cpu.jsonl`, `legacy-observations/`, `legacy-other.log` and `reference-integrity.json`. All 18 approved baseline RGBA digests and nine pinned bead file digests were verified. Frozen input files, the comparator, and the prior owner queue retained their hashes.

## Bead · geometry PASS · portable export ESCALATE

The authored screen and observation use the same analytic ray, closest gap, ink width, four-bit mask and floor distance. The observation writes an RGBA8 texture through `bake_tex` and reads its produced bytes. The screen restores the legacy fitted cast shadow and compensates for the ACES/sRGB output transfer. Fixed widths stand in for derivative-controlled toon-band transitions.

The first compiled observation used an invalid unsigned literal spelling and produced no images. After repairing the authored literals, both 512² time-zero frames passed. The whole-view row at y=128 transitions from ground RGBA (161,199,126,87) at x=135 to partial ink RGBA (128,153,102,65) at x=136. The through-view bore at y=256 transitions from (82,110,245,60) at x=185 to the ground’s (161,199,126,61) at x=186. Both strips match the frozen samples at those coordinates.

Final frozen comparison was **9/9**. The four through frames had zero pixels above one code. Each whole 512² frame had eight isolated silhouette pixels above one code and the 1280×800 frame had three. All frames had zero off-edge pixels and zero alpha pixels above one code. The 16-frame 512² clip was 0.147875 ms p95 with control 0.020958 ms. The bundle writer returned “portable bundles require a typed bake or path resource descriptor”. The new look is **OWNER APPROVAL NEEDED**.

Evidence includes `bead-initial-retry1/` with transects, `bead-final/score.json`, `bead-final-clip.jsonl`, `bead-repack.log` and `approval/OWNER_APPROVAL_NEEDED.md`.

## glTF and retarget · ESCALATE

Packed output holds the unchanged anchor’s single kind and asset at its 20 expected placements. Two static primitives pack into one asset, with triangle IDs 0 and 1 and distinct red and green factors. The GPU drew both triangles. Both beauty and albedo probes showed black triangles. The shared shader emitter uses an empty authored lobe list for the mesh kind when it chooses the emission-only and albedo formulas. It builds a separate populated `lobes` list for the actual mesh BSDF. The GPU material result remains failed. The CPU twin still reads the first primitive's factors and needs the selected triangle’s factors.

The 100,000-triangle debug builds completed at **331.740 ms** and **557.989 ms**. The second removed overlap with the corpus CPU process. Both missed 100 ms. The previous 23.854 ms measurement used the optimized lab profile. A comparable optimized D1d measurement remains open. The negative controls produced E602 for KHR clearcoat and E601 for an unmapped retarget. Skin hierarchy, inverse binds, textured glTF, animated TRS, and the positive two-rig planted walk remain open. No toe drift was measured.

Evidence includes `cpu/import.json`, `cpu-quiet/import.json`, `cpu/mesh-rows.json`, `cpu/primitives-gpu.json` and the refusal logs.

## Hardware rays · ESCALATE

The adapter request checks the ray-query feature and all three capacity limits. Device attachment verifies the granted capacities. BLAS input validates the actual asset and transforms, shares the BLAS across placements and passes row identity through TLAS custom data. The portable per-kind shader uses the same mesh’s Stage 0 BVH for inclusive-interval nearest and any-hit. The compiled World path still refuses mesh kinds at its path assembly boundary.

The first `build loft-cli --bins --examples` stopped on the imported duplicate visibility in the runner. Retry 1 used the published d1a runner and stopped on the observation writer borrow. Retry 2 built the CLI and CPU and corpus examples, then stopped on an inferred array parameter in `phase5_ray`. The parameter type was made explicit after the last permitted build. No fresh hardware hit/miss or backend parity result was produced. The historical zero-limit panic remains the reproduction. Logs are `native-build.log`, `native-build-retry1.log`, `native-build-retry2.log` and `out/meta/i5-r2/ray.log`.

## Integration dependencies

- **Portable screen resources.** The path assembly produces the ordered present pass, row membership, BVH and descriptor words with typed buffer access. The bundle writer and runner consume those resources from `PassDecl` and the persisted resource table. Teach the writer to emit that plan for `screen_pass`, with its actual viewport requirements. Preserve the established wire writer when existing section shapes suffice. Allocate a new compiler ABI and a reader for the previous layout if the persisted resource declaration changes.
- **Compiled World triangles.** Stage 0 owns mesh-local positions, triangle IDs, primitive factors and the mesh BVH. The path compiler must admit the Mesh draw variant and consume the same triangle selection for intersection, geometric normal and material. Pass stable asset IDs, row IDs and 3×4 placements to the device attachment consumer. Select portable BVH or native TLAS once, with typed unsupported-device failure for a required native route. Keep device acceleration structures out of persisted bytes. If portable mesh/texture payloads gain a wire representation, put it in ASSETS under a new compiler ABI rather than reinterpreting ABI 1. The Rust CPU twin consumes the winning triangle’s primitive factors from the same ranges. In `emit::kind_module`, lighting and albedo selection must use the populated mesh lobe list.
- **Retarget.** The Stage 0 importer must produce typed rig hierarchy, bind and inverse-bind data, and TRS tracks. The retarget lowerer consumes resolved joint IDs and samples into the existing BODY, pose and prior tables. A positive probe must compare the target toe’s world point to an independent point throughout the planted interval. The two placements and E601 control have distinct acceptance rows.
