Effort: high

You are the final integrator for 20 Nov 2026. The owner says: wrap up, merge, commit and push everything. Work carefully, one repo at a time. Use `~/.local/bin/agent-build` for builds and tests. NEVER run ps, pgrep, lsof or spindump while ~/Library/Logs/FreezeSampler/STALL exists. Do not install binaries or launch daemons (that needs the owner's sudo). Leave the exact install commands instead.

1. **freeze_sampler** (~/git/freeze_sampler, default branch master; lanes fs1 and fs2 in the worktree ~/git/_wt/freeze-stallmarker; notes in ~/git/loft/agent_notes/2026-11-20_v1/lanes/fs1.md and fs2.md):
   - review the diffs;
   - build and test (`agent-build cargo build --release`, `agent-build cargo test`);
   - merge into master. Another session may have uncommitted edits in the main checkout: do the merge in the worktree, or with `git -C` without touching the working tree, and never overwrite uncommitted files;
   - push origin master;
   - remove the worktree with `git worktree remove` only if it is clean and merged.
2. **cambium** (~/git/_wt/cambium-v010, branch release/v0.10; commits fb755e9 (installed), d5b56b9 (cam1 seeds) and cam2 (no lsof); notes in lanes/cam1.md and cam2.md):
   - review;
   - run `agent-build go test ./...` and check the exit code;
   - push branch release/v0.10 to origin. Do not touch ~/git/cambium main or its uncommitted edits. Do not tag a release.
   - Leave the install command for the owner: `go build -o ~/.local/bin/cambium ./cmd/cambium` with a backup of the old binary.
3. **loft** (~/git/loft):
   - confirm ix4's result (out/meta/ix4/MERGED and BLOCKED);
   - update docs/V1_5_GOALS.md statuses and agent_notes/2026-11-20_v1/HANDOFF.md to the final state (what merged today, what is still open);
   - commit by path and push main. Never stage tools/crab_*.py or tools/dev.sh (other sessions' edits).
4. **Write ~/git/loft/agent_notes/2026-11-20_v1/WRAPUP.md** (40 lines or fewer): per repo, the pushed SHAs, the tests run with their results, what is still open, and the owner's install steps:
   - the freeze-helper launchd plus the Full Disk Access approval;
   - the cambium binary.

   Commit and push it.

Commit messages end with "Co-Authored-By: GPT (codex) <noreply@openai.com>". If a merge or test fails, do not force it: record it in WRAPUP.md as open. Final message ≤150 words.
