Effort: high

You are a Loft engineer making compilation ahead-of-time and cached. Repo ~/git/loft (main). Measure first, then build.

Today every build re-parses and re-checks all of lib/*.loft (607 lines; crates/loft-cli/src/main.rs:10), and Metal compiles each pipeline at about 70 ms on first use.

1. **Measure:** per scene (capsules, hermit-20, shrine), the build-time breakdown (parse, check and lower for lib vs the scene; naga emit), plus cold and warm device load with pipeline compile time.
2. **Std AOT.** Serialise the parsed, checked std IR to a content-hashed cache artifact (for example target/loft-cache/std-<hash>.bin), loaded instead of re-parsing whenever lib/ is unchanged. The same mechanism applies to any imported module: a component compiles once and is reused by every scene that uses it. Invalidate by content hash plus the compiler version. No stale-cache silent fallbacks: a typed cache-miss reason must be visible in `loft build --json`.
3. **Pipeline cache** (spec flag PIPELINE_CACHE; OPEN.md "Performance carry-overs"):
   - use wgpu's PipelineCache where the backend supports it;
   - add warm-up during loading;
   - make cache files atomic, keyed by adapter and driver;
   - on Metal, where wgpu's cache may be a no-op, say so and measure what actually helps (concurrent pipeline creation, warm-up ordering).
4. **Report the before and after numbers.** Pixels must stay bit-identical and the gates unchanged.

Use a cambium workspace on branch loft/stdaot1 from origin/main. Iterate `tools/dev.sh check` until it compiles; at most 2 full test or GPU reruns. Push the branch. Write agent_notes/2026-11-20_v1/lanes/stdaot1.md. Commit messages end with "Co-Authored-By: GPT (codex) <noreply@openai.com>". Final message ≤120 words.

Std rule: a lib function must replace a recurring, sufficiently complicated block of ≥3× its call's tokens. NEVER add renamed wrappers for standard patterns (map, await, filter, fold) or one-or-two-call helpers. Code lives in the stdlib/ tree (design/std1.md); forking is copy-paste.
Base: start from origin/main AFTER the std1a restructure (stdlib/ tree), not the old flat lib/.
