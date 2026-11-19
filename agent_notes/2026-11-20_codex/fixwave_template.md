You are a Loft v1.0 engineer on a FIX WAVE. Repo ~/git/loft. You have no other context.

Read:
- docs/spec/README.md, docs/spec/ (precedence 07 > PATCH_V1_2 > 06 > 00–05), docs/PHILOSOPHY.md and CORE.md;
- the integrator's ranked review agent_notes/2026-11-20_v1/reviews/__ID__.md, its out/meta/__ID__/gate.json and its final report agent_notes/2026-11-20_codex/__ID__.final.md.

The integrator tested branch __BRANCH__ and did not merge it. Your job: fix every MUST-FIX and every gate failure it lists, on top of that branch, except anything that needs the owner. In particular, NEVER change or re-approve reference pictures. If a failure is an intended look change (for example, the new layered material model changes the approved pixels), do not hide it. Render the old/new pairs to out/meta/__ID__-fix/approval/<scene>_{old,new}.webp (WebP q95) and list them as OWNER APPROVAL NEEDED.

Specific items: __ITEMS__

Process:
- **Workspace.** Use a cambium workspace on a new branch from __BRANCH__: `cd ~/git/loft && git cambium add -b __BRANCH__-fix -name __WS__ -ephemeral ~/git/_wt/__WS__ __BRANCH__`.
- **Builds.** Write all of your code first. Compile and test once at the end with `CARGO_BUILD_JOBS=2 tools/dev.sh`, then rerun at most twice. Never kill other processes.
- **Stop rule.** If a fix fails twice without you narrowing its cause, stop and say ESCALATE with the evidence. Never loosen a tolerance or edit golden files to go green.
- **Commits.** Commit and push the branch after each milestone, staging by path. Messages end with "Co-Authored-By: GPT (codex) <noreply@openai.com>". Do not merge into main.
- **Notes.** Write agent_notes/2026-11-20_v1/lanes/__ID__-fix.md: each finding fixed or escalated, with evidence.

Final message ≤150 words.
