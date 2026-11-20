# pf1b · Reduce loop overhead on long SDF marches

You are a Loft engineer. Repo ~/git/loft, main at or after the PF1 report. You have no other context.

Read CORE.md, docs/PHILOSOPHY.md, docs/spec/README.md, agent_notes/2026-11-20_v1/OPEN.md and agent_notes/2026-11-20_v1/design/pf1.md, including the ordered image comparisons.

## Rules

- Use a Cambium workspace with `git cambium add -b loft/pf1b -name loft-pf1b -ephemeral ~/git/_wt/loft-pf1b origin/main`. Push the lane branch. The integrator merges main.
- Write the prediction first. Iterate `CARGO_BUILD_JOBS=2 tools/dev.sh check CRATE` on each edited crate until it compiles cleanly. Compile errors are ordinary iterations. Allow at most two full-test reruns and two GPU reruns. If the cause fails to narrow after two fixes, escalate with evidence.
- Hold `tools/gpu_lock.sh` for every GPU command, including GPU tests. Use the root workspace, lab profile, two build jobs and the pinned toolchain.
- Keep kind dispatch exhaustive, parse inputs at the boundary and preserve bit-identical pixels. Preserve the author’s camera, sampling rule, hit policy, kind/row IDs and reference images. Any subsequent look change needs explicit owner approval.
- Record matched main/candidate pass and frame p50/p95, triangle controls, per-frame ticks, command counts and actual decoded comparisons in `agent_notes/2026-11-20_v1/lanes/pf1b.md`. Keep evidence in the primary checkout’s `out/`. Commit by path. End commit messages with `Co-Authored-By: GPT (codex) <noreply@openai.com>`.
- Owned files are the traced visibility emitter and diagnostic step instrumentation in crates/loft-passes/src/emit.rs, its existing emitter modules and a focused existing renderer fixture if needed and the lane note.

## Task

Keep the same checked shader module, row L, sample sequence, hit tolerance and 192-step cap. PF1 grouped four consecutive march bodies under one outer loop, testing hit and far exit after every sample. On Shrine that saved 0.384 ms of Vis and 0.420 ms frame p95. Matched ascending and descending row-order images agreed at all three cameras. On Hermit the same prototype saved median Vis time but made frame p95 2.046 ms worse across the ABBA blocks. The source-level loop shape is worth pursuing only with a passing short-march control.

Start from the measured split in step counts. Keep the original scalar path for the first 24 iterations and group the remaining 168 steps four at a time. At t=0, 81.5% of Shrine’s successful step iterations and 3.6% of Hermit’s lie beyond step 24. This predicts about 0.31 ms of Shrine Vis and 0.01 ms of Hermit Vis plus Shadow, before loop transition overhead. The estimate omits misses and hidden fragments. Use one emitted step body with an explicit exit reason. Preserve the diagnostic step count, hit distance bits, both cameras and all early-exit boundaries. Leave the check policy, mathematical expression order and public scene language intact. If the short prefix still regresses frame p95, localize the tail before proposing a broader lowering.

## Score

At 1280 × 800 with 30 warmups, target ≥0.25 ms lower Shrine Vis p50 and ≤3.80 ms frame p95 over 100 close t=0 frames. Hermit count=20 must stay within the 5.60 ms p95 Speed budget over the 300-frame orbit and within 2% of a matched control p95. Use A/B/B/A with identical camera inputs on one queue. Preserve command and pipeline counts. Compare lossless beauty, depth, ID and SDF-step AOV at 0/4/9 s, and inspect an early hit, an interval exit and a last-iteration hit or exhaustion. For Shrine require zero differences under both fixed row orders, then time its natural cull order. Report this lane alone before combining it with the sun-map lane, since both can affect Hermit’s shadow pass.

Final message ≤120 words.
