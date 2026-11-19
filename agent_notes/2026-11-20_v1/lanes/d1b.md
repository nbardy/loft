# D1b · Recover GPU headroom

## Predictions before implementation

| Work | Baseline | Prediction and acceptance probe |
| --- | --- | --- |
| Hermit-20 | i4 p95 5.989 ms; Vis, Shadow and Shade medians 1.581, 0.638 and 2.513 ms; shell 621 ALU, 32 transcendentals and 43 storage loads | 5.1 ms p95. Bake the 193 × 33 distinct UV vertices once at load for the exact 192 × 32 cells. Keep 36,864 corners, winding, per-instance pose, and both seam endpoints. Compare projected positions, depth and approved-shelf controls before timing. Lift view and roughness terms and fixed sky directions from the light loop by dependency. Preserve samples, order, BSDFs and shared module count at 1 and 20 instances. |
| Coral Shrine | Close p95 4.374 ms; Vis, Shadow and Shade medians 2.217, 0.456 and 0.591 ms | 3.7 ms p95 at the archived close camera and 1280 × 800, 30 warmups and 100 frames. Reject conservative branch clusters before exact leaf tests. Query the solid world from receiver hits and draw the carved lantern with all 3,072 instances. Check paired bore rays and keep raw pass stamps. |
| Phase 5 timing | 6/12; 5 capsule and blob commands, 17 hermit commands; control p50 0.028–0.057 ms. The fixed-camera i5 capsule replay also tails. | Ring p95 at 1 and 20 instances, respectively, of 0.65 and 0.95 ms for capsules, 0.85 and 1.9 ms for blobs, and 4.4 and 5.1 ms for hermits. Official Speed 12/12. Pair immediate retirement and a bounded ring over the same 300 orbit inputs, command stream, queue and shader hashes. Each retained serial records first start to last end and pass stamps after its own submission completes. Record both tails and control times. The 0.35 ms threshold stays fixed. |
| Bakes | 4.358 ms for 20 dispatches across a 2048² mask and 128² cube | 1.8 ms through the last mip, with separate base and mip stamps. Hoist column-invariant work and specialize box filters per format. Retain every format conversion and tail. Verify every mip texel against the existing pipeline. Expect 20 load dispatches and 5/0/0/5/0 frame bake commands. Compare every component of dirty parameters through the common execution plan. |
| Spectral | 9 storage buffers in one stage, 8 available | At most 8 per stage and a rendered 128² four-sample study. Pack checked read-only descriptor/media words into the existing arena. Preserve group 0 and writable photon bins. Count by entry point and stage before creating the actual pipelines. Verify closed-world, energy, media and BVH probes. |

Evidence baseline is in the primary checkout under `out/meta/{i4-fix,i4-r2,i5-r2,d1}`. The 18 layered cases retain their existing owner decisions. Keep lossless inputs for pixel comparisons.

## Base

The lead ruling supplies the Phase 2 and 3 line from `loft/int23-fix`. Main at `8fd710db37d4b1d701f6b0d4cd97accf9b84e434` lacks `d4b7646`; the m23 Speed 6/12 result is the frame-time investigation baseline. The lane fast-forwarded to `d8ca52e`, the common merge of that line with `origin/main`. The ownership base is the published `Prepare D1 lane base` commit after verifying both scored ancestors.
