You are Loft's chief engineer running a lean-down audit (documents only; you may run line counters and read code). Repo ~/git/loft (main).

Read docs/legacy/proposals/2026-11-19_lean_rust_loft.md (the old lean Rust plan: ten core types replacing ~60, one Error/Verdict/Medium/Bundle, a 9.8k total budget with per-lane ceilings, 'compose once'), docs/proposals/2026-11-20_meta_engine_plan.md (L0 ≤1,000, L1 ≤1,500, L2 ~10k), docs/PHILOSOPHY.md, and the current crates (~32k lines: passes 15.6k, cli 5.4k, std 4.3k, parse 2.7k, runner 1.6k, plan/format/ir/abi 2.7k).

Note: lanes x2a (execution path) and x2b (motion twin) are editing loft-runner/loft-cli/loft-passes right now; audit main as it is and plan around them.

Write agent_notes/2026-11-20_v1/design/lean1.md:
1. Per crate: lines, what it owns, duplicate representations and parallel paths (name files and functions), dead code, and kind/null checks inside handlers that should be sum-type dispatch.
2. The 'ten types' map for the new architecture: which types/enums should be the single representation of each concept, and which ones they replace.
3. Per-crate line ceilings (target total, predicted 15–18k; argue yours) and a gate that enforces them (tools/check.sh), measured by scc or tokei code lines.
4. At most 4 Sol lane briefs to reach the ceilings with no behaviour change (bit-identical pixels, identical gate scores), disjoint files, sequenced after x2a/x2b where they overlap: agent_notes/2026-11-20_codex/lean1a..d.brief.md in the shape of w1.brief.md; list ids in agent_notes/2026-11-20_codex/lean1.lanes.

Commit by path to main and push; messages end with "Co-Authored-By: GPT (codex) <noreply@openai.com>". Final message ≤150 words.