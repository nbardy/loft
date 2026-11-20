# Lane: integrate creative's Loft work and pin it to current Loft main

Worktree ~/git/_wt/creative-loft-integrate (branch loft/integrate, from creative main). Merge, in order, reading both
sides of every conflict: `loft/vendor-submodule` (vendor/loft submodule, generated allowlist, serve exit 7),
`loft/reconstruct-migrate` (reconstruction pinned to the training Loft; 9be0fbac), `loft/migrate` (training on the
pinned Loft, live `loft context` prompts, budgets with a named compiler regime, crab-*-v2 seed sets; acd73767). The
branches each bumped vendor/loft at different times: end with ONE pin at ~/git/loft origin/main
(git -C vendor/loft fetch origin && git -C vendor/loft checkout origin/main; git add vendor/loft).
Then, on that pin:
- Regenerate the import allowlist (training/loft/host: bun generate-imports.ts && --check); review import-policy.json
  for refusals (each deny has a reason).
- Loft's API changed since e7c3c5a (authoring-API waves 3–4: draw.shader, compute in wgsl text with holes,
  `loft check --explain`, context teaches WGSL text first; see ~/git/loft/docs/MIGRATION.md and docs/api.md). Rerun
  `bun ~/git/loft/tools/migrate.ts` batch mode over the v2 seeds' code and the reconstruction ports; if anything
  changes, re-run the porting tool for those (same budget regime) and update the sets' manifests/hashes.
- Focused checks: training/loft host typecheck + imports test; PYTHONPATH=. uv run ... pytest -q training/loft; the
  seed/config offline checks; the reconstruction exporter smoke; 2 seed renders per set.
- Commit (stage by path; never git add -A; never data_scraping/ or git-ignored data), messages ending with
  Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>.
Do NOT touch the main checkout ~/git/creative (another session has staged and modified files there) and do not push.
Final message: merge conflicts and resolutions, the pin, check results, the final SHA of loft/integrate, and whether
`git -C ~/git/creative merge --ff-only loft/integrate` would conflict with the main checkout's dirty files (check
with `git -C ~/git/creative diff --name-only` and `--cached` against the files loft/integrate changes; report overlap).
