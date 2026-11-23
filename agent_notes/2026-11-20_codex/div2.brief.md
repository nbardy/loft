# div2: render-engine and material diversity for the 16 crab seeds
Effort: xhigh

Lead: you own the design and the implementation. Repos: ~/git/creative, worktree from `loft/seed-diversity` on a new branch `loft/seed-render`, plus ~/git/loft (main) if a stdlib engine or material is missing. Never touch other sessions' uncommitted files. Never start a paid training run.

## Problem
div1 fixed shape (see agent_notes/2026-11-20_v1/design/div1.md and out/meta/gallery/div1_sheet.webp). Rendering is still narrow:
- 15 of the 16 seeds use the same `stdlib::integrators::direct::sun_sky`, and one uses woodblock. None uses the other shipped engines: stdlib/engines/{realtime, realtime_matched, reference, spectral_caustic, woodblock}.loft, plus the restir integrator.
- Every seed uses the same `ggx + coat + emit_lobe` from one shared crab.loft. The "looks" are 4 FINISH tuples in species.py, so it is roughness and coat changing, not render style.
- No seed looks like a real crab: dull, rough chitin; mottled, non-uniform pigment; darker joints; slightly wet sheen only on top; ambient occlusion in the leg sockets; soft sky light. They all read as clean plastic toys.

## Goal
Keep each seed's div1 species, pose and camera (shape diversity is approved). Give the 16 seeds substantially different render pipelines and materials, chosen from at least 6 render families, at least 2 seeds each:
1. **Photoreal chitin** (at least 3 seeds): the reference or realtime_matched engine; rough GGX (roughness 0.55–0.8, coat ≤0.1 and only on dorsal surfaces); procedural mottling, pits and wear in albedo and roughness; darker, desaturated joints and leg tips; AO; sky plus sun; neutral grading. Target: a natural-history photo of a live crab on wet sand. It must not look shiny.
2. Woodblock / print (the existing style).
3. Toon / cel with ink outlines.
4. Painterly or watercolour (a new style file in stdlib/styles if needed; follow the std ≥3× rule in docs/PHILOSOPHY.md §4).
5. Spectral caustics or underwater: the crab under shallow water with caustics, using the spectral_caustic engine.
6. One more family of your choice that differs in the integrator or schedule, not only the material, e.g. restir night scene with many small lights, or silhouette/backlit graphic.
Write new styles or materials as forkable stdlib code where they recur; otherwise put them inline in the seed.

## Measure
- Add a **style** check to the div1 gate: DINO distance on a grayscale, blurred-silhouette-removed version is not enough. Use the pinned DINOv2 on the full beauty plus a simple palette/contrast/edge-density style descriptor. Report the nearest style neighbour per seed, before and after. Register the prediction before rendering, as div1 did.
- All seeds must still pass div1's shape gate, check, strict lint and both grammars, with clip budget ≤2,000 ms CPU and ≤50 ms GPU at 512².
- Realism check for the photoreal seeds: highlight coverage (pixels above 0.9 luminance inside the matte) ≤2%, and roughness and albedo variance across the shell above a threshold you set from a reference photo of your choice. Report the numbers.

## Deliver
- An old-vs-new sheet at ~/git/loft/out/meta/gallery/div2_sheet.webp: the div1 render beside the div2 render, labelled with species and render family. Also one 3×3 close-up sheet of the photoreal seeds.
- Commits on `loft/seed-render`. The Creative remote cannot take this history (there is a 477 MB mbox in old history), so also write a verified git bundle to ~/git/loft/out/meta/div2/. Loft-side stdlib additions go on Loft main via a branch plus merge, pushed.
- Do not merge into the training branch: the lead reviews the sheet first.
- Builds and tests go through ~/.local/bin/agent-build, one at a time; GPU renders take /tmp/loft-gpu.lock.
- Commit messages end with "Co-Authored-By: GPT (codex) <noreply@openai.com>".
- Final message ≤150 words: families × seeds table, style-distance before and after, realism numbers, sheet path.
