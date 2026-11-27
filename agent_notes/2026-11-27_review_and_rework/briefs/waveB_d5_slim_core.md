# Lane d5-slim-core: Relocate CPU Twins Out of Core Crates Into `tools/loft-verify/` & Clean Obsolete Runtime Flags

Effort: high
Mode: CODE-ONLY (LOFT_NO_COMPILE=1 — DO NOT run cargo, rustc, tools/dev.sh, or tools/check.sh!)

## Mission
Drive Loft's Rust core (`crates/`) sharply closer to the ~10k LOC target in `docs/PHILOSOPHY.md` §1 & §6 by relocating verification-only CPU reference twins (~1,250 LOC) out of `crates/loft-std`, `crates/loft-runner`, and `crates/loft-passes` into `tools/loft-verify/`, and cleaning up obsolete `web-sys` / dead `edit` leftovers in `tools/check.sh` and `crates/loft-native`.

## Strict File Ownership (ONLY edit/delete/add files under these paths)
- `crates/loft-std/Cargo.toml`, `crates/loft-std/src/lib.rs`, `crates/loft-std/src/stepping.rs`, `crates/loft-std/src/cpu_twin.rs`, `crates/loft-std/src/cpu_twin/compute.rs`
- `crates/loft-runner/Cargo.toml`, `crates/loft-runner/src/lib.rs`, `crates/loft-runner/src/stepping.rs`, `crates/loft-runner/src/backend.rs`
- `crates/loft-passes/src/lib.rs` (ONLY removing `pub mod bvh;` / `bvh` re-exports if any — do NOT touch other passes!), `crates/loft-passes/src/bvh.rs`, `crates/loft-passes/src/bvh/twin.rs`
- `crates/loft-passes/tests/bvh.rs`, `crates/loft-passes/tests/engines_cpu.rs`, `crates/loft-passes/tests/phase5.rs`, `crates/loft-passes/tests/reference/` (and any other `crates/loft-passes/tests/*.rs` that imports `CpuWorld`/`bvh::Hit`)
- `crates/loft-cli/Cargo.toml`, `crates/loft-cli/tests/motion_lane.rs`, `crates/loft-cli/tests/escape_gpu.rs`
- `crates/loft-native/src/lib.rs` (ONLY dead-code cleanup from removed `edit`/wasm paths; KEEP `reuse.rs` and all active render/profile/capture methods intact!)
- `tools/loft-verify/` (`Cargo.toml`, `src/lib.rs` (new), `src/main.rs`, `src/cpu_twin.rs`, `src/cpu_twin/compute.rs`, `src/bvh_twin.rs`, `src/trace.rs`, `src/suite.rs`, `src/path_twin.rs`, `examples/`, `tests/`)
- `tools/check.sh`

**DO NOT touch** `crates/loft-parse/`, `crates/loft-check/`, `crates/loft-emit/`, `crates/loft-passes/src/{bundle,frame_pass,program,shade_schedule,pipeline/*}.rs`, `stdlib/`, or `Cargo.lock` (owned by sibling lanes / integration).

## Exact Changes
1. **Create `tools/loft-verify/src/lib.rs`:**
   - Expose `pub mod cpu_twin;` and `pub mod bvh_twin;` from `loft-verify` so `tools/loft-verify/src/main.rs`, `tools/loft-verify/examples/*`, and `tools/loft-verify/tests/*` all share them cleanly without any crate dependency cycle (`loft-verify` depends on `loft-cli`, `loft-passes`, `loft-runner`, `loft-std`, `loft-ir`, `loft-abi`, `loft-eval`, `loft-check`, `loft-parse` as needed — add any missing workspace path deps to `tools/loft-verify/Cargo.toml`).
2. **Relocate `cpu_twin` (~916 LOC) out of `crates/loft-std` and `crates/loft-runner` into `tools/loft-verify/`:**
   - Move `crates/loft-std/src/cpu_twin.rs` + `crates/loft-std/src/cpu_twin/compute.rs` → `tools/loft-verify/src/cpu_twin.rs` + `tools/loft-verify/src/cpu_twin/compute.rs`.
   - Fix internal imports (`crate::stepping::*` → `loft_std::stepping::*`, etc.) and ensure any helper items in `loft_std` needed by `cpu_twin` are `pub` (check every `pub(crate)` item `cpu_twin` uses!).
   - Delete the `cpu-twin` feature flag from `crates/loft-std/Cargo.toml`, `crates/loft-runner/Cargo.toml`, and `crates/loft-cli/Cargo.toml`.
   - Remove the `#[cfg(feature = "cpu-twin")]` `step_pass_cpu`/`step_bundle_cpu` wrappers from `crates/loft-runner/src/{lib.rs,stepping.rs,backend.rs}` and `crates/loft-std/src/{lib.rs,stepping.rs}`.
   - Move `crates/loft-cli/tests/motion_lane.rs` and `crates/loft-cli/tests/escape_gpu.rs` → `tools/loft-verify/tests/motion_lane.rs` and `tools/loft-verify/tests/escape_gpu.rs`, updating imports to `loft_verify::cpu_twin::*` (check relative paths like `env!("CARGO_MANIFEST_DIR")`! Note `crates/loft-cli` is 2 levels deep and `tools/loft-verify` is ALSO 2 levels deep `../../`, so verify every `Path::new(env!("CARGO_MANIFEST_DIR"))`!).
3. **Relocate `bvh/twin.rs` (~281 LOC) out of `crates/loft-passes` into `tools/loft-verify/`:**
   - Move `crates/loft-passes/src/bvh/twin.rs` → `tools/loft-verify/src/bvh_twin.rs`, delete `crates/loft-passes/src/bvh.rs` (and `crates/loft-passes/src/bvh/`), and remove `pub mod bvh;` from `crates/loft-passes/src/lib.rs`.
   - Check EVERY field/function/method `bvh_twin.rs` uses from `loft_passes` (`Compiled`, `KindEval`, etc.) and make sure they remain accessible publicly (`pub`).
   - Update all callers in `tools/loft-verify/src/{trace,suite,path_twin}.rs` and `tools/loft-verify/examples/*` to import `loft_verify::bvh_twin::{CpuWorld, Hit}`.
   - Find ALL tests under `crates/loft-passes/tests/` that reference `loft_passes::bvh` / `CpuWorld` (`bvh.rs`, `engines_cpu.rs`, `phase5.rs`, `reference/`, etc.) and move ONLY those CPU-twin tests cleanly into `tools/loft-verify/tests/` (fixing `CARGO_MANIFEST_DIR` if needed and checking `tools/loft-verify/Cargo.toml` dev-dependencies). Keep non-twin compiler tests in `crates/loft-passes/tests/`.
4. **Fix `tools/check.sh` obsolete `web-sys` flags:**
   - Wave A deleted `crates/loft-wgpu/src/web.rs` and the `web-sys` feature flag from `loft-runner`/`loft-wgpu`. Update lines 65 and 82 of `tools/check.sh` so `wasm` / `full_gate` check `--target wasm32-unknown-unknown` without the deleted `--features web-sys` / `--features loft-runner/web-sys` flags.
5. **Audit `crates/loft-native/src/lib.rs`:**
   - Remove any dead helper functions left behind by Wave A's deletion of `edit.rs` / `edit_cmd.rs`. Do NOT delete `reuse.rs` or any method used by `loft-cli`, `loft-verify`, or `tools/gates/capture.rs`.

## Rules (ZERO-COMPILE MODE)
- **DO NOT run `cargo`, `rustc`, `tools/dev.sh`, or `tools/check.sh`.**
- Double-check EVERY single import, visibility (`pub` vs `pub(crate)`), `Cargo.toml` dependency, and `CARGO_MANIFEST_DIR` path so the final integration compile passes cleanly!
- Commit cleanly on your branch by explicit path.
- Final report (<= 250 words): exact LOC removed from `crates/`, files moved to `tools/loft-verify/`, and any integration notes.
