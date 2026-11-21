# v15_meta_seeds

Based on origin/main `67cb62b`. Twelve non-crab baselines cover five rendering,
four motion, and three simulation or escape-hatch tasks. Each manifest row gives
the baseline limitation, the proposed engine rewrite, and a comparison metric.

Predicted result was 12 drawable baselines with successful check, lint, dual
grammar recognition, profile and capture. The final `--check training/seeds`
smoke met all five checks for all 12 scenes using the existing release binary.
The release's still command uses fixed 1280×800 capture. All 12 WebP q95 stills
were captured at t=0.91 s under the GPU lock and assembled into one sheet.

On Apple M4 (Metal), the locked 384×384 profiles used 30 warmup and 60 measured
frames with no overrides. GPU frame p95 was 1.361–4.603 ms, with triangle control
p95 of 0.032–0.120 ms. No Rust builds were run.

On a checkout with the release binary, run `tools/basin_test.sh --prompts OUT`
and `tools/basin_test.sh --check DIR [OUT]`. Set `LOFT_BIN` to an absolute path
when using a shared release binary. The scorer uses uv and ImageMagick.
Each cold prompt contains the live context followed by one subject brief.

The primary checkout's `out/loft-v15-meta/` holds `loft-v15-meta.tar.gz`
(seeds, manifest, and three prompts), plus `smoke/` (scores, profile JSON,
logs, stills, and contact sheet). Run the exported scenes with Loft's v1.5 stdlib.
