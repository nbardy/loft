# Authoring API build: lane brief (18 Nov 2026)

The design is `~/git/loft/docs/proposals/2026-11-18_authoring_api.md`. Read all of it, then your lane's row in its
§9 work plan (owner files, dependencies, pass/fail check) and the section(s) it implements. Build that lane completely:
code, examples migrated, docs (`docs/guide.md`, `docs/errors.md`, `DESIGN.md` where behaviour is described;
`docs/api.md` regenerated with `bun src/cli/main.ts api`). Where the design is wrong once you are in the code, do
the right thing and say why in your note; do not silently diverge.

## Setup
`cd ~/git/loft && git cambium add -b feat/api-<lane> -name loft-api-<lane> -ephemeral ~/git/_wt/loft-api-<lane>` (from
current main), then `cp -c -R ~/git/loft/node_modules ~/git/_wt/loft-api-<lane>/`. Work only in that worktree.

## Rules
- Edit only your lane's owner files (plus new files and the examples/tests your change requires). If you need a
  one-line registration in a file another lane owns, list it in your note for the integrator instead.
- Sum types with one exhaustive dispatcher; no silent fallbacks (typed error or a named variant); errors name the fix.
- A new public name must buy a capability. Write the user's code first and read it cold.
- Tests only for results others build on, each able to name the bug it catches. Run `pnpm typecheck` and your
  focused tests only. **Do not run the full suite** (the integrator runs one per wave; the machine is shared).
  GPU-heavy runs: keep them few and small.
- Stills: WebP q95, a few, small, under `agent_notes/2026-11-18_api_<lane>/`.
- Write `agent_notes/2026-11-18_api_<lane>.md`: what changed, the user's code, the pass/fail check with its measured
  result, what is left.
- Commit on `feat/api-<lane>` in your worktree, staging by path (never `git add -A`, never `tools/crab_*.py`), message
  ending with `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`. Do not push or merge.
- Final message: 150 words: the pass/fail check result with numbers, done / not done, and integrator notes.
