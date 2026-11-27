# Lane c2-collapse-runtime: Slim Runtime Core, Delete `web.rs` & `loft-native/edit.rs`, Wire Execution v3

Effort: high
Mode: CODE-ONLY (LOFT_NO_COMPILE=1 — DO NOT run cargo, rustc, tools/dev.sh, or tools/check.sh!)

## Mission
Execute the runtime portion of `agent_notes/2026-11-27_review_and_rework/BLUEPRINT.md` §2 (Wave A). Read `docs/PHILOSOPHY.md` §6 first. Cut ~2,000+ lines of bloated/duplicated runtime code and wire `Execution` v3 overrides end-to-end through `loft-runner`.

## Strict File Ownership (ONLY edit/delete/add files under these paths)
- `crates/loft-std/`
- `crates/loft-wgpu/`
- `crates/loft-runner/`
- `crates/loft-native/`
- `Cargo.toml` (ONLY if removing an example target; do NOT remove workspace members yet until integration)

## Exact Cuts & Rewrites
1. **Delete `crates/loft-wgpu/src/web.rs` (289 LOC):**
   - Remove `mod web;` / `pub mod web;` and `wasm-bindgen` / `js-sys` / `web-sys` boilerplate tied solely to the broken parallel JS metadata exporter (`loft-wgpu` already targets WebGPU via `wgpu`). Check `crates/loft-wgpu/Cargo.toml` and `crates/loft-runner/` for any references and clean them up.
2. **Delete `crates/loft-native/src/edit.rs` (142 LOC):**
   - Remove half-baked Tier-0 hot edit from `loft-native` (document the `crates/loft-cli/src/edit_cmd.rs` stub/removal needed at integration).
3. **Deduplicate `crates/loft-native/src/motion.rs`:**
   - Merge the duplicated `motion_layout` / `scalar_writes` scalar-f32 `Row` inspection into one clean helper (~60 lines saved).
4. **Move `crates/loft-runner/src/tests.rs` (372 LOC) out of `src/` into `crates/loft-runner/tests/runner.rs`:**
   - Expose only the minimal `pub` / test helper needed so `tests/runner.rs` compiles cleanly and `tools/lines.py` stops counting 372 lines of tests as runtime core.
5. **Relocate `crates/loft-std/src/compute_twin.rs` (747 LOC) + `stepping.rs` (341 LOC) cleanly behind an optional feature or out of default runner path:**
   - Read all callers of `compute_twin` and `stepping` across `loft-runner`, `loft-native`, `loft-cli`, `tools/loft-verify` using `rg`.
   - If `loft-runner` only needs a tiny tick-history/clock tracker (~80 lines) while the Naga WGSL CPU interpreter (`compute_twin.rs`, 747 LOC) is only used by verification/tests, separate `compute_twin.rs` behind a `cpu-twin` Cargo feature (or cleanly decouple it from the runtime core) and slim `loft-runner`'s thin 8–32 line forwarding wrapper files (`bundle.rs`, `events.rs`, `params.rs`, `pick.rs`) into `lib.rs` / `encode.rs`.
6. **Wire `Execution` v3 overrides in `crates/loft-runner/src/lib.rs` & `initialize.rs`:**
   - Pass decoded `&Execution` into `loft_wgpu::schedule::Schedule::with_execution` and `storage::allocations_with_execution` (instead of stripping `DESCRIPTOR` before calling `loft-wgpu`), so `CullMode::Back`, `BlendMode::Alpha`, `SamplerSpec`, and optional vertex tables actually work end-to-end in `Runner::load_bundle`!
7. **Keep the clean ~25-line `Inputs` dirty-tracking fix in `crates/loft-std/src/inputs.rs`** (already proven clean in Wave 1).

## Rules (ZERO-COMPILE MODE)
- **DO NOT run `cargo`, `rustc`, `tools/dev.sh`, or `tools/check.sh`.**
- Read ALL relevant files and callers across the workspace via `rg` before editing so every import, visibility (`pub`/`pub(crate)`), and signature is 100% compiler-ready.
- Commit cleanly on your branch by explicit path. Do NOT push or merge.
- Final report (<= 250 words): exact LOC removed, files deleted/moved, and any downstream integration line needed in `loft-cli`/`tools/loft-verify`.
