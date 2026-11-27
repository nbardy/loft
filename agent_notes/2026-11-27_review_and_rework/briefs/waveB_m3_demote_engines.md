# Lane m3-demote-engines: Demote Hardcoded Renderers & WGSL String Surgery Out of Rust Into `stdlib/render/`

Effort: high
Mode: CODE-ONLY (LOFT_NO_COMPILE=1 — DO NOT run cargo, rustc, tools/dev.sh, or tools/check.sh!)

## Mission
Execute Milestone M3 of `agent_notes/2026-11-27_review_and_rework/BLUEPRINT.md` (Wave B). Read `docs/PHILOSOPHY.md` §6 first. Delete hardcoded engine/scene/style branches and brittle WGSL `.replace(...)` string surgery from `loft-check`, `loft-emit`, and `loft-passes`, moving stock engine recipes cleanly into `stdlib/render/` while preserving **identical consumer `.loft` schedule verb signatures** so zero existing rollouts break.

## Strict File Ownership (ONLY edit/delete/add files under these paths)
- `crates/loft-check/src/defaults.rs`
- `crates/loft-emit/src/` (`emit.rs`, `emit/shade_tiles.rs`, `emit/paths.rs`, `emit/lighting.rs`, `material_shape.rs`, `bvh.wgsl`, `spectral_path.wgsl`, `world_path.wgsl`, `world_medium.wgsl`, `emit/shade_coverage.wgsl`, `emit/shade_edges.wgsl`, `emit/shade_coverage_owner.wgsl`)
- `crates/loft-passes/src/shade_schedule.rs`, `crates/loft-passes/src/pipeline/lower_passes.rs`
- `stdlib/render/` (new directory for demoted recipes & WGSL templates!)
- `stdlib/engines/` and `stdlib/core.loft` (keep exact public schedule names & consumer compatibility!)

**DO NOT touch** `crates/loft-parse/`, `crates/loft-check/src/{check,prog}.rs`, `crates/loft-passes/src/{lib,bvh*,bundle,frame_pass,program,pipeline/schedule}.rs`, `crates/loft-std/`, `crates/loft-runner/`, or `tools/` (owned by sibling lanes).

## Exact Demotions & Deletions
1. **Delete `defaults.rs`'s hardcoded `woodblock(PrimaryWorld, SolidShadow, MainCam)` rewrite (`crates/loft-check/src/defaults.rs:27-55`):**
   - Express `pub schedule woodblock(world, shadow, cam)` directly in `stdlib/engines/woodblock.loft` (and re-export or keep accessible so `coral_shrine.loft` and any scene calling `woodblock(...)` works purely through normal `.loft` schedule resolution, with zero hardcoded `"woodblock"` rewrite in Rust!).
2. **Delete scene/style/lobe literal name branches in `crates/loft-emit/src/emit.rs` and `emit/paths.rs`:**
   - Remove hardcoded `"woodblock_pigment"` (`emit.rs`), `"sun_sky"` parameter-name special cases (`emit.rs`), and `"spectral_dielectric"` literal matching (`emit/paths.rs`). Express those choices via structural properties (e.g., parameter/field/lobe kind, schedule/template args, or stdlib helper attributes), NOT string-matching user function names!
   - Double-check every caller in `crates/loft-emit/src/` and `stdlib/` so existing scenes (`capsules`, `blobs`, `hermit`, `coral_shrine`, `out/spark_pairs/`, `corpus/`) produce identical behavior.
3. **Move renderer-specific WGSL recipes out of `crates/loft-emit/src/` into `stdlib/render/`:**
   - Move `spectral_path.wgsl`, `world_path.wgsl`, `world_medium.wgsl`, `emit/shade_coverage.wgsl`, `emit/shade_edges.wgsl`, and `emit/shade_coverage_owner.wgsl` into `stdlib/render/`.
   - Notice `crates/loft-emit/src/emit/shade_tiles.rs` already uses `include_str!("../../../../stdlib/core/...")` + `render_template(...)`. Use that clean `stdlib/render/` template pattern (with explicit `{{HOLE}}` slots) so `compile(src)` works both on inline strings and files, while zero renderer-specific WGSL recipes live in `crates/loft-emit/src/`!
4. **Replace brittle WGSL `.replace(...)` string surgery in `emit/shade_tiles.rs` and `emit/paths.rs`:**
   - Eliminate the 9 exact-text `.replace(...)` calls targeting `bvh.wgsl` local variables (`shade_tiles.rs`), comment-line splitting (`shade_tiles.rs`), and byte-length header slicing (`paths.rs`).
   - Instead, parameterize `bvh.wgsl` (or a clean template helper) with explicit `{{HOLE}}` markers / clean structured WGSL composition — zero fragile substring surgery!
5. **Preserve 100% compatibility** with `out/spark_pairs/`, `corpus/`, `scenes/`, and `engines/gates.json` (`capsules`, `blobs`, `hermit`, `coral_shrine`).

## Rules (ZERO-COMPILE MODE)
- **DO NOT run `cargo`, `rustc`, `tools/dev.sh`, or `tools/check.sh`.**
- Read all owned files completely upfront. Verify every struct, import, pub visibility, and call site carefully so it compiles cleanly on the first integration pass.
- Commit cleanly on your branch by explicit path.
- Final report (<= 250 words): exact Rust/WGSL LOC deleted from `crates/`, new `stdlib/render/` files created, and any integration notes.
