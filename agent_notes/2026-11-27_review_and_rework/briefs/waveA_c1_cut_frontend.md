# Lane c1-cut-frontend: Cut Dead Frontend Slop, Creature ABI & Hardcoded Stdlib Whitelist

Effort: high
Mode: CODE-ONLY (LOFT_NO_COMPILE=1 — DO NOT run cargo, rustc, tools/dev.sh, or tools/check.sh!)

## Mission
Execute the frontend/ABI portion of `agent_notes/2026-11-27_review_and_rework/BLUEPRINT.md` §2 (Wave A). Read `docs/PHILOSOPHY.md` §6 first. Cut ~2,000+ lines of dead/misplaced code cleanly while keeping 100% of existing `.loft` rollouts (`out/spark_pairs/`, `corpus/`, `scenes/`, `stdlib/`, `engines/gates.json`) valid and type-sound.

## Strict File Ownership (ONLY edit/delete/add files under these paths)
- `crates/loft-abi/`
- `crates/loft-ir/`
- `crates/loft-builtins/`
- `crates/loft-parse/`
- `crates/loft-check/`
- `crates/loft-eval/src/refine.rs` (delete) and its call-site/mod decl in `crates/loft-eval/src/lib.rs` (ONLY the `refine` line!)
- `tools/loft-verify/` (ONLY if relocating `path_twin.rs` / `bsdf.rs` helpers into a new file `tools/loft-verify/src/path_twin.rs` without editing `suite.rs`/`controls.rs`)

## Exact Cuts & Rewrites
1. **Delete `crates/loft-parse/src/cst.rs` (352 LOC):**
   - `parser.rs` builds a CST tree only to flatten it to `Vec<Lex>` and discard it!
   - Replace with a lean flat lexer (`crates/loft-parse/src/lex.rs`, ~100 LOC) producing `Vec<Lex>` directly with `(kind, text, span)` so `parser.rs`, `loft context`, and `loft lint` keep working cleanly (keep whatever minimal `Token` / `lex` public API `loft-cli/src/{context,lint,fork}.rs` calls — read those 3 files upfront to preserve their exact call signatures!).
2. **Delete dead AST, checker, builtins, and IR items:**
   - Remove `Item::Unit`, `KindDecl.fns`, `Stmt::Assign.op`, `TyExpr::Generic`, `NumSuffix::Deg` from `loft-parse/src/ast.rs` and every dead arm in `parser.rs`, `loft-check`.
   - Remove unused `WgslLowering::{Prefix, Infix}` from `loft-builtins/src/lib.rs`.
   - Remove duplicate `MotionMath` name list from `loft-check/src/references.rs` — read directly from `loft_builtins::catalog()`.
   - Remove duplicate `bind` + `self.expr(&sc.init)` at `loft-check/src/check.rs:359-366`.
   - Remove unused `Op::Intrinsic`, `IrModule`, `PassRegistry` from `crates/loft-ir/src/bundle.rs` (verify zero callers across workspace first!).
   - Delete `crates/loft-check/src/kernel_units.rs` (97 LOC — fake kernel-only unit checker) and remove its hooks in `check.rs` / `prog.rs` (keep unit syntax literals like `0.38m`, `40deg`, `24s` lowering to scaled `f32` in `parser.rs` so zero `.loft` files break!).
   - Delete `crates/loft-eval/src/refine.rs` (310 LOC — broken divisor checker) and remove `pub mod refine;` from `loft-eval/src/lib.rs` (check `loft-passes` call sites and note any 1-line removal needed at integration!).
3. **Strip creature/game structs out of `crates/loft-abi/src/lib.rs`:**
   - Verify with `rg` across the entire repo whether `BodyRow`, `ChainRow`, `SegmentRow`, `CreatureHeader`, `FootPin`, `ManipState`, `TrailNode`, `PoseJoint` have ANY callers outside `crates/loft-abi/tests/golden.rs`. Delete all unused creature structs from `loft-abi/src/lib.rs` (and update `golden.rs` to test only generic wire records: `BvhNode`, `LeafRef`, `VisRec`, `VerdictRecord`, `MeshRow`, `AdvanceArgs`, `det`).
4. **Replace hardcoded 24-file stdlib whitelist in `crates/loft-check/src/source/scope.rs`:**
   - Read `crates/loft-check/src/source.rs` and `scope.rs` completely.
   - Replace the hardcoded 24-file `STOCK_PATHS` array (`scope.rs:54-82`) and hardcoded migration filenames (`bead_observation.loft`, `trace_coverage.loft`) with general directory/module discovery under `stdlib/`, while preserving the exact `use stdlib::...` and implicit `core.loft` resolution behavior required by existing rollouts!
5. **Move inline `#[cfg(test)]` suites out of `crates/*/src/` into `tests/`:**
   - Move large inline test modules in `crates/loft-check/src/check.rs`, `crates/loft-abi/src/det.rs`, etc. into their respective `tests/` directories so `tools/lines.py` reflects pure non-test core LOC.

## Rules (ZERO-COMPILE MODE)
- **DO NOT run `cargo`, `rustc`, `tools/dev.sh`, or `tools/check.sh`.**
- Read ALL call sites across the whole workspace with `rg` before deleting or renaming any `pub` item so downstream crates (`loft-passes`, `loft-emit`, `loft-eval`, `loft-cli`) stay 100% type-compatible (or document any exact 1-line downstream call-site cleanup in your final report for the integration pass).
- Commit cleanly on your branch by explicit path. Do NOT push or merge.
- Final report (<= 250 words): exact LOC removed, every deleted file/symbol, and any downstream integration line needed.
