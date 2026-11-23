# lean2: core to ≤18,000 Rust lines (R2 wave 2), one owner
Effort: xhigh

Lead: you are the single owner of the core cut (docs/V1_5_GOALS.md R2 and G5.1). Never split this into parallel lean lanes (the scientist's rule after wave 1). Never start a paid training run. Never touch other sessions' uncommitted files.

## Where we are
`scc` on crates/ counts 39,095 Rust code lines (passes 6,243, check 5,096, cli 4,736, eval 3,391, wgpu 2,755, parse 2,718, emit 2,400, tier0 2,347, runner 2,155, std 1,986, native 1,438, plan 1,095, smaller ones below). The target is ≤18,000 under `tools/check.sh lines`, and wave 2 aims at ~14,500–15,500. Wave 1 (lean1a–d, ilean) gave one execution path and a wgpu-free loft-std; read those final notes first. The division of ownership is fixed: the compiler owns parse/check → typed HIR, AD and Lipschitz, Table+Kernel+Pass lowering, and bundles. The library (stdlib/*.loft) owns all rendering, lighting, schedule and motion recipes. Follow docs/PHILOSOPHY.md (§4: std functions earn their place at ≥3× token savings; never as renamed wrappers) and the owner's rule: hard parts stay exposed as rewritable code, and never trade control for fewer lines.

Another session is landing speed and load work on the unmerged branches perf-frame, perf-load, perf-instrument and perf-precision (worktrees under ~/git/_wt/loft-perf-*). Read their diffs and design around them. Do not edit or merge them.

## Phase 1: design (commit it before writing any code)
Write agent_notes/2026-11-20_v1/design/lean2.md:
1. **Inventory by function, per crate:** each block of ≥100 lines is classified as compiler-essential, library-owned recipe (move to .loft), duplicated (two paths doing one job: name both), dead (no caller from the CLI, tests or examples), or test/fixture code counted as core.
2. **The cut list:** each item gets the lines removed, the lines added (including the .loft side), the risk, and the check that proves no behaviour change (picture gate tiers, Twin, grammars, the relevant Speed/Load cases).
3. **A prediction per crate,** written before cutting, of where the total lands. If it does not reach ≤18,000, say exactly what design change would get there, and stop for review instead of cutting features.
4. **The order:** land merges in an order that keeps main green and avoids the perf branches' files until they land.

## Phase 2: implement, after the lead reads the design (the lead will answer in agent_notes/2026-11-20_v1/design/lean2_review.md; wait for that file, checking every 10 minutes)
Work in a Cambium worktree (`git cambium add -b loft/lean2 -name lean2 -ephemeral ~/git/_wt/loft-lean2`). Make one commit per cut item, each passing check, clippy, the Rust tests it touches, both grammars and the picture gate. At the end, run once: the full suite, Picture, Twin, Speed and Load under the quiet-GPU conductor, plus `tools/check.sh lines` with the ratchet set to the new total. Builds go through ~/.local/bin/agent-build; GPU work takes /tmp/loft-gpu.lock. Merge to main and push only when every gate passes and the totals match your prediction within 10% (or the deviation is explained).

Commit messages end with "Co-Authored-By: GPT (codex) <noreply@openai.com>". Final message ≤150 words: lines before and after per crate, predicted versus actual, gate results, and what remains to reach the wave-2 range.
