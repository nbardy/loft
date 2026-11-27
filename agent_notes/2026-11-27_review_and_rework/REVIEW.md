# Loft Codebase Review — 27 Nov 2026

Independent full-codebase audit across all 16 workspace crates, `stdlib/`, `scenes/`, `grammar/loft.lark`, `engines/gates.json`, `out/spark_pairs/`, and `tools/loft-verify/` (~51.7k non-asset lines), grounded strictly in the code and syntax files rather than docs.

## Summary Verdict

Loft is **genuinely compelling as a specialized procedural SDF/parametric scene + deterministic motion compiler** with unusually disciplined verification. However, **it is not yet a meta-engine**: it is a hardcoded visibility-buffer / raymarch / path-tracer renderer with a bespoke DSL on top and a siloed standalone compute mode, carrying real compiler soundness bugs, fragile WGSL string-surgery codegen, and significant drift from `docs/PHILOSOPHY.md`.

---

## 1. Usefulness

### Strengths
- **High authoring density for procedural SDF/parametric scenes:** `stdlib/engines/hermit.loft` (169 LOC) vs `tests/controls/hermit_shade.wgsl` (488 LOC) proves the core thesis for content authoring — `scan` with analytic `spine'(u)`, `map` coordinate pipelines (`q |> mirror_x |> move(...)`), `kind` batching, and composable BSDF lobes (`ggx + film + coat`) are ~3× shorter and much cleaner than hand-written WGSL + wgpu plumbing.
- **Real native embeddability:** `loft-runner`'s `Runner::attach` + `encode_into` records into a host-owned `wgpu::Device`/`CommandEncoder` respecting host formats, MSAA, Reverse-Z, and `LoadOp::Load` (`crates/loft-wgpu/src/attachments.rs:67-72`).
- **Source-derived LLM tooling:** `loft context` (`crates/loft-cli/src/context.rs:86-118`) genuinely extracts stdlib signatures from AST/CST; `loft fork` (`crates/loft-cli/src/fork.rs:77-100`) cleanly rewrites `self::` imports and validates the module graph.

### Pain Points & Gaps
1. **Missing language primitives force ugly workarounds across stdlib & scenes:**
   - **No `v3@Space` vector literal syntax:** `[x, y, z]@World` always yields `p3@World`, forcing `[x, y, z]@World - [0.0, 0.0, 0.0]@World` everywhere (5× in one call at `scenes/crab.loft:70-75`, plus `body.loft`, `mood.loft`, `springs.loft`, `transport.loft`, `coral_shrine.loft`).
   - **No arrays on `kind` rows or locals:** `stdlib/motion/body.loft:25-51` manually unrolls ~100 columns (`ax0..ax7`, `toe_z0..toe_z7`); `stdlib/motion/gait.loft:291-305` lists 67 fields in one `writes` clause and packs/unpacks 14 `v4`s.
   - **No struct return values in expression/twin paths:** `stdlib/motion/mood.loft:5-16` defines `pub type Mood = frame@World` and packs 10 unrelated mood scalars into `origin`/`tangent`/`normal`/`binormal` (`affect.normal.z` = `claw_open`).
   - **`rgb` abused as untyped `vec3` across stdlib:** positions (`grid_spot`), box half-extents (`box_sdf`, `torso`), normals/light dirs (`bsdf.loft`), noise coords (`fbm2`), and sky dirs are typed `rgb` instead of `v3`.
   - **No bitwise/shift operators in surface grammar:** `body.loft:102-109` loops 8× multiplying by 16 for nibbles; `particle_flip/native.loft:18-31` divides by `256` and `65536`.
2. **Authored compute and rendering are siloed:** `crates/loft-passes/src/lib.rs:463-477` rejects any schedule combining `AuthoredCompute` (`table`/`kernel`/`dispatch`) with rendering, path tracing, or motion.
3. **Browser/WASM runner is an incomplete metadata exporter:** `crates/loft-wgpu/src/web.rs:99-291` does not build pipelines or run frames and rejects MSAA, multi-draw bundles, indexed draws, `ComputeFusedTicks`, `PassLoop`, and `GenerateMips`.
4. **Stubs & narrow limits:** `loft fmt` is a 5-line stub (`crates/loft-cli/src/fmt.rs`); native MSL escape hatches do not exist (`ShaderTarget::Wgsl` hardcoded); Tier 0 edit (`crates/loft-native/src/edit.rs:56-75`) still runs full `loft_passes::compile()` and refuses motion/bakes/path tracing; `loft-gltf` strips textures, normals, UVs, alpha, emission, skins, and animation.

---

## 2. Code Quality

### Strengths
- Strict binary wire & replay codecs (`crates/loft-format/src/bundle.rs`, `replay.rs`) with BLAKE3 integrity checks, alignment verification, recursion bounds, and canonical map checks.
- Workspace-wide `unsafe_code = "deny"` and `wildcard_enum_match_arm = "deny"`.
- Solid static Naga telemetry (`crates/loft-passes/src/telemetry.rs`) and keyed-MAC atomic disk cache (`crates/loft-passes/src/cache.rs`).
- Careful deterministic math (`crates/loft-abi/src/det.wgsl`) and quantization (`crates/loft-eval/src/quant.rs`).

### Concrete Bugs & Soundness Holes
1. **Provably unsound Lipschitz certification (`crates/loft-eval/src/eval.rs:1387-1395`):** `map2` ignores componentwise `f(p, q).lip()` and uses `la.hypot(*lb)` — certifying vector `pow`, discontinuous `step`, and `atan2` as finite-Lipschitz. Broadcast `Val::V`/`Val::S` also omits the `sqrt(n)` factor.
2. **`sample_c2` hardcodes `L = 0.0` (`crates/loft-eval/src/eval.rs:903`)**, `Iv` arithmetic lacks outward rounding (`crates/loft-eval/src/lip.rs:5-6`), and `refine.rs` misses `for`-loop widening, vector row fields, and `.x` on constructed vectors.
3. **Physical units are unchecked outside `kernel`s (`crates/loft-check/src/check.rs:731, 773, 968`):** erased to `f32` everywhere else, and trivially forged inside kernels by calling any `fn` (whose body is never unit-checked).
4. **No BSDF energy conservation check exists** (despite `loft-check/src/bsdf/energy.js` precomputing a GGX LUT and `loft-check` hosting a CPU path tracer in `path_twin.rs`).
5. **Broken comparison operator rule (`crates/loft-check/src/check.rs:1619`):** `<`/`<=`/`>`/`>=` typecheck on `Bsdf`, `Xform`, `Pt@World`, `Bool` because `meet(x, y)` accepts any `x == y`.
6. **Transitive E504 determinism check skips `Det` `kernel`s** (`crates/loft-check/src/check.rs:104-120`, only iterates `p.laws`).
7. **Non-sticky determinism tier (`crates/loft-passes/src/motion.rs:60`):** `tier = law.tier;` overwrites `NonDet` if a subsequent law is `Det`.
8. **Mixed `i32`/`u32` `for`-loop bound swap (`crates/loft-eval/src/eval.rs:504`):** `(Val::I(b), Val::U(a))` swaps `lo` and `hi`, running 0 iterations.
9. **Duplicate `FieldMedium` emission (`crates/loft-passes/src/pipeline/stage0.rs:149-159`):** `add_media` runs once plus once per `PointLamp`.
10. **Broken `validate_wgsl` capabilities (`crates/loft-format/src/bundle.rs:214-231`):** hardcodes `Capabilities::default()`, rejecting valid bundles using subgroups, f32 atomics, or ray queries.
11. **AD defects (`crates/loft-eval/src/ad.rs:56, 167, 272-284`):** exponential AST duplication (no CSE), shallow-cloning `If` without refreshing node IDs, and zero derivative for `%`.
12. **`det.rs` vs `det.wgsl` drift:** `det_log` missing from `crates/loft-abi/src/det.rs`; `det_sin(-0.0)` uses Rust `signum()` (`-1.0`) vs WGSL `sign(-0.0)` (`0.0`).
13. **`loft render --time T` skips motion sampling (`crates/loft-cli/src/render.rs:79-119`)**, unlike `loft still` and `loft clip`.
14. **Fragile string-surgery WGSL codegen:** `loft-emit` does not depend on `naga`; pipelines are stitched with exact-text `.replace(...)`, comment-line splits, and header-length slicing (`emit/shade_tiles.rs:31-57`, `emit/paths.rs:12-47`, `loft-passes/src/bundle.rs:168-169`, `loft-wgpu/src/mips.rs:128-150`, `loft-runner/src/initialize.rs:67`).
15. **Grammar vs parser drift & LLM-benchmark overfitting:** `grammar/loft.lark` and `parser.rs` disagree in both directions; `scope.rs:248-251, 388-399` and `check.rs:389` weaken module privacy and `@Space` rules specifically for historical crab eval runs.
16. **Monoliths & duplication:** `check.rs` (1,853), `eval.rs` (1,782), `parser.rs` (1,675), `emit.rs` (1,285); 4 ray-AABB implementations; 3 BVH traversals; 3 CPU twins; 3 ~600-line copies of the gait solver; stale Syntax-A diagnostics.

---

## 3. Performance Engineering

### Strengths
- O(kinds) shader scaling via indirect draws/dispatches.
- Epoch-cached bind groups and `RenderBundle`s, wave pass coalescing, dirty-tracked params, `StagingBelt`, triple-buffered async event outbox, hardware GPU timestamps with Metal 2nd-encoder resolve workaround.
- Rigorous ABBA/BAAB bootstrap parity harness with FMA anti-cheat witness (`tools/loft-verify/src/controls.rs`).

### Bottlenecks & Gate Credibility Gaps
1. **Per-frame CPU allocation churn in `loft-runner` (`encode.rs`, `embed.rs`, `loft-std/inputs.rs`):** allocates `Vec<TextureFormat>` twice per render pass per frame, rebuilds `BTreeSet<PassId>` every frame, allocates param `Vec<f32>` twice per frame, allocates `Vec<BindGroup>`/`Vec<Dispatch>`/`Vec<Color>`/`wave.to_string()` every frame, and re-uploads unchanged input buffers.
2. **Per-frame full CPU recompile + pipeline rebuild for animated clips (`crates/loft-cli/src/clip.rs:156-177`).**
3. **GPU ray/BVH inefficiencies:** 6 float divisions per AABB test in `bvh.wgsl:16-27` and `mesh.wgsl:43-52` (no precomputed `inv_d`); median-split BVH (no SAH); un-ordered child push in `mesh.wgsl:77-81`; double interior marching in `shade_edges.wgsl:8-13`; 6-tap finite-diff normals in `world_path.wgsl:87-95`; 3× surface eval per pixel; lockstep motion opening 1 compute pass + 1 buffer copy per tick per pass (`encode.rs:472-508`).
4. **Silent pass on contended Speed gates (`tools/loft-verify/src/suite.rs:752-761`):** `!c.contended` is filtered out of `failed`, so a contended Speed gate exits 0; `QUIET_CONTROL_P50_MS = 1.10` is hardcoded for Apple M4.
5. **`controls.rs` bugs:** still uses deprecated 1-triangle probe (`CONTENDED_MS = 0.35`) instead of `load_control`, and `whole_frame_ms` (`controls.rs:743-745`) fails to filter `0` timestamps from skipped shadow passes.
6. **No real pass fusion:** `crates/loft-passes/src/fusion.rs` only wraps motion dispatches into `Bundle`s.

---

## 4. Faithfulness to the Meta-Engine Concept

- **Core size:** Runtime core (`loft-std` + `loft-wgpu` + `loft-runner`) is 5,711 non-test LOC (+1,362 in `loft-native`), plus runtime `naga`, vs `PHILOSOPHY.md`'s "hundreds of lines."
- **Privileged compiler verbs & hidden templates:** `realtime.loft` (15 LOC) and `woodblock.loft` (17 LOC) are thin wrappers over compiler-hardcoded verbs (`render_bundle(Vis64)`, `find_shadow_map`, `shade_tiles`, `outline_and_paper`, `screen_pass`, `Drawables`, `Casters`, and `defaults.rs`'s secret `woodblock(...)` rewrite). 26 of 34 `stdlib/core/*.wgsl` files are compiler-internal Mustache templates never imported by `.loft`.
- **Sealed raster pipeline state:** No user `target` declarations (`E405`), no vertex buffers (`schedule.rs:235`), no `Face::Back` culling or alpha blending (`command.rs:170-189`), hardcoded samplers, hardcoded `1/120 s` tick rate, and dummy raster passes required even for path tracing (`schedule.rs:124-133`).
- **Scene/demo hardcoding in core crates:** `CreatureHeader` in `loft-abi/src/lib.rs:64-90`; horizontal caustic receiver `abs(h.p.y) < 0.015 && h.n.y > 0.99` in `spectral_path.wgsl:63,78`; compiler branches on literal names `"woodblock_pigment"`, `"sun_sky"`, `"ggx"`, `"sss"`, `"emit_lobe"`, `"spectral_dielectric"`; 24-file stdlib whitelist including `stdlib/migration/bead_observation.loft`; crab claws/eyestalks hardcoded into `Walker`/`Trunk`.
