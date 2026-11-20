Effort: medium

You are a Rust engineer on freeze_sampler (~/git/freeze_sampler), the Mac freeze recorder. Another session may be editing it: work in a git worktree (`git -C ~/git/freeze_sampler worktree add ~/git/_wt/freeze-stallmarker -b stall-marker`), and never touch the main checkout's uncommitted files.

**Goal:** make the rule "no process polling during a stall" enforceable.
1. **Write a stall marker.** When the recorder detects a process-lookup stall (`proc_stuck_s > PROC_STALL_S`, src/main.rs), atomically write ~/Library/Logs/FreezeSampler/STALL containing the onset time and the blocked PID. Remove it when the stall clears, and on clean shutdown. Remove a stale marker on start-up.
2. **Take a second stackshot** about 60 s after the first, so the next incident shows whether WindowServer, logd or launchd joined the convoy.
3. **Record the parent PID and command line of new processes at spawn time** where the recorder already watches spawns, so a blocked PID's parent can be named.
4. Build with `~/.local/bin/agent-build cargo build --release` and run the tests. Commit on branch stall-marker with messages ending "Co-Authored-By: GPT (codex) <noreply@openai.com>". Do NOT install or restart the running recorder, and do not push.
5. **Write a note** of 20 lines or fewer at ~/git/loft/agent_notes/2026-11-20_v1/lanes/fs1.md: what changed, and how to install it.

Final message ≤100 words.
