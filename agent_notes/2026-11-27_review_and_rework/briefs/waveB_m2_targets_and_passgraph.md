# Lane m2-targets-and-passgraph: Real `target` Declarations & Arbitrary Multi-Pass Render Graphs

Effort: high
Mode: CODE-ONLY (LOFT_NO_COMPILE=1 — DO NOT run cargo, rustc, tools/dev.sh, or tools/check.sh!)

## Mission
Execute Milestone M2 of `agent_notes/2026-11-27_review_and_rework/BLUEPRINT.md` (Wave B). Read `docs/PHILOSOPHY.md` §6 first. Turn Loft's schedule into a genuinely arbitrary, composable GPU render + compute pass graph so users and `stdlib/` can author G-buffers, cascades, bloom, TAA, and fluid+render pipelines purely in `.loft` + WGSL.

## Strict File Ownership (ONLY edit/add files under these paths)
- `crates/loft-parse/src/parser.rs`, `ast.rs` (ONLY `target_decl` support — currently rejected as `E405` at `parser.rs:625-637`)
- `crates/loft-check/src/check.rs`, `prog.rs` (register `Item::Target` declarations in `Prog` and check target formats/extents/msaa)
- `crates/loft-passes/src/frame_pass.rs`
- `crates/loft-passes/src/pipeline/schedule.rs`
- `crates/loft-passes/src/bundle.rs`
- `crates/loft-passes/src/program.rs`
- `crates/loft-passes/tests/targets.rs` (new test)

## Exact Implementation
1. **Parse & check `target Name: Format [W x H | view [* scale]] [msaa N]`:**
   - Already in `grammar/loft.lark:30`; remove the `E405` rejection in `parser.rs:625-637` and produce `Item::Target(TargetDecl)`.
   - Support formats matching `loft_ir::ImageFormat`: `rg32uint`, `rgba16f` (`rgba16float`), `rgba32f` (`rgba32float`), `depth32f` (`depth32float`), plus MSAA `1 | 4`, and extent `Fixed(w, h)` or `View(scale)`.
2. **Generalize `FramePass` (`crates/loft-passes/src/frame_pass.rs`):**
   - Lift the 13-name `FRAME_STORES` restriction (`frame_pass.rs:18-19`) and 9-name `FrameView::named` ceiling (`program.rs:149-163`): a `pass ... reads [...] writes [...]` can now read/write ANY declared `target` OR `table` alongside existing stock frame stores!
   - Allow `draw("shader.wgsl", ...)` inside a `pass` to target ANY declared `target` (color + optional depth), not only `Swapchain`, and accept optional raster state overrides (`cull: none|front|back`, `blend: opaque|additive|alpha`) lowering cleanly into `Execution` v3 `RasterState`!
3. **Preserve 100% backward compatibility:**
   - All existing stock schedules (`realtime.loft`, `woodblock.loft`, `capsules.loft`, `hermit.loft`, `coral_shrine.loft`, `out/spark_pairs/**/*.loft`) continue lowering identically.
4. **Add `crates/loft-passes/tests/targets.rs`:**
   - Test a custom 2-pass `.loft` schedule declaring `target GBuffer: rgba16f view` + `target Depth: depth32f view`, writing to `GBuffer` in pass 1, reading `GBuffer` and drawing to `Swapchain` in pass 2, plus a `target` + `table` mixed compute-and-render schedule.

## Rules (ZERO-COMPILE MODE)
- **DO NOT run `cargo`, `rustc`, `tools/dev.sh`, or `tools/check.sh`.**
- Read all owned files completely upfront. Commit cleanly on your branch by explicit path.
- Final report (<= 250 words): exact changes, new syntax forms lowered, and integration notes.
