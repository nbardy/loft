You are a Loft engineer. Repo ~/git/loft, main at or after 315cd04 (v1.0-alpha plus the runner cut). You have no other context.

Read docs/spec/README.md (precedence 07 > PATCH_V1_2 > 06 > 00–05), docs/PHILOSOPHY.md, CORE.md, agent_notes/2026-11-20_v1/OPEN.md and agent_notes/2026-11-20_v1/design/d1.md.

Rules:
- Use a cambium workspace: `git cambium add -b loft/x2a -name loft-x2a -ephemeral ~/git/_wt/loft-x2a origin/main`.
- Builds: write the code first. Then iterate on compile errors with `CARGO_BUILD_JOBS=2 tools/dev.sh check` (cargo check is cheap) until it compiles cleanly; never ESCALATE on a compile error. The 'at most 2 reruns' limit applies only to full test and GPU runs.
- Kinds of thing are sum types with one exhaustive dispatcher, with no kind or null checks inside handlers. No silent fallbacks.
- NEVER change or re-approve reference pictures; pixels must stay bit-identical unless your task says otherwise.
- Do not merge into main. Push your branch.
- If a fix fails twice without its cause narrowing, ESCALATE with evidence.
- Write agent_notes/2026-11-20_v1/lanes/x2a.md with measured before and after values.
- Owned files: crates/loft-runner, crates/loft-cli native execution/host code, crates/loft-passes pass-preparation modules (NOT motion/solver codegen, NOT shading). Touch nothing else except your lane note.
- Commit messages end with "Co-Authored-By: GPT (codex) <noreply@openai.com>".

Task: Implement d2.md class 3 (agent_notes/2026-11-20_v1/design/d2.md): one execution path. Build on branch loft/d2c (d2c's corrected head has explicit buffer ranges, numeric binding resolution, packed bake fallback and a shared frame recorder; its build stopped on a trivial compile error, now fixed in principle). Merge origin/main first. Targets: native `Loaded` uses the full shared PassDecl preparation; portable descriptors carry every field (binding lengths, slot numbers, not names); feature-gated R16Float with a typed refusal; concurrent pipeline creation on cold load. All 18 frames bit-identical to main; Load ≥31/33; execution code ≤2,500 lines (report per crate before and after). Delete every duplicate path you replace.

Final message ≤150 words.