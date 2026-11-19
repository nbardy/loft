# D1a · One bundle executor

The lead ruling starts at the scored Phase 2/3 branch and merges origin/main before importing the two later phases. Base branch tip was `498c0f30acbdbb71fde5f237177f96847e3feecd` (score `d4b76463824104af280c127650e38f39026e44e5`), with origin/main `8fd710db37d4b1d701f6b0d4cd97accf9b84e434` merged at `d8ca52e489e82a0b3e39258d52c3e103fe229de6`. Phase 4 branch parent `23562541072a6cf1e6f949ae2d5444d54e04f7ec` (score `6b59b384f9f4e6e0ce959a603347db9040b81299`) was imported at `d3faf4395c1320d4d477bb89809b9e5e8d0671f7`. Phase 5 branch parent `e8ffd8cb634ae994e89fdf8edefa9291e3c49295` (score `c9a3350d916329299931f344ee63acc62f013f8c`) was imported at `637401fdc4939dbfcae73a5c22480bba46bda3db`. Those commits contain the inherited changes and conflict resolutions. Fast-forward the lane base to `b92f8c057585a476d9b1d9fec82d94f114f6de0b` for the three entry point moves. Subsequent d1a commits own runner, format, std except promotion.rs, passes/bundle and the CLI artifact adapter.

## Predictions

- L0 allocation is 120 entry/backend, 220 resources, 200 preparation, 235 recording, and 165 transfer/timing. Target 940 and ceiling 1,000 physical lines. Historical counts are main 1,003, i23 2,779 + 11 included, i4 1,000, and i5 1,993. Charge displaced device execution and follow source inclusions recursively.
- The archived prior at t=0.5 s should return 0 from a timed 64×64 one-frame clip under 50 ms. Metal supports in-encoder timestamps, which record around the advance even if empty pass markers are elided. The closed prior should still sample 0.77375 m.
- A fused step over 481 ticks should reproduce the state exactly on the same device. Counters and group identities should stay stable on a repeated frame and change only where a handle is rebound. External host attachments should retain the occluder and untouched depth in both depth conventions. Unsupported attachment, loop or mip inputs should fail before recording.
- Source-built and decoded hermit bundles should use the same ordered executor and produce identical lossless frames on the same device. Wire encoding remains the inherited three/four-section format.

## Implementation and handoff

- `3429e11`, `96e6709` and `7fb88f9` complete inherited import reconciliation for mesh variants, the layered material sum, replay field names and timestamp indices. The path and CPU World front doors return E405 for raster-only mesh input pending d1d. The authored mesh raster path still uses its imported asset and lobe.
- `6bc9ab3` owns the D1a implementation. `loft-passes::bundle::lower(program, compiled, size, frame)` prepares resource IDs, scheduled render and fixed-step bindings and packed frame templates. The CLI artifact adapter supplies the camera and uses the frozen codec. Source callers can use `Runner::load_bundle`; file callers use `Runner::load_bundle_bytes`, which delegates to it. Both record with `Runner::encode_into`. The native `Loaded` source render facade still has its own device executor. Its takeover is pending with d1b. Native motion goes through the shared runner.
- `739f236ed4c5f66c16433eeaf1c3813ddd9b41fe` timestamps the actual first and last fused dispatch, including intervening GPU work. It adds `advance_timed_into` on the concrete runner and retains the existing backend trait. The first attempted repair used encoder boundaries. That returned exit 6 on Metal. The dispatch stamps eliminate the dependency on empty passes or pending encoder markers.
- The runner owns the classified resource handles, first-bind checks, epochs, device preparation, record dispatch, transfer and timing. Std owns host inputs, fixed-step twin and checkpoint selection, and pick/outbox decoding. Format remains compiler-free. `promotion.rs`, the backend trait, goldens, tolerances and frozen references were left intact.

## Limits for d1d and i6

- The persisted attachment descriptor covers 2D single-layer `Rg32Uint`, `Depth32Float` and `Rgba16Float` with one or four samples (one for `Rgba16Float`). The host supplies one color and at most one depth view in matching base formats and sample count. Internal targets have independent per-pass views. The host texture contract declares format, extent, view class, samples, mips and usages before binding. The supported sampler declaration is comparison. Storage-pulled and indexed draws work through the retained ABI. There is a specific refusal for vertex attributes without a layout and for texture binding arrays.
- The explicit mip command supports a declared 2D single-layer `rgba16float` mip chain with sampled and storage usages. Portable source lowering returns “portable bundles require a typed bake or path resource descriptor” for the inherited bake/path schedules. Their native path remains separate. Conditional loops return “conditional loop requires shader lowering” during load. Fixed-count loops retain ordered children. Browser compilation passed. Browser device execution is unscored.
- The wire stayed at the inherited ABI 1. Reader matrix from i5-r2 plus the current four-section load:

  | Reader | ABI 1, three sections | ABI 1, four sections |
  | --- | --- | --- |
  | Pre-resource reader | Accept | Reject `Version` |
  | i5 resource reader | Accept, default resources | Accept |
  | D1a reader, identical codec | Accept, default resources | Accept |

  The inherited fourth section is resource metadata and differs from the specified typed mesh/texture payload. A payload change needs an explicit ABI revision and an ABI 1 import path for both header shapes. Preserve replay provenance by checking the old bundle digest before mapping it to the converted bundle digest. The old reader rejects any new ABI. D1a produced no serialized schema change or migration of the pinned trace.

## Measurements

Physical lines use the i23-r2 `record.py` convention and count recursively included sources once. Inline tests count and the two separately named runner tests are excluded throughout. Counts follow the formatter run.

| Destination | Handoff | D1a | Delta |
| --- | ---: | ---: | ---: |
| Runner, local | 3,738 | 3,756 | +18 |
| Runner, included | 11 | 0 | −11 |
| Std, local | 991 | 1,685 | +694 |
| Std, newly included visibility helper | 0 | 11 | +11 |
| Format | 867 | 867 | 0 |
| Passes bundle lowering | 1 | 573 | +572 |
| CLI artifact adapter | 525 | 13 | −512 |
| CLI native tree | 2,019 | 2,000 | −19 |

No GPU executor was displaced into Std or Format. The native tree still contains the separate Loaded executor, so 3,756 is a lower bound on combined Layer 0. The 940 prediction and 1,000 ceiling failed.

- `tools/check_core_words.sh` passed. `cargo tree --locked -p loft-runner -p loft-format` found only the ABI, IR, format and std Loft library edges. The compiler and parser were absent.
- `CARGO_BUILD_JOBS=2 tools/gpu_lock.sh tools/dev.sh test loft-runner -p loft-cli -p loft-format -p loft-std` stopped at compile errors. The first build found the shader-stage alias. The first retry reached Phase 4/5 variant mismatches in Passes. The second retry reached two stale private-field uses in the existing edit probe. The final diff updates those uses to the prepared pipeline/group accessors. The native test sweep was left pending after the two retries.
- `CARGO_BUILD_JOBS=2 tools/dev.sh check loft-runner --target wasm32-unknown-unknown --features web-sys` exited 0 at `739f236`. The public backend trait was checked by diff. The CLI binary built successfully with `CARGO_BUILD_JOBS=2 tools/dev.sh build loft-cli --bin loft`.
- On Apple M4 Metal, the archived `prior.loft` check reported `Flight: Held`. The timed 64×64 one-frame clip at t=0.5 s exited 0 with six commands and 0.517625 ms GPU p95 against 50 ms after moving stamps onto real dispatches. The host probe accepted both archived and source-generated hermit files and the source-generated prior file. Each submitted two frames at 1280×800. The two-pixel host probe observed the expected preserved colors and occlusion in standard and reverse depth. It did not separately read back depth or inspect cached handle identities.
- The 481-tick source record and same-device replay reported the same 32-byte state digest `d953a59bb7d78610ef76609e8557b4af4bc07b7b72fd7e50b238e5b6d6efbffe`. The lossless PNGs matched byte for byte and the replay Truth command exited 0.
- Exact invocations, exits, lossless frames, bundles and binary digests are in the primary checkout under `out/meta/d1a`. The logs distinguish the failed encoder-boundary run from the passing dispatch-timestamp run. The combined suite and GPU gate belong to i6.

## ESCALATE

Revision `739f236`, input the merged Phase 2–5 runner. Expected 940 physical L0 lines with a ceiling of 1,000. Observed 3,756 runner lines before the separate native executor. The native suite exhausted both compile retries. The epoch-isolation and stale-swap assertions, native versus standalone hermit pixel parity, and browser execution remain unverified. The inherited ABI 1 resource-extension compatibility gap remains for integration. No intended visual changes were submitted for owner approval.
