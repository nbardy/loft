# Lane m3-demote-engines: Demote Hardcoded Renderers & WGSL String Surgery Out of Rust Into `stdlib/render/`

Effort: high
Mode: CODE-ONLY (LOFT_NO_COMPILE=1 — DO NOT run cargo, rustc, tools/dev.sh, or tools/check.sh!)

## Mission
Execute Milestone M3 of `agent_notes/2026-11-27_review_and_rework/BLUEPRINT.md` (Wave B). Read `docs/PHILOSOPHY.md` §6 first. Delete hardcoded engine/scene/style branches and brittle WGSL `.replace(...)` string surgery from `loft-check`, `loft-emit`, and `loft-passes`, moving stock engine recipes cleanly into `stdlib/render/` while preserving **identical consumer `.loft` schedule verb signatures** so zero existing rollouts break.

## Strict File Ownership (ONLY edit/delete/add files under these paths)
- `crates/loft-check/src/defaults.rs`
- `crates/loft-emit/src/` (`emit.rs`, `emit/shade_tiles.rs`, `emit/paths.rs`, `emit/lighting.rs`, `material_shape.rs`, `spectral_path.wgsl`, `world_path.wgsl`, `world_medium.wgsl`)
- `crates/loft-passes/src/shade_schedule.rs`, `crates/loft-passes/src/pipeline/lower_passes.rs`
- `stdlib/render/` (new directory for demoted recipes!)
- `stdlib/engines/` (update `realtime.loft`, `woodblock.loft`, `reference.loft`, `spectral_caustic.loft` to compose `stdlib/render/` cleanly while keeping exact public schedule names!)

## Exact Demotions & Deletions
1. **Delete `defaults.rs`'s hardcoded `woodblock(PrimaryWorld, SolidShadow, MainCam)` rewrite (`crates/loft-check/src/defaults.rs:27-55`):**
   - Express `pub schedule woodblock(world, shadow, cam)` directly in `stdlib/engines/woodblock.loft` (imported via `stdlib/core.loft` or `woodblock.loft`) so `coral_shrine.loft:213` works purely through normal `.loft` schedule calls!
2. **Delete scene/style/lobe literal name branches in `crates/loft-emit/src/emit.rs`:**
   - Remove hardcoded `"woodblock_pigment"` (`emit.rs:957`), `"sun_sky"` parameter-name special cases (`emit.rs:196, 583, 861, 1000`), and `"spectral_dielectric"` literal matching (`emit/paths.rs:140`). Express those choices via explicit schedule/template parameters or structured attributes, NOT string-matching user function names!
3. **Move `spectral_path.wgsl` (hardcoded `y = 0` horizontal caustic photon mapper!), `world_path.wgsl`, and `world_medium.wgsl` out of `crates/loft-emit/src/` into `stdlib/render/`:**
   - Make `stdlib/engines/reference.loft` and `stdlib/engines/spectral_caustic.loft` reference those `.wgsl` modules explicitly in `stdlib/render/` instead of embedding demo-specific shaders inside the Rust compiler binary!
4. **Replace brittle WGSL `.replace(...)` string surgery in `emit/shade_tiles.rs` and `emit/paths.rs`:**
   - Eliminate the 9 exact-text `.replace(...)` calls targeting `bvh.wgsl` local variables (`shade_tiles.rs:31-41`), comment-line splitting (`shade_tiles.rs:49-57`), and byte-length header slicing (`paths.rs:47, 94`). Use clean parameterized template slots (`{{HOLE}}`) or structured module assembly with explicit bindings.
5. **Preserve 100% compatibility** with `out/spark_pairs/`, `corpus/`, `scenes/`, and `engines/gates.json` (`capsules`, `blobs`, `hermit`, `coral_shrine`).

## Rules (ZERO-COMPILE MODE)
- **DO NOT run `cargo`, `rustc`, `tools/dev.sh`, or `tools/check.sh`.**
- Read all owned files completely upfront. Commit cleanly on your branch by explicit path.
- Final report (<= 250 words): exact Rust LOC deleted, new `stdlib/render/` files created, and any integration notes.
