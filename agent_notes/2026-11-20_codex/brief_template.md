You are building Loft v1.0, a 3D meta engine, in the repo ~/git/loft. You have no other context. Read these first:
- docs/PHILOSOPHY.md
- CORE.md
- docs/spec/README.md
- every file in docs/spec/. These are the source of truth. 06_ADDENDUM.md overrides the earlier files where they differ. README.md holds three lead rulings: the DAG error is E505; motion raw blocks are `wgsl det (...)`; missing timestamps exit 6.
- agent_notes/2026-11-20_meta/ for what is already built (Phase 0 and Phase 1, in the old syntax A, in crates/, lib/ and engines/).

YOUR PHASE: __PHASE__. Its deliverable, pass/fail gates, code deltas and deletions are the "__PHASE__" row of docs/spec/05_NUMBERS_AND_BUILD_ORDER.md §2. Build that row completely, from the scientist's spec. __EXTRA__

Process:
- **Workspace.** Work in a cambium workspace: `cd ~/git/loft && git cambium add -b __BRANCH__ -name __WS__ -ephemeral ~/git/_wt/__WS__`. If it already exists, continue in it: inspect it, keep what is right, never start over.
- **Predict first.** Write predictions before building, in agent_notes/2026-11-20_meta/__NOTE__.md. Score every gate once at the end, against those predictions.
- **Builds.** Use tools/dev.sh. It respects the shared compile lock; wait on it, and never kill other processes.
- **Tests.** Run focused tests and the phase's gates. Run one full workspace test at the end.
- **Code style.** Use sum types with exhaustive dispatch. Refusals are typed and name the fix. No silent fallbacks.
- **Saving work.** Commit and push the branch after every milestone; the machine restarts often.
- **Commits.** Stage by path; never use `git add -A` outside your own new files. End messages with "Co-Authored-By: GPT (codex) <noreply@openai.com>".
- **Media.** Stills are WebP q95; clips are SVT-AV1 crf 42, saved under agent_notes/2026-11-20_meta/__NOTE__/.
- **Merging.** When every gate of your row passes, merge main into your branch, rerun the gates, merge into main, and push. If a gate fails and you cannot fix it, do not merge; write down exactly why.

Your final message (≤200 words) covers:
- every gate, predicted vs measured;
- the merged sha, or the reason you did not merge;
- what is left.
