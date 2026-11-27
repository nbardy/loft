# Pending Branches, Worktrees & Stashes Audit — 27 Nov 2026

Current `main` is `f858d61f` (`Single cull + single bundle per model scene`). Since `6bde57a5`, `main` has only added/edited `out/spark_pairs/` rollout data (338 files, all under `out/`).

---

## 1. Merge Candidates (High Value, Zero Conflict with `main`)

| Branch | Tip SHA | Ahead/Behind `main` | What It Contains | Verdict |
|---|---:|---:|---|---|
| **`loft/integrate-1125`** | `a66e04b4` | +512 / −12 | Already integrates **4 completed, reviewed lanes**: (1) `loft/lang` (`f323ff6d..2e1558dc`: named & default build args, zero-row kind elision, exported `pub world` groups, `select` over `Bsdf`, comment-before-first-stmt grammar parity — 6/6 byte-identical frames), (2) `loft/perf` (certified guard boxes for field kinds, retained visibility for unchanged frames), (3) `loft/perf-harden` (hardened tile lists, off by default so constant GPU commands hold), (4) `loft/grammar-chars` (`u`/`i` int suffixes in `loft.lark`), plus duplicate shadow caster refusal. **0 overlapping files with `main`** (all 338 files changed on `main` since merge-base `6bde57a5` are under `out/spark_pairs/`). | **MERGE FIRST** (after one `tools/check.sh full` or static + focused gate check). Supersedes `loft/lang`, `loft/perf`, `loft/perf-harden`, `loft/grammar-chars`, and `rescue/2026-11-27/_wt-loft-integrate-1125`. |
| **`loft/wave1`** (or `parked/wave1-cleanup`) | `9bcc02f4` | +1 / −19 | 10-line cleanup moving motion-math classification to `loft-builtins` catalog instead of duplicating in `loft-check/src/references.rs`. | **CHERRY-PICK** `9bcc02f4` (or let `r27-soundness` absorb it). |
| **`fix/crossing-deck`** | `cd853a99` | +1 / −19 | `.loft`-only fix clamping landing into walker footing so `crossing` feet stay on ferry deck. | **HOLD FOR OWNER VISUAL CHECK** (`out/meta/motion_stride/crossing_before_after_fixed.webp`); note it adds a `Walker` field and changes trace hashes. |

---

## 2. Keep Parked (Do NOT Merge as-is)

| Branch | Tip SHA | Why Keep Parked / Superseded |
|---|---|---|
| `loft/image-laws` / `parked/image-laws` | `41ffb4e7` | Uncompiled prototype of a law over `ColorHdr`. Revisit only after unified pass graph lands. |
| `loft/me-host`, `loft/me-typed`, `loft/me-int`, `loft/metaengine`, `loft/metaengine-core` | `1afd3cd6` etc. | Deliberately rejected in `agent_notes/2026-11-23_meta_engine/CLOSE.md` §"Second close" (S1/S3 saved almost no lines without `cut-tape` and added no user capability). Keep as reference or archive tag before deleting. |
| `loft/boss-otter`, `loft/boss-walrus`, `loft/boss-heron`, `loft/boss-whiskers` | various | Active/recent character scene studies (otter r18, walrus r17, heron v2, whiskers). Keep if still needed for character ballots; do not merge into core `main`. |
| `loft/fur-field`, `loft/fur-strands`, `loft/fur-surface`, `auncel/a`–`d` | various | Research/study branches. Archive or leave untouched until studies close. |

---

## 3. Stale Worktrees, Uncommitted Primary-Checkout Edits & Stashes to Clean Up

### Dead / Detached Worktrees (`git worktree list`)
1. **4 prunable dead worktrees** (directories already gone):
   - `/private/var/folders/.../cambium-seed-warm-2378440055/worktree` (`cambium/cambium-seed-warm-2378440055`)
   - `/Users/nicholasbardy/git/_wt/boss-otter` (`loft/boss-otter`)
   - `/Users/nicholasbardy/git/_wt/loft-lean2` (`loft/lean2-merge`)
   - `/Users/nicholasbardy/git/_wt/loft-merge-2026-11-23` (`merge-2026-11-23-r2`)
   - **Action:** `git worktree prune`.
2. **7 orphaned `.muse/worktrees/subagent-v2-*` worktrees** (all detached HEAD at `2afc5e55`, Nov 25):
   - **Action:** Inspect for uncommitted diffs, then `git worktree remove --force .muse/worktrees/subagent-v2-*`.

### Primary Checkout Uncommitted Changes (7 modified files + untracked `.done`/`.final.md` logs)
- Modified: `.gitignore`, `tools/dev.sh` (merges light/heavy compile lock), `agent_notes/2026-11-20_codex/run.sh`, `v15_context.brief.md`, `v15_motion_mood.brief.md`, `tools/crab_concepts.py`, `tools/crab_faces.py`.
- **Action:** Review and either commit `tools/dev.sh` + `.gitignore` cleanly on `main` or stash/drop stale `2026-11-20_codex` scratch edits so `main` is clean before merging `loft/integrate-1125`.

### Historical Branches & Stashes (~60+ obsolete branches)
- Safe to delete once `loft/integrate-1125` merges (after optional `archive/2026-11-27` tag if desired):
  - Redundant `rescue/2026-11-21/*`, `rescue/2026-11-24/*`, `rescue/2026-11-26/*`, `rescue/2026-11-27/*` (duplicates of named branches).
  - Historical merged/superseded refs: `loft/lean2*`, `merge-2026-11-23*`, `perf-integrate*`, `perf-frame*`, `perf-load`, `perf-edges`, `perf-instrument`, `perf-precision`, `cut-tape`, `cut-unused`, `probe-door`, `probe-rows`, `tools-agent-build`, `fix/context-lobes-builtins*`, `fix/one-shot-errors`, `fix/space-hints`, `fix/missing-return`, `fix/round-cone-divisor`, `fix/bsdf-values`, `loft/i1-*`, `loft/d2-*`, `loft/v15-*`, `loft/cap1c`, `loft/phase2`, `loft/std1c`, `loft/std1d`, `loft/port1`, `loft/x2b`, `cambium/cambium-seed-warm-*`.
- **4 old stashes** (`stash@{0..3}` from `build1-*` and `v15a`): verify empty/superseded and `git stash clear`.
