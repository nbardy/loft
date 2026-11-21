# V15 execution integration

Base `e200231`. One Cambium lane folded X2a `68400b0` and Wave 15 `f83ecd6`, `bf1e54f`, and `699216e`.

The source host lowers to the same bundle declarations as portable export. Both callers share runner preparation and command recording. `loft-native` owns device setup, source orchestration, profiling, and presentation. `loft-wgpu` owns GPU attachment, pipeline, and resource helpers. The per-frame uploads use a staging belt tied to encoder submission. Resource usage follows the declared passes, and storage bindings expose their logical byte ranges. Native World shadows and bounce rays use Metal AABB traversal and the authored intersection functions. Portable bundles retain software traversal.

## Rust code lines

SCC 3.7.0 counted Rust under `crates/` with `tests/` directories excluded. In-source test code is included.

| Crate | Main | Lane |
| --- | ---: | ---: |
| loft-abi | 372 | 372 |
| loft-cli | 6,869 | 4,801 |
| loft-format | 740 | 740 |
| loft-ir | 456 | 627 |
| loft-native | 0 | 1,429 |
| loft-parse | 2,652 | 2,670 |
| loft-passes | 16,146 | 17,829 |
| loft-plan | 859 | 859 |
| loft-runner | 1,787 | 2,179 |
| loft-std | 4,305 | 1,991 |
| loft-wgpu | 0 | 2,796 |
| shared | 10 | 10 |
| **Core** | **34,196** | **36,303** |
| CLI native subtree, included above | 2,208 | 0 |

`tools/check.sh lines` passes the two execution ceilings and the dependency ceiling. It exits 1 on the total, 18,303 lines above the 18,000 limit. The GPU and native host splits are counted in full. Main's parser and passes crates alone were already at 18,798 lines.

`cargo tree -p loft-std -i wgpu` changed from:

```text
wgpu v30.0.1
└── loft-std v0.1.0
```

to:

```text
error: package ID specification `wgpu` did not match any packages
```

## Verification

- `cargo check --workspace --all-targets` passed. The workspace suite first passed 178 with one ignored evidence test after fixing the base's stale corpus path. The final rerun passed 177 with one ignored and one Tier 0 timing miss at 0.557 ms against 0.500 ms. That test passed in a quiet standalone retry at 0.346 ms, with zero pixels above 1/255 in both runs. The separate full-resolution archive test matched all 18 native frames byte-for-byte.
- The raw compute readbacks, NonDet statistical twin, sampled StudyInk native and decoded-bundle pixel match, every-mip bake parity, and multi-session encoder test passed. Rust picture, temporal, and timing scorer controls passed.
- An Apple M4 Metal smoke test confirmed `hardware_ray_query: true` and five raster schedule passes. Its two frames at 128×80 are a binding check.

## Release gates on Apple M4 Metal

- The 1280×800 release gate held Pictures 19/19, Twin 36/36, and Truth 34/34. Ordered captures matched current main byte-for-byte in RGBA, ID, and D32 for all 19 frames. The stochastic fixture scored 41.73–42.07 dB at variance 0.00004491. The gain mutant failed at 6.74 dB.
- Speed held 10/12. Capsules-1 measured 0.860 ms against 0.800 ms. Hermit-20 timing was unknown at 0.210 ms triangle p95. Load held 25/33. Cold misses were Capsules-20 1.136 s, Blobs-20 1.393 s, Hermit-1 1.963 s and Hermit-20 1.928 s. Hermit warm loads reached 0.194 s in both cases against 0.060 s, with the inherited 14 vs 13 pipeline counts.
- The first Table B window refused timing at 0.678 ms triangle p95. The focused B-20 retry measured **10.523 ms** with hardware traversal against **14.583 ms** legacy over 300 frames at 1280×800 and time 0. Its candidate p95 95% interval was [10.253, 10.979] ms. Before and after triangles were 0.118 and 0.066 ms p95. The in-profile triangle was 0.114 ms. No build overlap was detected. Compared with the held v15a software render, B-1 and B-20 had 37 and 1,852 pixels above 2/255.
- The six-pair Speed/Load run refused the first window at 0.638 ms triangle p95. The retry preserved four complete pairs, then refused the fifth candidate arm at 0.190 ms. Capsules-20 also had an internal 0.209 ms control. No six-pair verdict was certified. Across the four recorded pairs Hermit-20 warm loads were 75.20–118.81 ms slower.
- The wasm32 runner check passed. Both grammar recognizers matched expected results for 306/306 files, and the current shell scripts passed syntax checks. The release gate and picture stages were run directly after stopping the duplicate full-wrapper corpus scan.

The main merge is held by the core ratchet, release Speed and Load misses, the unqualified paired Speed/Load run, and the final workspace timing miss. [Evidence](../../../out/meta/v15_exec/)
