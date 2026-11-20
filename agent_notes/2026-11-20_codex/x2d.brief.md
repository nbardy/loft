You are a Loft engineer. Repo ~/git/loft, main at or after 315cd04 (v1.0-alpha plus the runner cut). You have no other context.

Read docs/spec/README.md (precedence 07 > PATCH_V1_2 > 06 > 00–05), docs/PHILOSOPHY.md, CORE.md, agent_notes/2026-11-20_v1/OPEN.md and agent_notes/2026-11-20_v1/design/d1.md.

Rules:
- Use a cambium workspace: `git cambium add -b loft/x2d -name loft-x2d -ephemeral ~/git/_wt/loft-x2d origin/main`.
- Builds: write the code first. Then iterate on compile errors with `CARGO_BUILD_JOBS=2 tools/dev.sh check` (cargo check is cheap) until it compiles cleanly; never ESCALATE on a compile error. The 'at most 2 reruns' limit applies only to full test and GPU runs.
- Kinds of thing are sum types with one exhaustive dispatcher, with no kind or null checks inside handlers. No silent fallbacks.
- NEVER change or re-approve reference pictures; pixels must stay bit-identical unless your task says otherwise.
- Do not merge into main. Push your branch.
- If a fix fails twice without its cause narrowing, ESCALATE with evidence.
- Write agent_notes/2026-11-20_v1/lanes/x2d.md with measured before and after values.
- Owned files: a new tool tools/ab_legacy.sh and its note only; check out tag legacy/2026-11-19 into a cambium workspace (read-only use). Touch nothing else except your lane note.
- Commit messages end with "Co-Authored-By: GPT (codex) <noreply@openai.com>".

Task: Measure the old engine against the new on the same scene. Build/run the old engine from tag legacy/2026-11-19 (its own README/AGENTS at that tag: bun + its Rust loader) and the new from origin/main. Pick the closest matching scene (hermit crab, 1 and 20 instances, or capsules) at 1280×800 with the same camera and time. Report per engine: frame p95 over 300 frames, cold load, warm load, shader modules, pipelines, GPU commands per frame, and the source tokens of the scene. Hold the GPU lock for each run; no concurrent builds during timing. Write agent_notes/2026-11-20_v1/AB_OLD_VS_NEW.md with a table, contact sheet of both renders, and the caveats of any mismatch in scene content.

Final message ≤150 words.