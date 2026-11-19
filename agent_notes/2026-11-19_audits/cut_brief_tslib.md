# Audit: how to cut Loft's code in half (read-only; write one file)

Repo ~/git/loft. Read CORE.md (goal, priorities) and docs/proposals/2026-11-19_lean_rust_loft.md §1-§3 (the lean
plan: ~10 unified types, one path). Then read the CODE in your area exhaustively: src/geom, src/map, src/soft, src/spacetime, src/style, src/draw, src/lib, src/relief, and examples/ (how the library is used).

Context: the TS engine (src/compile, src/host, src/bake, src/frame, parts of src/draw and src/style that lower to
WGSL) will be deleted when the Rust path reaches picture parity; TS keeps only the recorder (src/record) and the
library. rust/crates/loft-loader is deleted at that cutover too. Count it as going, but say what in it the new code
still lacks.

Find every way to cut lines without losing a capability an author or the GPU needs:
- duplication (functions, types, near-copies, per-kind code that one sum type plus one dispatcher would replace);
- a cleaner data model (fewer, more general types; one representation where there are several);
- dead or unreachable code, unused exports, compatibility shims, defensive checks that typed boundaries make
  unnecessary;
- over-general machinery used once;
- library names that buy no capability (CORE.md: a new name must buy a capability);
- tests that catch no nameable bug.

For each cut give: the files and line ranges, the lines removed (measured with wc/scc, not guessed), what replaces
it (show the new type or function signature), the risk, and whether it can land now or waits for the cutover. Rank
by lines saved per unit of risk. End with the total: current lines, lines after all cuts, and a phased order.

Write ~/git/loft/agent_notes/2026-11-19_audits/cut_tslib.md (2,500-5,000 words, file:line evidence). Edit nothing
else. Final message: 150-word summary with the totals.
