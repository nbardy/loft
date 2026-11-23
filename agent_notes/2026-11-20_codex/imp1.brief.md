# imp1: fork and improve crab seeds, recording every step as a pairwise Improve record
Effort: xhigh

Lead: you own the design and the implementation. Start only after div2 has finished: wait until agent_notes/2026-11-20_codex/div2.done exists (check every 5 minutes; do not inspect processes). Then read div2.final.md and its sheet. Repos: ~/git/creative (worktree from div2's `loft/seed-render`, new branch `loft/crab-improve-pairs`) and ~/git/loft. Never touch other sessions' uncommitted files. Never start a paid training run, and never call a paid judge or model API: you are the critic and the editor.

## Goal
The owner wants the weaker seeds forked and improved, with every improvement step tracked as before/after program pairs plus renders, for the pairwise image-improvement dataset.

## Which seeds
At least 6:
- fiddler and porcelain (the lead flagged both in agent_notes/2026-11-20_v1/APPROVALS.md: the fiddler's crusher reads as a blob, and the porcelain crab is low contrast);
- the photoreal-chitin seeds from div2 (target: a natural-history photo of a live crab on wet sand, dull rough chitin, not shiny);
- the 2 seeds with the weakest look on div2's sheet, judged by you, with the reason written down.

## Loop per seed: 3–4 rounds
1. **Critique** the current render (frame zero plus one motion frame) against the named axes of the existing Improve scorecard (see training/loft_envs/seed_programs/crab-improve-v1 and training/loft_reward_v2 for the axis names; use those names, do not invent new ones). Name the 1–2 axes this round targets and what must be preserved.
2. **Fork:** write 3 candidate edits of the current program, each a different idea for the same critique. Edits must stay within one seed program; stdlib changes follow the std ≥3× rule.
3. Render all candidates under the same camera and times. Each must pass check, strict lint, both grammars, the div1 shape gate against the other seeds, and the clip budget (≤2,000 ms CPU, ≤50 ms GPU at 512²).
4. Rank the candidates and the parent by eye against the critique, writing one sentence per ranking. The best one becomes the parent for the next round only if it beats the current parent. Stop a chain early if two rounds bring no improvement.

## Records: keep everything, losers included
Extend the crab-improve-v1 record format into a new frozen export, training/loft_envs/seed_programs/crab-improve-pairs-v1/:
- **Per candidate:** id, parent id and SHA256 (`program_parents` filled, unlike v1), exact code bytes, critique text, targeted axes, preservation list, renders (lossless frame-zero beauty plus the motion frame, same times as the parent), gate results, timings, `origin: "model:gpt-6.2-astra"`, and the round.
- **pairs.jsonl:** one row per ordered comparison you made (child vs parent, and child vs sibling): `{chosen, rejected, axes, reason, margin: clear|slight|tie}`. Ties are recorded and not dropped.
- **status:** `proposed`. The lead reviews and then sets `accepted` or `rejected` per pair; until then nothing is admitted to the trainer or the registry.
- Follow the v1 freeze and verify pattern: a freeze.py with --verify, SHA256SUMS and a manifest.
- Keep eval separation. If a seed is in an eval split, its chain is eval-only.

## Deliver
- One sheet per chain at ~/git/loft/out/meta/gallery/imp1/<seed>.webp: rounds left to right, all 3 candidates per round, the winner outlined, and the critique as a caption.
- An overview at ~/git/loft/out/meta/gallery/imp1_overview.webp: the start versus the final render for each chain.
- Commits on `loft/crab-improve-pairs`. The Creative remote cannot take its history (a 477 MB mbox), so write a verified bundle to ~/git/loft/out/meta/imp1/.
- Builds and tests go one at a time through ~/.local/bin/agent-build; GPU renders take /tmp/loft-gpu.lock.
- Commit messages end with "Co-Authored-By: GPT (codex) <noreply@openai.com>".
- Final message ≤150 words: the chains (seed, rounds, final verdict), the pair counts (chosen/tie), the sheet paths, and anything you could not improve and why.
