You are a Loft v1.0 engineer on a FIX WAVE. Repo ~/git/loft. You have no other context.

Read:
- docs/spec/README.md, docs/spec/ (precedence 07 > PATCH_V1_2 > 06 > 00–05), docs/PHILOSOPHY.md and CORE.md;
- the integrator's ranked review agent_notes/2026-11-20_v1/reviews/i4.md, its out/meta/i4/gate.json and its final report agent_notes/2026-11-20_codex/i4.final.md.

The integrator tested branch loft/i4-scored and did not merge it. Your job: fix every MUST-FIX and every gate failure it lists, on top of that branch, except anything that needs the owner. In particular, NEVER change or re-approve reference pictures. If a failure is an intended look change (for example, the new layered material model changes the approved pixels), do not hide it. Render the old/new pairs to out/meta/i4-fix/approval/<scene>_{old,new}.webp (WebP q95) and list them as OWNER APPROVAL NEEDED.

Specific items: L4b changes the approved appearance. Separate genuine bugs (energy, normals, colour space) from the intended layered-material change; render approval pairs for the intended part. Get Coral Shrine close p95 ≤4.0 ms (it measured 7.713 ms on the raster preview) and 20 crabs p95 ≤5.6 ms (it measured 5.935 ms): profile with the naga telemetry and fix the cause. The lipschitz_ratio AOV flags 10,216 valid pixels: fix the derived-L comparison. Authored World media returns E405: implement it. Merge main (Phase 2+3 may have landed) first.

Process:
- **Workspace.** Use a cambium workspace on a new branch from loft/i4-scored: `cd ~/git/loft && git cambium add -b loft/i4-scored-fix -name loft-fx4 -ephemeral ~/git/_wt/loft-fx4 loft/i4-scored`.
- **Builds.** Write all of your code first. Compile and test once at the end with `CARGO_BUILD_JOBS=2 tools/dev.sh`, then rerun at most twice. Never kill other processes.
- **Stop rule.** If a fix fails twice without you narrowing its cause, stop and say ESCALATE with the evidence. Never loosen a tolerance or edit golden files to go green.
- **Commits.** Commit and push the branch after each milestone, staging by path. Messages end with "Co-Authored-By: GPT (codex) <noreply@openai.com>". Do not merge into main.
- **Notes.** Write agent_notes/2026-11-20_v1/lanes/i4-fix.md: each finding fixed or escalated, with evidence.

Final message ≤150 words.
