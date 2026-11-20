Effort: medium

You are a Rust engineer on freeze_sampler (~/git/freeze_sampler), the Mac freeze recorder. Another session may be editing it: work in a git worktree (`git -C ~/git/freeze_sampler worktree add ~/git/_wt/freeze-stallmarker -b stall-marker`), and never touch the main checkout's uncommitted files.

**Goal:** make the rule "no process polling during a stall" enforceable.
1. **Write a stall marker.** When the recorder detects a process-lookup stall (`proc_stuck_s > PROC_STALL_S`, src/main.rs), atomically write ~/Library/Logs/FreezeSampler/STALL containing the onset time and the blocked PID. Remove it when the stall clears, and on clean shutdown. Remove a stale marker on start-up.
2. **Take a second stackshot** about 60 s after the first, so the next incident shows whether WindowServer, logd or launchd joined the convoy.
3. **Record the parent PID and command line of new processes at spawn time** where the recorder already watches spawns, so a blocked PID's parent can be named.
4. Build with `~/.local/bin/agent-build cargo build --release` and run the tests. Commit on branch stall-marker with messages ending "Co-Authored-By: GPT (codex) <noreply@openai.com>". Do NOT install or restart the running recorder, and do not push.
5. **Write a note** of 20 lines or fewer at ~/git/loft/agent_notes/2026-11-20_v1/lanes/fs1.md: what changed, and how to install it.

Final message ≤100 words.

**Also make the recorder itself safe during a stall** (lead, 20 Nov). Today its per-PID `identity` lookup was one of the 11 convoy waiters, and an `lsof` and a `spindump` joined after onset.
1. **At most one process-info lookup in flight.** If a lookup has not returned (stuck in an uninterruptible kernel wait), never start another. Keep sampling the lock-free counters (memory, swap, compressor, load, GPU) and mark process fields stale with a typed reason.
2. **Back off under stress, do not ramp up.** Replace "process scan every second while alerting" with slower process scans when load > 2× cores, spawn rate is high or pressure ≥ warn, and none at all during a detected stall.
3. **No `lsof` anywhere in the recorder.** At most one spindump or stackshot per incident at a time; the second stackshot only after the first finishes.
4. **Prefer one bulk process-list read** over per-PID reads. Read per-PID detail only for the top few by footprint or CPU, and never for processes younger than about 1 s (the convoy formed around a short-lived git).
5. **Keep the cost claim honest:** re-measure the idle CPU and RAM after the changes, and update the README "Cost" section if it moved.

**Refinements (lead, 20 Nov; supersede the points above where they differ):**
- The single in-flight lookup that gets stuck IS the stall detector. Keep it, and never add a second.
- During a stall, capture **raw kernel stackshot buffers only** (one at onset and one about 60 s later; the 20:18:32 buffer was captured fine mid-stall). Do NOT symbolicate during the stall: spindump's symbolication step is what blocked. Symbolicate after the stall clears or on the next boot, then attach the result to the crash review.
- **Spawn lineage:** prefer `eslogger exec fork exit` (Endpoint Security, macOS 13+; needs root and Full Disk Access), which streams parent, PID and argv with no polling. If it is unavailable, fall back to the existing scan, and record the fallback reason in the samples as a typed reason, not silently.
- Say in the lane note which of these claims you verified on this machine and which are inference.
