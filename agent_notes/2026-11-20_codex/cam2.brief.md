Effort: high

You are a Go engineer on cambium. Work in ~/git/_wt/cambium-v010 (branch release/v0.10, at or after d5b56b9). Do not touch ~/git/cambium (main).

**Why:** on 20 Nov the Mac hard-froze twice, at 15:3x and 20:17. Each time a kernel process-lookup lock convoy formed around a short-lived `git` process, and every tool that asks the kernel about other processes (ps, pgrep, lsof, proc_pidinfo) joined the queue. Cambium's in-use check runs `lsof -p <pid> -d cwd` on processes (rg 'lsof|lsofCwds|cwdReader|inuse' internal/), and its test suite runs it in parallel while spawning many `git` children.

1. **Remove every `lsof` and per-process cwd lookup** from cambium. Decide "in use" from what cambium already owns: the workspace receipt and lock files, recorded owners, and open-handle markers that cambium-managed tools write. If something truly needs another process's cwd, make it opt-in behind a flag and off by default. When in-use cannot be determined, return a typed "unknown in-use" reason: no silent fallback.
2. **Tests:** cap parallel subprocess spawning (no unbounded `t.Parallel` with git children). Add one test proving the in-use check spawns no `lsof`.
3. **Verify:** run `~/.local/bin/agent-build go test ./...` (full, with the exit code checked) and build /tmp/cambium-nolsof. Commit by path on release/v0.10 with messages ending "Co-Authored-By: GPT (codex) <noreply@openai.com>". Do NOT push or install.
4. **Write a note** of 20 lines or fewer at ~/git/loft/agent_notes/2026-11-20_v1/lanes/cam2.md.

Final message ≤100 words.
