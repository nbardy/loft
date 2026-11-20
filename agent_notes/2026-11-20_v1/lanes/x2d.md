# X2d · Legacy engine A/B

Compare the retired ordered renderer at `acb7e8b` with the native renderer at `1194779`. The capsules fixture matches the shape layout at both counts. At 1280 × 800 and t = 0, predict 0.5 and 0.8 ms p95 for the new engine from the previous Metal gate, and 1–2 and 2–6 ms for the legacy engine given its four-sample visibility and per-hit secondary rays. Predict five draws plus dispatches per frame on each path. The silhouettes should align. Lighting and edge coverage should differ.

| Instance groups | GPU p95, ms, legacy → native | Cold device load, ms, legacy → native | Warm device load, ms, legacy → native |
| ---: | --- | --- | --- |
| 1, first | 3.927 → 2.973 | 609.663 → 513.708 | 12.491 → 5.568 |
| 1, repeat | 2.718 → 2.269 | 362.094 → 607.304 | 6.561 → 2.607 |
| 20, first | 9.389 → 2.798 | 476.615 → 449.818 | 11.087 → 2.070 |
| 20, repeat | 9.326 → 1.019 | 405.851 → 470.712 | 7.877 → 1.828 |

At both counts, shader modules went 5 → 4, pipelines 5 → 5, frame commands 5 → 5, and authored scene tokens 1,197 → 621. The frame-time forecasts were low. The 20-group ranking held across the repeat despite a 1.780 ms shift in native p95. The 4 stills and 2 legacy bundles each reproduced bit for bit. The 2 native stills matched PF1 main RGBA, and all 18 frozen reference checksums verified. [A/B report and contact sheet](../AB_OLD_VS_NEW.md) · [Raw measurements](../../../out/meta/x2d/results.json)

The load instrumentation used the native capsules gate. In both executions Twin held 6/6 and Truth 6/6, while Pictures held 0/6 against the existing frozen appearance. All six gate RGBA hashes matched PF1 main in both executions. Speed held 3/4 because the separate 20-group orbit scored 1.174 / 1.186 ms against 1.100 ms. Load held 10/11 because the 1-group source-to-device cold load exceeded 500 ms. [First gate log](../../../out/meta/x2d/new-gate.log) · [Repeat gate log](../../../out/meta/x2d/replay/new-gate.log) · [Gate RGBA hashes](../../../out/meta/x2d/native-gate-pixels.json)

The focused native Cargo check and both optimized builds passed. Static checks verified the wgpu/Naga pins, 295 Phase 5 identities, and 281/281 grammar outcomes. [Static log](../../../out/meta/x2d/static.log) · [Workspace test log](../../../out/meta/x2d/workspace-tests.log) · [Doctest log](../../../out/meta/x2d/workspace-docs.log)
