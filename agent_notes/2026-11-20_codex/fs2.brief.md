Effort: high

You are a Rust systems engineer on freeze_sampler (~/git/freeze_sampler). Build on lane fs1's branch `stall-marker` in its worktree ~/git/_wt/freeze-stallmarker. Read ~/git/loft/agent_notes/2026-11-20_v1/lanes/fs1.md first. Never touch the main checkout's uncommitted files.

**Goal: a cheap, better sysdiagnose for freezes.** Capture in seconds, a few MB, mostly from rings armed BEFORE the freeze, and never call process-table APIs during a stall.

**Theory first.** Before coding, write docs/incident_capture.md with what each piece costs (CPU, RAM, disk/day, capture time) and what a run will show. Measure against it at the end.

Add a tiny root helper, `freeze-helper` (new binary in the same crate, a launchd LaunchDaemon plist in ./launchd/, with install and uninstall scripts the OWNER runs with sudo; do not install it yourself):
1. **Always on:**
   - (a) **Spawn lineage:** run `eslogger exec fork exit` (Endpoint Security, macOS 13+; needs root and Full Disk Access) into a bounded ring file of ~20 MB, with timestamp, pid, ppid, executable and argv. If eslogger is unavailable, record a typed reason (no silent fallback).
   - (b) **Log tail:** a bounded rolling tail of the unified log, filtered to errors, faults, watchdog and hang lines. Measure its cost; if it costs more than about 0.1% CPU, make it opt-in.
2. **On stall** (`~/Library/Logs/FreezeSampler/STALL` appears; watch it with kqueue, not polling):
   - a raw kernel stackshot buffer;
   - `tailspin save` of the always-on kernel trace (check `tailspin info` first);
   - a second raw stackshot after 60 s.

   NO ps, pgrep, lsof, proc_pidinfo or symbolication during the stall. For the stackshot, verify whether the private stackshot interface (stackshot_capture_with_config, the facility spindump uses) is callable. If it is not, use spindump's raw-capture mode, writing the buffer with no symbolication. Say which one works, as verified on this Mac.
3. **After recovery or the next boot:**
   - symbolicate the raw buffers (spindump -i, tailspin decode);
   - cut the lineage ring around the blocked PID (parent chain);
   - write one incident bundle `crashes/<id>/` with stacks.txt, lineage.json, counters (from the sampler), log tail and a summary;
   - link it into the existing `report` and crash-review flow.
4. **Tests:** one end-to-end dry run using a synthetic STALL marker (create it, check the bundle appears in under 10 s and is under 20 MB, remove it). Build with `~/.local/bin/agent-build cargo build --release` and run the tests.
5. **Measure** the idle cost of the helper (CPU and RSS over 5 min) and update the README Cost section honestly.

Commit on stall-marker (or a new branch incident-capture on top of it). Messages end with "Co-Authored-By: GPT (codex) <noreply@openai.com>". Do not push or install. Write a note of 30 lines or fewer at ~/git/loft/agent_notes/2026-11-20_v1/lanes/fs2.md: what works (verified) versus inferred, the measured costs, and the exact owner install steps (sudo plus the Full Disk Access approval). Final message ≤120 words.
