You are a Loft v1.0 engineer on a FIX WAVE. Repo ~/git/loft. You have no other context.

Read:
- docs/spec/README.md, docs/spec/ (precedence 07 > PATCH_V1_2 > 06 > 00–05), docs/PHILOSOPHY.md and CORE.md;
- the integrator's ranked review agent_notes/2026-11-20_v1/reviews/i5.md, its out/meta/i5/gate.json and its final report agent_notes/2026-11-20_codex/i5.final.md.

The integrator tested branch loft/i5-scored and did not merge it. Your job: fix every MUST-FIX and every gate failure it lists, on top of that branch, except anything that needs the owner. In particular, NEVER change or re-approve reference pictures. If a failure is an intended look change (for example, the new layered material model changes the approved pixels), do not hide it. Render the old/new pairs to out/meta/i5-fix/approval/<scene>_{old,new}.webp (WebP q95) and list them as OWNER APPROVAL NEEDED.

Specific items: Bring the escalated lanes L5a (glTF import and retarget), L5d and L5f (corpus) back to green. Make the standalone .loftb runner load multi-render-target bundles (hermit). Clear the blockers on import, retargeting, grammar, hardware ray queries and runner constraints. Corpus: there are no approved references for the 77 regenerated scenes, which needs the owner. Make every scene build and pass truth and twin on the CPU, and render one contact sheet per 15-scene batch to out/meta/i5-fix/approval/ for owner approval. Do not count them as verified. Merge main first.

Process:
- **Workspace.** Use a cambium workspace on a new branch from loft/i5-scored: `cd ~/git/loft && git cambium add -b loft/i5-scored-fix -name loft-fx5 -ephemeral ~/git/_wt/loft-fx5 loft/i5-scored`.
- **Builds.** Write all of your code first. Compile and test once at the end with `CARGO_BUILD_JOBS=2 tools/dev.sh`, then rerun at most twice. Never kill other processes.
- **Stop rule.** If a fix fails twice without you narrowing its cause, stop and say ESCALATE with the evidence. Never loosen a tolerance or edit golden files to go green.
- **Commits.** Commit and push the branch after each milestone, staging by path. Messages end with "Co-Authored-By: GPT (codex) <noreply@openai.com>". Do not merge into main.
- **Notes.** Write agent_notes/2026-11-20_v1/lanes/i5-fix.md: each finding fixed or escalated, with evidence.

Final message ≤150 words.
