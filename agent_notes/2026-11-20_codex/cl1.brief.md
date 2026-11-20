Effort: medium

You are a Python engineer on ~/git/cleanup_tools. Another session may be active in the main checkout: work in a git worktree (`git -C ~/git/cleanup_tools worktree add ~/git/_wt/cleanup-guard -b runtime-guard`) and never touch the main checkout's uncommitted files. Read agent_notes/2026-11-20_freeze_git_churn.md and HELP_CLEAN_SYSTEM.md first.

Why: two hard freezes on 20 Nov (20:17 and 21:44) began with a `git` child of this repo's jobs stuck mid-exec, minutes after `cold-runtime-clones --apply` and `apfs_dedupe --apply` had rewritten ~208 executable-type files under /opt/homebrew/Cellar/go, ~/Library/Python and ~/Library/Caches/ms-playwright.

1. **A single shared guard,** used by every script that clones, dedupes, compresses or otherwise rewrites files in place (apfs_dedupe, transparent_compress, the cold-runtime-clones scripts and any similar `--apply` path):
   - **(a)** A hard DENY list of runtime and toolchain roots: /opt/homebrew/Cellar, /opt/homebrew/opt, /usr/local/Cellar, ~/Library/Python, ~/.rustup, ~/.cargo, the uv python and tool dirs (`uv python dir`, `uv tool dir`), ~/Library/Caches/ms-playwright, /Library/Developer, /Applications. Also refuse any Mach-O executable or dylib anywhere, by checking magic bytes. Refusal is a typed error naming the path, with no silent skip. An explicit `--allow-runtime` override exists and prints a warning.
   - **(b)** A busy check before any `--apply`: refuse if the 1-minute load is ≥ 2× cores, or the `~/.local/bin/agent-build` slot is held, or ~/Library/Logs/FreezeSampler/STALL exists. Use sysctl and the slot file only: no ps, pgrep or lsof.
2. **Replace `git -C` loops** with one batched process, or throttle them to one git at a time.
3. **Add a test for the guard** (a deny-list path and a Mach-O file are both refused). Run the repo's suite as it already runs it.
4. Commit on branch runtime-guard with messages ending "Co-Authored-By: GPT (codex) <noreply@openai.com>". Do not merge or push: the repo owner merges. Write a note of 15 lines or fewer at ~/git/loft/agent_notes/2026-11-20_v1/lanes/cl1.md.

Final message ≤80 words.
