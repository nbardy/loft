# Lean Rust: wire 2, the decls, the fills and the prelude (19 Nov 2026)

Plan: `docs/proposals/2026-11-19_lean_rust_loft.md` §3.2 (pinned here as built), §1.4–§1.7, §1.9. Follows
`agent_notes/2026-11-19_lean_rust_driver.md` ("What blocks the cutover" 1–4). Branch `feat/lr-wire2` from `2726371`.

## Brief (written before the edits)

- **Blocker.** 0 of 295 corpus bundles can be drawn and 214 are refused by Naga. (a) The wire has no layout for
  Draw, Pass, Medium, Camera, Check or Solve decls. (b) A `Kernel` decl carries no fill body, so every table is
  `Init::Zero`. (c) Library raw WGSL reads globals only the TS host declares (`loft_detail` 204, `view_dir` 2,
  `loft_cast` 2, `loft_passtex_*` 3, `loft_compute_*` 2, `soft_box` 1).
- **Owner of this lane:** the recorder (`src/record/*`), loft-ir `wire.rs`, decode and print, and emit's prelude.
  Not mine: loft-core emit/Naga call sites, f16 gating and loft-draw (feat/lr-speed); `solve.rs` (feat/lr-solve).
- **Target.** Each decl kind is one variant of loft-ir's per-kind decl sum type and of the TS writer, pinned in
  §3.2. Rust print = TS print on all 125 example files. One named prelude owned by emit; a global it does not name
  is a typed refusal naming the fix.
- **Expected.** Built rises from 27 toward the R0 build count (R0 built 238 of 295), less what the refusal names.
  Drawn stays near 0 until the TS library writes hit/place/paint functions (lane 3.1's TS half) and loft-draw runs
  on loft-gpu (feat/lr-speed). Parity is reported, not expected.
- **Cheapest check.** `tools/dev.sh test loft-ir`, the corpus print-equality test, `tools/parity/run.ts rust`.
- **Stop rule.** Two failed fixes in one subsystem: stop and write the failure class.
- **Claim boundary.** Counts on one machine (M4, loaded), on this branch's commit.
