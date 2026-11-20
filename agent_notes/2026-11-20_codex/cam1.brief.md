Effort: high

You are a Go engineer on cambium, a git-worktree manager with CoW environment seeding. Work ONLY in ~/git/_wt/cambium-v010 (branch release/v0.10, HEAD at or after fb755e9). Do NOT touch ~/git/cambium (main) or its uncommitted edits, which belong to another session.

**The problem (measured 20 Nov 2026 in ~/git/loft):** .cambium.toml declares `[[path]] path="target" policy="seed" seed="cargo-deps"`. Every cargo-deps layer under ~/git/loft/.git/cambium/layers/*/metadata.json from today has no `payload/`, and one claimed present=true with no payload (the bug fb755e9 fixed). So every `git cambium add` workspace started with a cold target/ and rebuilt wgpu, naga and every other dependency from scratch, and nothing reported it. A separate Loft lane (build1) is warming Loft's seed. Your job is to make cambium itself robust.

1. **Report cold seeds loudly.** `cambium add` and `cambium status` (text and --json) must report when a seed rule cloned nothing, with a typed reason: no source build for this lock, capture failed, toolchain mismatch, and so on. No silent cold workspaces. Find where seed capture and apply happen (internal/native/layers.go and its callers) and why capture produced no payload.
2. **`cambium seed warm [PATH]`:** for a cargo-deps seed, build third-party dependencies for the current Cargo.lock and toolchain into the seed layer, without depending on the primary checkout's state. For example, run a dependency-only build in a scratch directory with `cargo build` of the workspace's dependencies (check how cargo-chef or the `--dependencies` approach works, and pick the simplest correct method), then capture. Key the layer by lock, toolchain, target triple and profile.
3. **Harvest on remove:** when `cambium remove` runs on a workspace whose target/ holds dependency artifacts for a lock or toolchain that has no warm seed, publish them into the seed layer (dependencies only, never workspace crates), under the existing layer lock. The swarm then warms itself.
4. **A Rust default:** when a repo has Cargo.lock and the filesystem supports CoW, and no explicit [[path]] rule covers target, default to seeding cargo-deps for target. An explicit `policy = "skip"` (room-runners' choice) still wins. Document it in the README.
5. **Regression tests:**
   - a cold seed is reported;
   - a warmed seed is cloned into a new workspace and cargo does not rebuild the dependencies (check with cargo's freshness: `cargo build -v` reports "Fresh");
   - harvest-on-remove publishes;
   - an explicit skip is respected.
6. **Verify:** run `go test ./...` (full, with the exit code checked) and build `/tmp/cambium-seedfix`. Then prove it end to end on ~/git/loft: `/tmp/cambium-seedfix seed warm target` in ~/git/loft, then `git -C ~/git/loft cambium add` a scratch workspace using the new binary (put /tmp first on PATH for that one command). Show that its `tools/dev.sh check --workspace` compiles no third-party crates, then remove the workspace safely. If the existing Loft layers are corrupt, quarantine them (rename aside, never delete) and re-warm.

Commit on release/v0.10, staging by path. Messages end with "Co-Authored-By: GPT (codex) <noreply@openai.com>". Do NOT push, and do NOT replace ~/.local/bin/cambium: the lead installs it after review. Write a ≤30-line note at ~/git/loft/agent_notes/2026-11-20_v1/lanes/cam1.md with the numbers: the cold vs warm workspace check time. Final message ≤120 words: the commits, test results, binary path and end-to-end timings.

**Resume note (after 15:36 restart):** Commit `d5b56b9` is already on `release/v0.10` inside `~/git/_wt/cambium-v010`. Continue from `d5b56b9` to finish verification (`go test ./...`, `/tmp/cambium-seedfix`, end-to-end Loft warm-seed proof) and write `cam1.md`.
