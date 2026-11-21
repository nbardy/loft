# std1d · Transport and frame

Base `faba0615b6bdc33f105d51939ecf3af25c5decea` includes the scored std1a merge `f8fc612`. Scored implementation `ed6ca1e`, path background/control rerender `09b9772`, decoder spelling and README `9e9cc86`. [Prediction](../../../out/meta/std1d/prediction.md) · [Commands](../../../out/meta/std1d/scored.log) · [Targeted rerender](../../../out/meta/std1d/final-scored.log)

Vis-buffer, forward and path-primary connect authored callbacks to HDR, depth and packed IDs. Realtime print consumes shadow-map visibility and low-order cosine-convolved sky. Reference print consumes RT visibility and secondary irradiance and averages common IDs before ink. Fresh ReSTIR sky reservoirs have no temporal state. Spectral RGB is refused as a LightSummary callback. The spectral study retained its decoded RGBA.

Both one-import swaps preserved Vis64 and depth bytes at 1 and 20 rows. Forward also matched tiled woodblock HDR exactly. Capsules, Blobs and Hermit matched base RGBA. Shrine changed three beauty pixels, first at (53,47). The separate visibility probe first differed at (54,47), CoralBranch row 2262 versus 2518. All four legacy plans and WGSL matched byte-for-byte. [Hashes and raw controls](../../../out/meta/std1d/readback.json) · [Shrine rows](../../../out/meta/std1d/final/shrine-id-diff.json)

The white furnace's unclipped CPU reference was 0.500003. GPU centre-ROI mean at 1024 spp was 0.499599 against 0.5. Unit Lambertian sphere-light mean was 0.062594 against 0.0625. GPU RMS at 16/64/256 spp was 0.008600/0.004756/0.002087 for furnace and 0.011218/0.006291/0.002838 for the sphere. CPU sphere-only RMS was 0.000146/0.000071/0.000037. [Independent controls](../../../out/meta/std1d/cpu-numeric.json)

One paired Apple M4 Metal batch at 256×160, fixed View t=0, one spp, 5 warmups and 20 measured frames. Entries are p95 ms at 1/20 rows and steady commands. Base was the legacy Woodblock on the same buoy scene. Triangle controls spanned 0.319–2.872 ms. The timings are contended.

| Recipe | p95 1 / 20 | Commands |
| --- | ---: | ---: |
| Base woodblock | 8.387 / 9.768 | 7 |
| Tiled woodblock / ReSTIR | 7.930 / 9.087 · 8.493 / 9.472 | 7 |
| Forward woodblock | 9.813 / 8.661 | 4 |
| Reference woodblock / physical | 8.457 / 8.439 · 9.413 / 10.841 | 2 |
| Toon / brass | 9.089 / 9.397 · 7.888 / 7.384 | 7 |
| Luminous | 9.641 / 10.936 | 6 |

Base first load/warm was 1739/13 ms. Tiled was 1969/17 ms, forward 1620/13 ms and reference woodblock 2024/16 ms. Warm source compile kept identical WGSL and spans (0.28 → 0.08 s wall). [Per-pass timing](../../../out/meta/std1d/timing/) · [Attachment and traffic ledger](../../../out/meta/std1d/attachments.md) · [Cold/warm source](../../../out/meta/std1d/compile-cache/)

All 316 static grammar inputs passed both recognizers in six shards. Focused furnace, module, Phase 4 and native parser checks passed. The integrator owns the workspace suite. Four README renders matched exactly with B `bb1f16e` overlaid. The final lowercase parameter spelling produced identical shaders. C was unavailable for an overlay, so the examples use lane-owned print and local toon/brass. [Static](../../../out/meta/std1d/static-shards.log) · [Focused](../../../out/meta/std1d/focused.log) · [Stages](../../../out/meta/std1d/stages.log) · [README comparison](../../../out/meta/std1d/readme-render/) · [Rename](../../../out/meta/std1d/parameter-rename/equality.txt)

The one-argument physical material query, portable path bundles, texture-based sky baking, diffuse/gloss separation for mixed continuation and mip bloom remain open. Studio AO and DoF compiled as selectable passes. The inherited spectral engine still owns its four-hero source. [Optics plan](../../../out/meta/std1d/optics.plan) · [Eleven q95 approval stills and metadata](../../../out/meta/std1d/approval/index.json)
