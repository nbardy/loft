You are Loft's integrator. Your job: delete the old code from main, for real this time. Repo ~/git/loft. You have no other context.

The overnight step d0 deleted the legacy TS engine (src/), the old Rust (rust/) and the old tools and docs on branch loft/delete-legacy (8e7af64: 690 files, -96k lines), but it was NEVER MERGED. Since then, main gained v1.0-alpha, the runner cut and w1–w4. The gates may still use legacy pieces: the integrators ran "315 Bun tests" and "linked legacy lab binaries", and package.json references `bun src/...`.

1. **Workspace.** Use a cambium workspace on branch loft/delete-legacy-2 from origin/main. Use loft/delete-legacy as the guide, and re-apply the deletion against current main; do not blindly merge it.
2. **Delete:**
   - src/, rust/, and the old TypeScript tests and tools that only exercise the old engine;
   - old docs, moved to docs/legacy/ (or deleted where git history suffices);
   - DEPRECATED.md files that point at deleted code;
   - DESIGN.md (175 KB, the old engine), moved to docs/legacy/.

   Keep examples/ as read-only Phase 5 source with a README saying it no longer runs (as d0 did). Keep corpus/, train/ and viewer/ only if the new workspace uses them; otherwise move them to docs/legacy or delete them, and say which.
3. **Re-point whatever the gates and integrators actually use** (out/meta/*/run.py, score.py and pictures.py are evidence scripts; the real gate entry points are under tools/ and crates/) to the new Rust workspace only. Any gate that truly needs a legacy binary must be rewritten against the new CLI, or explicitly listed as removed in OPEN.md with the reason. No TypeScript may remain in the product. Bun/pnpm stay only if a remaining tool truly needs them; otherwise remove package.json, pnpm files, bunfig.toml and tsconfig.json.
4. **Rewrite README.md, AGENTS.md (CLAUDE.md links to it), CORE.md, tools/check.sh, tools/dev.sh and .cambium.toml** for the new workspace only. AGENTS.md must no longer mention the TS engine, bun test, src/ or rust/.
5. **Checks.** Build once with tools/dev.sh. Run all native tests, wasm32, grammar and the mutants, then ONE locked GPU five-gate run (tools/gpu_lock.sh). The scores must equal current main's (Twin 36/36, Truth 34/34, Speed 12/12, Load 31/33) and the pixels must be bit-identical.
6. **Merge and push.**
   - Merge into main and push to GitHub (origin).
   - Delete the stale branch loft/delete-legacy locally and on origin.
   - Write out/meta/del2/MERGED with the sha and a line count of what remains: `git ls-files | cut -d/ -f1 | sort | uniq -c`.
   - Update OPEN.md.

NEVER touch tools/crab_concepts.py or tools/crab_faces.py (another session's uncommitted edits). Do not stage them, do not delete them, and do not reset them; leave them modified in place. Commit messages end with "Co-Authored-By: GPT (codex) <noreply@openai.com>". Final message ≤150 words: what was deleted, what was kept and why, the gate scores, and the merged sha.
