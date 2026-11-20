# Loft build lane (18 Nov 2026)

Loft (`~/git/loft`) is a TypeScript (Bun) library: a scene is a module of expressions that compiles to WGSL + tables
and draws through WebGPU (Dawn via bun-webgpu; a Rust/wgpu loader in `rust/loft-loader` for the game). Read
`AGENTS.md`, the parts of `DESIGN.md` and `docs/` your lane touches, and the code around your change before writing.
The plan is `docs/proposals/2026-11-18_next.md`. The owner's positions: the model writes arbitrary maths (libraries
save tokens, never fence it in); looks are swappable parts; everything extensible; fast and light; every new public
name must buy a capability.

## Rules
- You run **unsandboxed** so you can use the GPU. Write only inside your worktree (named below); `out/` inside it for
  renders. Never touch `~/git/loft` itself, other worktrees, or any other repository. **Do not commit, push, or run
  git commands that change state**; the reviewer commits.
- Kinds of thing are variants of a sum type with one exhaustive dispatcher. No silent fallbacks: a typed error, or a
  named variant that records the default. Errors name the fix (`docs/errors.md`).
- A new public name must buy a capability. Write the user's code first (an example scene), read it cold, cut.
- Tests: only for results others build on, each able to name the bug it catches. Visual features get an example scene
  under `examples/` plus a GPU test of a measured property (not a golden image), and stills saved as WebP q95
  (`cwebp -q 95`) under `agent_notes/2026-11-18_<lane>/` (a few, small). Clips: SVT-AV1 (`-c:v libsvtav1 -preset 10
  -crf 42`).
- Checks: `pnpm typecheck`, your tests, `bun src/cli/main.ts check`/`build`/`still` on your example, then hand off to the integrating
  reviewer for one full suite on the combined revision. A busy test lock means defer and report the pending check. Regenerate `docs/api.md` (`bun src/cli/main.ts api`) if exports change;
  update `docs/guide.md` / `docs/errors.md` / `DESIGN.md` where behaviour is described.
- State costs as measured (ms per frame at a stated size, memory) or derived (say so).
- Write `agent_notes/2026-11-18_<lane>.md` (or a folder with that name): what, why, the user's code, measurements,
  what is left.
- Final message: a 150-word summary: done / not done (reason), with numbers.

