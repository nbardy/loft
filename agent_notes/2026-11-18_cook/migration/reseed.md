# Lane: re-port the creative seeds faithfully (worktree ~/git/_wt/creative-loft-migrate, branch loft/migrate)

What went wrong: training budgets set build ≤ 2 CPU s, a number derived for the coming Rust compiler. Today's
TypeScript compiler builds a crab in ~2.0–2.1 s, so 102 of 170 seeds failed "render budget (build)" by tens of
milliseconds; the scenes are not at fault. Hand/sol "fixes" then edited seeds (see crab-*-v2/REPORT.md and
out/port/*/{hand-*,sol}.jsonl), likely cutting work to fit the budget, which changes the art. A port must keep each
program as the codemod produced it unless the API change itself required an edit.

Do:
1. Make the build budget a typed config value that names its regime: `{ compiler: "ts", build_cpu_s: 8 }` now, and
   `{ compiler: "rust", build_cpu_s: 2 }` documented as the value to switch to when Loft's Rust compiler lands (Loft
   docs/proposals/2026-11-19_lean_rust_loft.md). Derive 8 s in a comment from the measured TS builds (crab ~2.1 s, Titan
   ~3.9 s cold, heron ~4.1 s) with headroom for load. Draw budget stays 50 ms/frame at 512²; a seed over it is kept
   but flagged `over_draw_budget` with its measured ms and excluded from training task rows (not deleted).
2. Re-run the porting tool for all 170 seeds from the CODEMOD outputs (out/port/<set>/code/*.ts as the codemod wrote
   them; regenerate with `bun ~/git/loft/tools/migrate.ts` batch mode if any were edited in place). Only a seed whose
   codemod output fails for a non-budget reason gets a minimal hand edit, recorded with a diff and the reason.
3. For every seed in the current crab-*-v2 sets that was hand/sol-fixed, diff its code against the codemod output and
   report what the fix changed; replace it with the faithful port.
4. Reassemble crab-actions-v2 and crab-improve-v2 (same layout: seeds.jsonl in v1 order, manifest with the budget
   regime, SHA256SUMS, split ledger, REPORT.md with counts: codemod-only / minimal hand edit / over draw budget /
   failed with reason). Rerun the focused tests and the offline config checks.
Rules: uv for Python; no paid calls; focused tests only; at most a few renders at once (shared 24 GB Mac). Commit on
loft/migrate staging by path (never git add -A; never data_scraping/), message ending with
Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>. Do not push. Final message: the counts table and commit SHA.
