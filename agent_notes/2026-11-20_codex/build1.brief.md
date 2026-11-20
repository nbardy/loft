Effort: medium

You are a Loft build engineer. Repo ~/git/loft. Make dependency builds shared across worktrees. Measure first.

Facts (lead, 20 Nov):
- sccache shows 2 hits out of 331 Rust compiles, so every lane worktree (~/git/_wt/loft-*, each with its own target/) rebuilds wgpu, naga and every other dependency from scratch.
- run.sh exports RUSTC_WRAPPER=sccache and CARGO_INCREMENTAL=0. It now scales CARGO_BUILD_JOBS (2, 3 or 4) with free memory.
- .cambium.toml may define a cargo-deps seed for target/. Check whether it is actually applied to new workspaces: `cambium status`, and look at a fresh workspace's target/.

1. **Measure the baseline:** a fresh cambium workspace from origin/main, `tools/dev.sh check --workspace` and `build loft-cli --fast`, wall time and CPU, plus the sccache stats before and after.
2. **Find why sccache misses.** Absolute paths in the cache keys differ per worktree, which is the likely cause. Fix it with the least machinery, for example:
   - sccache's base-dir or path-normalising setting if our version supports one;
   - a consistent `--remap-path-prefix` in RUSTFLAGS;
   - the cambium cargo-deps seed, so dependencies arrive pre-built;
   - a shared CARGO_TARGET_DIR for dependencies only, if that is safe with concurrent cargo.

   Pick the option that stays correct with 5 concurrent lanes.
3. **Apply it** in tools/dev.sh and .cambium.toml. If run.sh needs it, add at most one export line to agent_notes/2026-11-20_codex/run.sh, replacing the file atomically (write a temp file, then os.replace in Python), because running conductors call it. Re-measure on a second fresh workspace. Target: dependency rebuilds take ≤20% of the baseline wall time.
4. **Write agent_notes/2026-11-20_v1/lanes/build1.md** (≤30 lines, with the numbers). This is tooling only: merge it into main yourself if the workspace still builds and its tests pass. Commit messages end with "Co-Authored-By: GPT (codex) <noreply@openai.com>". Final message ≤100 words.
