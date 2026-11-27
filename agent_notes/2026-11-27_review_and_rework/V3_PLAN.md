# Loft v0.3 / v3 Architecture & Rework Plan

> Base: **`v0.2.0`** (`rework/meta-10k` merged onto `main`) — 27.7k Rust core LOC, `Execution` v3, real `target` pass graphs, demoted `stdlib/render/` templates, `v3@Space`/`match`/`if` in `build`, `490/490` grammar pass, `110/111` `spark_pairs` pass.

## North Star

Make Loft the **"Rust for GPU Worlds & Hybrid Art"**:
1. **Rust-level compile-time safety** for 3D geometry, shaders, and simulation (`@Space`, interval/Lipschitz bounds, disjoint `writes`, deterministic `det_*`, compiler-owned bindings/barriers/layouts).
2. **½ the tokens** of Loft v0.2 for the same scenes (~50–65% fewer tokens, zero duplicated SDF code in `material`).
3. **First-class hybrid Code + Blender-style Sculpting & Iterative Art** (`docs/PHILOSOPHY.md` §3.5): non-destructive composition of parametric `.loft` code + sculpted residual fields/volumes/splines/meshes + instant live viewport hot-reload.
4. **~8k–12k LOC Rust core** (`crates/`): direct-to-Naga IR, zero `.wgsl` template strings, ONE generic `wgpu` DAG runner, `stdlib/` uncapped.

---

## Track 1 — Core Slimming (v0.2.x → ~12k–15k LOC, zero user breakage)

Finish cutting the remaining legacy overlap left over from v0.1 so the v3 architecture lands on a tiny foundation:
1. **Collapse the 4 runtime crates into ONE `loft-runner` (~5.5k → ~2.0k LOC):**
   - Merge `loft-std` (845) + `loft-runner` (1,251) + `loft-wgpu` (2,194) + `loft-native` (1,161) into a single generic `wgpu` runtime crate (`loft-runner`, ~2k LOC) plus minimal `loft-abi` / `loft-ir` / `loft-format`.
   - Delete redundant trait layers (`ExecutionBackend` indirection where `wgpu` is the only backend), duplicated frame/param/attachment glue, and native/headless duplication.
2. **Port stock engines onto M2 `target` + `pass` graphs, delete legacy baked pass lowerers (~4k–5k LOC):**
   - Rewrite `stdlib/engines/{realtime,woodblock,reference,spectral_caustic}.loft` to declare `Vis64`, `Depth`, `Shadow`, `ColorHdr` via real `target` declarations and generic `pass` steps.
   - Delete legacy `Pass::Vis` / `Pass::Shadow` / `Pass::Shade` / `Pass::Path` special-case lowering from `crates/loft-passes/` and `crates/loft-emit/`.
3. **Clean pre-existing Clippy debt** in `loft-check` / `loft-std` so `tools/dev.sh lint --workspace` passes with 0 warnings under `-D warnings`.

---

## Track 2 — Loft v3 Language & Direct-to-Naga Compiler (½ the tokens, 0 `.wgsl` strings)

### 2.1 Unified `shape` / Geometry + Material Composition (kills ~35–40% token bloat + double compute)
**Problem today:** `trace(q) -> f` and `material(q, n) -> Bsdf` are separate entry points. Even when authors factor out SDF helper functions, `material(q, n)` must re-evaluate every sub-part SDF to determine which part was hit and blend materials.

**v3 design:**
- Flat CSG combinator blocks with direct material binding (`with`):
  ```loft
  shape Crab {
    // Implicit local `q: p3@Local`, implicit `pose: xf@Local>World`, auto-derived interval `bounds`.
    union(k: 0.04) {
      ellipsoid([0.42, 0.22, 0.34]) with chitin(q)
      mirror_x {
        at([0.15, 0.14, 0.28]) { sphere(0.05) with glossy_eye }
        capsule([0.2, 0, 0.1], [0.55, -0.1, 0.25], 0.04) with chitin(q)
      }
    }
  }
  ```
- **Two-pass codegen from ONE source tree (fast raymarch + zero duplicate code):**
  1. **Distance-only `trace_dist(q) -> f`:** Compiler strips all `with` material subexpressions so the 30–80-step raymarch loop pays **zero** material cost.
  2. **Single-pass `shade_hit(q, n) -> Bsdf`:** Evaluated **once** at the hit point, computing smooth-min (`k`) blend weights directly from the same CSG tree (exact material blending at smooth-union seams, zero author duplication!).
  3. **Auto-derived `bounds` + Lipschitz + guard boxes** from the exact same tree — authors never hand-write `bounds`.

### 2.2 Direct-to-Naga IR & Loft-Authored Shaders (kills `.wgsl` templates & string stitching)
- Emit `naga::Module` IR directly from Loft's typed IR (no `Loft → WGSL text → naga::front::wgsl::parse_str → naga::back::wgsl` round-trip!).
- Allow writing `@compute`, `@vertex`, and `@fragment` passes **directly in `.loft`** with typed `table<T>` and `image<Fmt>` handles.
- Passing lighting models, BSDFs, or SDFs into a renderer becomes ordinary typed higher-order/generic composition — **0 `.wgsl` template files, 0 `{{HOLE}}` slots**, full compile-time checking of bindings, formats, and `@Space`.

### 2.3 One Data Model (`table<T>` + `image<Fmt>`) & Functional Render DAGs
- Unify `kind` / `struct` / `target` / `bake` into typed **`table<T>`** and **`image<Fmt>`**.
- Replace magic schedule verbs with ordinary typed `stdlib` DAG functions:
  ```loft
  fn realtime(world: World, cam: Camera) -> Image<rgba16f> {
    let vis = raster_vis(world, cam)
    let shd = shadow_map(world.casters, size: 1024)
    let hdr = shade_tiled(vis, shd, world, light: sun_sky)
    tonemap_aces(hdr)
  }
  ```
- Compiler infers attachment allocation, aliasing, clear-on-first-write, and GPU barriers automatically from the DAG.
- Add record spread (`Crab { leg_len: 0.6, ..base }`) and expected-type space inference (`[0, 1, 0]` infers `p3@S` or `v3@S` from context while still forbidding `p3 + p3` and cross-space mixes).

---

## Track 3 — Hybrid Code + Blender-Style Sculpting & Iterative Art

Turn `docs/PHILOSOPHY.md` §3.5 into concrete, composable engine primitives:

1. **Non-Destructive Layered Geometry (`base_code + sculpt_delta`):**
   - A `SculptField` is an ordinary typed sparse/brick 3D `image<r16f>` (or `rgba16f` for SDF delta + paint/roughness delta) attached in a shape's `@Local` space (or bone/rig space!):
     ```loft
     shape Crab {
       union(k: 0.04) { ... } + sculpt_delta("sculpts/crab_shell.loftvol")
     }
     ```
   - **Lipschitz-certified sculpt volumes:** The load-time/edit-time brick updater computes the exact max gradient `||∇δ||` per brick (trivial 1-pass compute kernel!), so sculpted SDFs stay **100% sphere-trace sound** with zero manual `unproved` opt-outs!
   - **Hybrid SDF + Mesh + Spline:** Sculpt a base in Blender (`.glb`/`.obj`) OR inside Loft's live sculpt brush, import or bind as a `Mesh` or `SculptField`, boolean-union/smooth-blend it with procedural `.loft` details (patterns, scales, joints, fur), and raytrace the combined world in one pass!

2. **Interactive Direct-Manipulation Session (Blender-style Viewport + Code Side-by-Side):**
   - **Live GPU Sculpt / Paint Brushes** (just `@compute` passes in `stdlib/sculpt/` writing to the brick `SculptField` / paint `image`!): add, subtract, smooth, crease, pinch, inflate, vertex/surface paint — undo stack is a simple brick-diff ring buffer.
   - **Interactive Gizmos & Param Editing:** Pick any `Kind` row via the existing GPU `pick` buffer (`(kind_id, row_id, world_pos)`), drag transforms/control points/params live, and **write changes back** cleanly to either a sidecar data layer (`edits.loftdata` / `.loftvol` — never clobbering hand-written `.loft` logic!) or explicit `param` overrides.
   - **Sub-100ms Hot Reload:** Live reload `.loft` edits while preserving camera, live sculpt buffers, and motion state (via `Storage::compatible_state` layout hashes already in `loft-wgpu`).

3. **Dual-Workflow Parity (Human Artist + Visual AI Agent):**
   - Every sculpt/paint/pose brush is ALSO callable headlessly as a deterministic Loft command/table edit, so an AI artist agent can combine **code edits + targeted sculpt/paint strokes + multi-view raytraced critique** in the same loop!

---

## Suggested Milestone Sequence for v0.3 → v1.0 (v3)

| Milestone | Scope | Measurable Exit Gate |
|---|---|---|
| **M0 (Next — Benchmark)** | Run A/B prompt eval on `v0.2.0` (`v3@Space`, `match`, `if` in `build`, named/default args, reusable `schedule`) vs `v0.1.0` | Token count & first-pass compile rate measured on 10+ creature/scene prompts |
| **M1 (Track 1)** | Collapse 4 runtime crates → 1 generic `loft-runner`; port stock engines to M2 `target`/`pass`; delete legacy lowerers | Core `crates/` ≤ **15k LOC**, Clippy workspace `-D warnings` green, 100% GPU parity |
| **M2 (Track 2a)** | Unified `shape { union(k) { ... with mat } }` dual codegen (`trace_dist` + `shade_hit` + auto-bounds) | ≥ **35% token reduction** on `crab.loft` / `boss-otter`, zero raymarch slowdown |
| **M3 (Track 2b)** | Direct-to-Naga IR + Loft-authored `@compute`/`@fragment` shaders + typed functional render DAGs | **0 `.wgsl` templates** in `stdlib/render/`, core `crates/` ≤ **10k LOC** |
| **M4 (Track 3)** | Lipschitz-certified `SculptField` (`image3d` bricks) + `stdlib/sculpt/` GPU brushes + non-destructive code+sculpt hot-reload viewport | Live interactive sculpt + procedural `.loft` detail + raytraced render at 60 fps |
