You are a Loft engineer. Repo ~/git/loft, main at or after 315cd04 (v1.0-alpha plus the runner cut). You have no other context.

Read docs/spec/README.md (precedence 07 > PATCH_V1_2 > 06 > 00–05), docs/PHILOSOPHY.md, CORE.md, agent_notes/2026-11-20_v1/OPEN.md and agent_notes/2026-11-20_v1/design/d1.md.

Rules:
- Use a cambium workspace: `git cambium add -b loft/x2c -name loft-x2c -ephemeral ~/git/_wt/loft-x2c origin/main`.
- Builds: write the code first. Then iterate on compile errors with `CARGO_BUILD_JOBS=2 tools/dev.sh check` (cargo check is cheap) until it compiles cleanly; never ESCALATE on a compile error. The 'at most 2 reruns' limit applies only to full test and GPU runs.
- Kinds of thing are sum types with one exhaustive dispatcher, with no kind or null checks inside handlers. No silent fallbacks.
- NEVER change or re-approve reference pictures; pixels must stay bit-identical unless your task says otherwise.
- Do not merge into main. Push your branch.
- If a fix fails twice without its cause narrowing, ESCALATE with evidence.
- Write agent_notes/2026-11-20_v1/lanes/x2c.md with measured before and after values.
- Owned files: lib/light.loft, lib/bsdf.loft sky terms, engines/hermit.loft and engines/realtime.loft sky usage, bake_tex declarations. Touch nothing else except your lane note.
- Commit messages end with "Co-Authored-By: GPT (codex) <noreply@openai.com>".

Task: Hermit-20 shading spends 1.54 ms on 12 sky samples per pixel (agent_notes/2026-11-20_v1/design/pf1.md). Replace them with a prefiltered sky irradiance (and if needed specular) cubemap baked with `bake_tex ... on dirty(sun params) mips(true)` (spec 07 §4). Target: hermit-20 shade p50 down ≥1.0 ms. This is a look change by nature: render old/new pairs to out/meta/x2c/approval/ (WebP q95) and list them as OWNER APPROVAL NEEDED if any frame changes beyond 1/255; never re-approve references.

Final message ≤150 words.