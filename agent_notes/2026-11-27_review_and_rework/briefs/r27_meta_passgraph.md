# Lane r27-meta-passgraph: Unify Compute/Render Pass Graph & Open `loft-wgpu` Raster State

Effort: medium

## Mission
Deliver Milestone M1 of the meta-engine rework:
1. Remove the artificial silo between `AuthoredCompute` (`table`/`kernel`/`dispatch`) and raster/render/path schedules so a single `.loft` schedule and `.loftb` bundle can freely mix compute passes with rendering.
2. Stop `schedule.rs` from requiring dummy raster passes for `screen_pass` or standalone compute schedules.
3. Open up `loft-wgpu`'s raster pipeline state (back-face culling, alpha blending, configurable sampler modes, optional vertex buffers) via backward-compatible extensions in `loft-ir/src/execution.rs` — **WITHOUT breaking `LOFTB001` or any existing `.loft` rollout.**

## Strict File Ownership (DO NOT edit files outside this list)
- `crates/loft-ir/src/execution.rs`
- `crates/loft-wgpu/src/command.rs`
- `crates/loft-wgpu/src/storage.rs`
- `crates/loft-wgpu/src/schedule.rs`
- `crates/loft-passes/src/lib.rs`
- `crates/loft-passes/src/bundle.rs`
- `crates/loft-passes/src/pipeline/schedule.rs`
- `crates/loft-passes/src/authored_compute.rs`
- Plus new tests in `crates/loft-wgpu/tests/` and `crates/loft-passes/tests/`.

## Exact Changes to Implement
1. **Backward-compatible `Execution` v3 extensions (`crates/loft-ir/src/execution.rs`):**
   - Bump `VERSION` from `2` to `3`, while `Execution::read` continues decoding `1`, `2`, and `3` cleanly (so old bundles still load!).
   - Add optional per-pass raster pipeline overrides (`RasterState { pass: PassId, cull: CullMode, blend: BlendMode }` with `CullMode::{None, Front, Back}` and `BlendMode::{Opaque, Additive, Alpha}`) and optional per-table sampler descriptors (`SamplerSpec { table: TableId, nearest: bool, repeat: bool }`).
   - Keep `Execution::table()` returning `None` when all fields are empty/default so existing bundles with no overrides stay byte-identical.
2. **Open `loft-wgpu` raster & sampler state (`crates/loft-wgpu/src/command.rs`, `storage.rs`, `schedule.rs`):**
   - Honor `CullMode::Back` (in addition to `None` and `Front`) and `BlendMode::Alpha` (`BlendState::ALPHA_BLENDING`, in addition to `Opaque` and `Additive`) in `command.rs:170-189`.
   - Honor `SamplerSpec` (`FilterMode::Nearest`/`Linear`, `AddressMode::Repeat`/`ClampToEdge`) in `storage.rs:161-165` while keeping current linear/clamp-to-edge as the default.
   - In `schedule.rs:235-239`, instead of unconditionally rejecting `draw.vertex.is_some()`, validate that the vertex table exists and has `stride_bytes > 0` (and pass through cleanly when `vertex` is `None`, keeping vertex-pulling default behavior 100% intact).
3. **Unify `AuthoredCompute` with Render Schedules (`crates/loft-passes/src/lib.rs`, `bundle.rs`, `authored_compute.rs`):**
   - Currently `crates/loft-passes/src/lib.rs:463-477` rejects any schedule mixing `compiled.compute` with rendering, path tracing, or motion (`!compiled.compute.standalone || program.path.is_some() || !program.dispatches.is_empty()`).
   - Lift this restriction! Allow `authored_compute` tables and passes to lower into the **same** `loft_ir::Bundle` alongside rendered passes (emitting their `TableDecl`s, `ModuleBlob`s, `PassDecl`s, and `Execution` entries in declared schedule order without ID collisions against renderer tables/modules/passes).
   - Keep standalone compute schedules working identically (`particle_flip`, `particle_scatter`, `escapes.rs` tests must keep passing).
4. **Stop forcing dummy raster passes for `screen_pass` / standalone compute (`crates/loft-passes/src/pipeline/schedule.rs`):**
   - Currently `schedule.rs:124-133` unconditionally requires `shade_tiles` (`light`), `sun`, and a fullscreen `FramePass` with a `SKY` hole even when `screen_pass` is used or when the schedule is standalone compute.
   - Allow `screen_pass` schedules to compile with just `camera` + `screen_pass(...)` (still accepting existing schedules like `reference.loft` and `spectral_caustic.loft` that include dummy raster passes so zero existing scenes break!).

## Rules & Verification
- **Read all owned files + `crates/loft-passes/tests/escapes.rs` + `crates/loft-passes/src/program.rs` completely BEFORE editing.**
- **Write ALL code before your first compile.** Target 2–3 compiles total via `CARGO_BUILD_JOBS=2 tools/dev.sh`.
- Add integration tests proving:
  1. `Execution` v1/v2/v3 round-trip and `CullMode::Back` + `BlendMode::Alpha` + `SamplerSpec` decoding.
  2. A single `.loft` schedule mixing `table(...)` + `pass { compute(...) }` AND raster rendering compiles into one valid `loft_ir::Bundle`.
  3. A minimal `screen_pass` schedule compiles without dummy `render_bundle`/`shade_tiles` passes.
- Run focused tests: `tools/dev.sh test loft-ir`, `tools/dev.sh test loft-wgpu`, `tools/dev.sh test loft-passes`.
- Commit cleanly on your branch with explicit paths. Do NOT push or merge to `main`.
- Keep final summary concise (<= 200 words) with exact files changed and tests run.
