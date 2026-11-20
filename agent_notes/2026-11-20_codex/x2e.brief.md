You are a Loft engineer. Repo ~/git/loft, main at or after 315cd04 (v1.0-alpha plus the runner cut). You have no other context.

Read docs/spec/README.md (precedence 07 > PATCH_V1_2 > 06 > 00–05), docs/PHILOSOPHY.md, CORE.md, agent_notes/2026-11-20_v1/OPEN.md and agent_notes/2026-11-20_v1/design/d1.md.

Rules:
- Use a cambium workspace: `git cambium add -b loft/x2e -name loft-x2e -ephemeral ~/git/_wt/loft-x2e origin/main`.
- Builds: write the code first. Then iterate on compile errors with `CARGO_BUILD_JOBS=2 tools/dev.sh check` (cargo check is cheap) until it compiles cleanly; never ESCALATE on a compile error. The 'at most 2 reruns' limit applies only to full test and GPU runs.
- Kinds of thing are sum types with one exhaustive dispatcher, with no kind or null checks inside handlers. No silent fallbacks.
- NEVER change or re-approve reference pictures; pixels must stay bit-identical unless your task says otherwise.
- Do not merge into main. Push your branch.
- If a fix fails twice without its cause narrowing, ESCALATE with evidence.
- Write agent_notes/2026-11-20_v1/lanes/x2e.md with measured before and after values.
- Owned files: a new directory web/threejs_demo/ (HTML/JS, pnpm only if needed), and minimal fixes in crates/loft-std/src/web.rs. Touch nothing else except your lane note.
- Commit messages end with "Co-Authored-By: GPT (codex) <noreply@openai.com>".

Task: Build INTEGRATION_TODO.md §1 (agent_notes/2026-11-20_v1/INTEGRATION_TODO.md): a minimal page on Three's WebGPURenderer; `loft build engines/hermit.loft` to a .loftb; the WASM runner attaches to renderer.backend.device and loads it; Three draws a cube, Loft draws the crab into the same color target against Three's depth; one light's direction passed from Three to Loft params. Accept: mutual occlusion correct both ways, one shared GPUDevice, no readback, frame time printed; verified headless in Chrome via Playwright or the agent-browser CLI with a WebP q95 screenshot in out/meta/x2e/. Document how to run it in web/threejs_demo/README.md.

Final message ≤150 words.