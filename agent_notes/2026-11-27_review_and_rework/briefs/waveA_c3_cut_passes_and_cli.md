# Lane c3-cut-passes-and-cli: Delete `fusion.rs`, `sexp.rs`, `fmt.rs`, Relocate CLI Examples & `diff.rs`, Harden `authored_compute`

Effort: high
Mode: CODE-ONLY (LOFT_NO_COMPILE=1 — DO NOT run cargo, rustc, tools/dev.sh, or tools/check.sh!)

## Mission
Execute the `loft-passes` / `loft-cli` / `loft-emit` dead-code cut portion of `agent_notes/2026-11-27_review_and_rework/BLUEPRINT.md` §2 (Wave A). Read `docs/PHILOSOPHY.md` §6 first. Cut ~1,500+ lines of dead/misplaced Rust from core crates while keeping all rollouts and CLI commands intact.

## Strict File Ownership (ONLY edit/delete/add files under these paths)
- `crates/loft-passes/`
- `crates/loft-emit/`
- `crates/loft-cli/`
- `tools/loft-verify/` (ONLY adding relocated `diff.rs` / `examples/` files and wiring `tools/loft-verify/Cargo.toml`)

## Exact Cuts & Rewrites
1. **Delete `crates/loft-passes/src/sexp.rs` (122 LOC):**
   - Check all callers (`rg sexp crates/ tools/`), remove `loft ir --sexp` or dead callers cleanly.
2. **Delete `crates/loft-passes/src/fusion.rs` (201 LOC):**
   - It performs zero pass fusion — inline only the minimal motion bundle packaging function actually called by `loft-passes` / `loft-native` / `loft-cli` into `crates/loft-passes/src/motion.rs` (saving ~120+ lines).
3. **Harden `crates/loft-passes/src/authored_compute.rs` `append_to`:**
   - Filter out `loft_ir::execution::DESCRIPTOR` if present in `self.bundle.tables`.
   - Remap `DispatchExtent::FromRowCount { table, .. }` and `DispatchExtent::Indirect { buf, .. }` using the remapped `tables` slice (not just `pass.bindings`).
   - Merge `self.bundle.resources.resets` (remapped through `tables`) into `bundle.resources.resets`.
4. **Clean `crates/loft-emit/` dead & low-hanging bloat:**
   - Delete dead header lines 1–16 of `crates/loft-emit/src/world_medium.wgsl` (currently sliced away at runtime by `emit/paths.rs:52`!) and update `emit/paths.rs` so it stops slicing off dead header lines.
   - Clean up `crates/loft-emit/src/bvh.wgsl` and `mesh.wgsl`: precompute `inv_d = 1.0 / ray.d` cleanly WITHOUT keeping a bloated dual-path fallback (handle `rd == 0.0` cleanly with standard IEEE/select guards, ~0 net lines added!) and add near-child ordering in `mesh.wgsl`.
5. **Delete `crates/loft-cli/src/fmt.rs` (5-line stub) and relocate `crates/loft-cli/examples/` (624 LOC) to `tools/loft-verify/examples/`:**
   - Move `corpus_pictures.rs`, `gate_capture.rs`, `phase4_gate.rs`, `phase5_corpus.rs`, `phase5_cpu.rs` into `tools/loft-verify/examples/` so `crates/loft-cli` contains zero test/gate example bloat.
   - Move `crates/loft-cli/src/diff.rs` (256 LOC) to `tools/loft-verify/src/diff.rs` and dispatch `loft diff` through the existing sibling `loft-verify` exec path in `crates/loft-cli/src/main.rs` (just like `loft gate` and `loft controls` already do!).
6. **Remove `loft-passes` call to deleted `loft_eval::refine` if any** (check via `rg refine crates/`).

## Rules (ZERO-COMPILE MODE)
- **DO NOT run `cargo`, `rustc`, `tools/dev.sh`, or `tools/check.sh`.**
- Read ALL affected files and callers across the workspace via `rg` before editing so every import, visibility, and signature is 100% compiler-ready.
- Commit cleanly on your branch by explicit path. Do NOT push or merge.
- Final report (<= 250 words): exact LOC removed, files deleted/moved, and any downstream integration note.
