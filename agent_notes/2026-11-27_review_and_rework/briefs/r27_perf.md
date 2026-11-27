# Lane r27-perf: Runtime Hot-Path Allocation Cleanup & GPU Ray/BVH Perf

Effort: medium

## Mission
Eliminate per-frame CPU heap allocation churn in `loft-runner`/`loft-std` and fix the low-hanging GPU ray/BVH bottlenecks in `loft-emit`, while keeping 100% API compatibility and bit-identical rendered pixels on all existing scenes and rollouts.

## Strict File Ownership (DO NOT edit files outside this list)
- `crates/loft-runner/src/encode.rs`
- `crates/loft-runner/src/embed.rs`
- `crates/loft-std/src/inputs.rs`
- `crates/loft-emit/src/bvh.wgsl`
- `crates/loft-emit/src/mesh.wgsl`
- Plus tests inside `crates/loft-runner/src/tests.rs` or `crates/loft-std/tests/`.

## Exact Improvements to Implement
1. **Precomputed `inv_d` and ordered mesh BVH traversal (`crates/loft-emit/src/bvh.wgsl`, `mesh.wgsl`):**
   - In `bvh.wgsl` and `mesh.wgsl`, precompute `inv_d = 1.0 / ray.d` (preserving exact `rd == 0.0` safety / IEEE Inf/NaN slab semantics so zero hits change!) once per ray before traversal, replacing the 6 float divisions per AABB test inside the loop.
   - In `mesh.wgsl:56-96`, mirror `bvh.wgsl`'s near-child-first ordering and entry-distance pruning (`hit_a`/`hit_b` comparison so the far child is only pushed if its `tmin < best_t` and the near child is visited first in register).
   - Double check that `bvh.wgsl` and `mesh.wgsl` still preserve any exact symbol names/comments relied on by `crates/loft-emit/src/emit/shade_tiles.rs` (`loft_bvh_nodes[stack[depth]]`, `loft_bvh_leaves[first + j]`, etc.) — **read `crates/loft-emit/src/emit/shade_tiles.rs` before editing `bvh.wgsl`!**
2. **Eliminate steady-state per-frame CPU allocations in `crates/loft-runner/src/encode.rs` & `embed.rs`:**
   - Precompute the static `initializer_passes: BTreeSet<PassId>` once at `Instance` creation instead of rebuilding it every frame (`encode.rs:439`).
   - In `record_initializers` (`encode.rs:521`), do NOT allocate `Vec<f32>` of all params unless there is at least one dirty-clock initializer that actually needs checking.
   - In `Instance::prepare` / `embed.rs`, avoid allocating `Vec<TextureFormat>` twice per render pass per frame just to verify the cached `Canvas` matches; compare slices/iterators in-place and only allocate on cache miss.
   - Avoid `wave.to_string()` per batch per frame (`encode.rs:330`) — use a small stack buffer or static/cached label.
   - Avoid unnecessary `Vec<BindGroup>` / `Vec<Dispatch>` / `Vec<Color>` reallocations on hot steady-state frame recording where practical (e.g., reuse scratch `Vec`s on `Instance` or pass slices directly to `Batch`).
3. **Dirty-track `Inputs` uploads (`crates/loft-std/src/inputs.rs`):**
   - Track a `dirty: bool` flag on `Inputs` (initially `true` if non-empty, set on any `set_level`/`push_event`/`set_pose_packet` mutation, cleared on `advance`), so `Inputs::advance` returns `Ok(None)` on frames with zero input mutations instead of re-uploading the entire input buffer every frame.

## Rules & Verification
- **Read `crates/loft-emit/src/emit/shade_tiles.rs` and all owned files completely BEFORE editing.**
- **Write ALL code before your first compile.** Target 2–3 compiles total via `CARGO_BUILD_JOBS=2 tools/dev.sh`.
- Run focused tests: `tools/dev.sh test loft-std`, `tools/dev.sh test loft-runner`, `tools/dev.sh test loft-passes -- bvh` (to verify BVH/mesh WGSL compiles via Naga and passes CPU/GPU parity tests).
- Commit cleanly on your branch with explicit paths. Do NOT push or merge to `main`.
- Keep final summary concise (<= 200 words) with exact files changed and tests run.
