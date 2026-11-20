Effort: high

You are a Loft engineer. Repo ~/git/loft. Make the old-vs-new comparison apples to apples.

agent_notes/2026-11-20_v1/AB_OLD_VS_NEW.md (from lane x2d, tool tools/ab_legacy.sh) compared capsules at 1280×800 on M4, but the shading differs. Legacy `realtime()` uses matte shading, one sampled bounce, ray-traced occlusion, four-sample visibility, and its default sky and film curve. Native uses indirect visibility with its own lighting. So part of the speedup may just be less work.

Produce TWO matched comparisons, each with the same scene, camera, time, resolution and lighting features on both sides:
- **(A) Old reduced to match new.** Configure the legacy engine (tag legacy/2026-11-19, using its own options and engine composition) to drop the features native lacks: ray-traced occlusion, extra visibility samples, the bounce. Match native's lighting model as closely as legacy allows.
- **(B) New raised to match old.** Write a native `.loft` engine variant, for example engines/realtime_matched.loft forked from engines/realtime.loft, that adds the same features: one sampled diffuse bounce, ray-traced occlusion, four-sample visibility, and the same sky and film curve. Write it as ordinary forkable library code, not new compiler features. If something cannot be expressed today, say exactly what and use the closest equivalent.

For each comparison:
- **Prove the match.** Render both, report PSNR or SSIM (or the mean absolute difference) between the legacy and native frames, and make a WebP q95 contact sheet. A comparison only counts as matched if the images are visually equivalent; state the residual differences.
- **Measure:** GPU p95 over 300 frames, warm and cold load, modules, pipelines, commands, and tokens. Do this for 1 and 20 groups, two runs each, with the GPU lock held and no concurrent builds while timing. If another worker holds the GPU lock, wait.

Then extend tools/ab_legacy.sh and update AB_OLD_VS_NEW.md with a new section "Matched comparisons". Keep the original table and caveat. Give one honest headline per comparison: how much of the speedup is architecture and how much is doing less work.

Use a cambium workspace on branch loft/ab2 from origin/main. Iterate `tools/dev.sh check` until it compiles; at most 2 full GPU reruns. Merge into main yourself only if all 18 gate frames are bit-identical, since this lane must not change shipped engines. Otherwise push the branch. Commit messages end with "Co-Authored-By: GPT (codex) <noreply@openai.com>". Final message ≤150 words: both matched tables and the headline.
