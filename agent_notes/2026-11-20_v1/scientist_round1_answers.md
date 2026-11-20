# Round 1 Response — Chief Scientist

All claims below are checked against `main` (`fa09876`), `AB_OLD_VS_NEW.md`, `tok1.md`, `std1.md`, `lean1.md`, `pf1.md`, and `x2*` lane reports. Every figure is tagged **(measured)** or **(predicted)**.

---

## 1. Is critique 1 right that our per-pixel path is the problem? Where to look first?

**Partially right — and misdiagnosed on the headline `58 ms vs 27 ms` number.**

1. **First, throw out the raw `58 ms` magnitude — the matched A/B ran under 100% external GPU contention.** In `AB_OLD_VS_NEW.md:114–138`, the native triangle control measured **0.315–0.974 ms (measured)** against the **0.350 ms** contention cutoff (vs. **0.030–0.096 ms (measured)** uncontended), and machine GPU utilization was pinned at **100% (measured)**. The ~2× ratio on Table B is directionally real, but the absolute **27–32 ms (measured)** delta is contention-inflated and must be re-measured in a quiet window.
2. **Second, Table B is 2× slower because of a schedule/API omission, NOT because Loft's BSDF ALU is slow.** As `AB_OLD_VS_NEW.md:60–65` documents, `shade_tiles` does not expose `(hit, px, world)` to the shader, and Vis64 is single-sample. To reproduce legacy's 4-sample MSAA + ray-traced sun + 1-bounce GI, the author had to **discard the raster visibility buffer completely** and run `screen_pass` with **4 full-screen software BVH primary-ray queries per pixel** plus finite-difference normals. That is an architectural plumbing bug, not slow pixel math.
3. **Where to look first (strictly ranked by actual milliseconds on M4):**
   1. **Expose `(hit, px, world)` (and optional 4× MSAA VisBuf) inside `shade_tiles` + wire `EXPERIMENTAL_RAY_QUERY` into `World`.** Restores hardware raster primary visibility for heavy lighting and puts secondary shadow/GI rays on M4 RT cores. Expected impact on matched Table B (20 groups, uncontended): **~50–70% frame-time reduction (predicted)**, flipping native from ~2× slower to **faster than legacy (predicted)**.
   2. **Finish sky cubemap baking (`x2c`) by fixing the missing Shade pipeline cube binding.** `pf1` measured **1.542 ms (measured)** of Hermit-20's **2.188 ms (measured)** Shade time sitting inside the unbaked 12-direction per-pixel BSDF sky loop; `x2c` escalated solely because the Shade pipeline layout lacked the cube binding (`out/meta/x2c/binding.log`). Expected win: **-1.0 to -1.3 ms on Hermit-20 (predicted)** (taking Hermit-20 from **3.75 ms (measured)** down to **~2.5 ms (predicted)**).
   3. **CoralBranch analytic proxy culling on Shrine.** `s1_lip` (`21a3ba1`) already eliminated the march-step bottleneck (**L: 22.87 → 1.0000002**, mean steps **129.7 → 7.5**, Shrine Vis **-0.935 ms p50 / -0.592 ms p95 (measured)**). What remains on Shrine is the 3,069 `CoralBranch` proxies (**0.957 ms (measured)** upper bound in `pf1`), worth **-0.25 to -0.45 ms (predicted)** via coarse cluster culling.
   4. **Do NOT waste time tuning BSDF ALU or march steps further right now.** `HermitShell.wgsl` already hoists view-dependent terms into `loft_prepare_bsdf`, `s1_controls` measured Hermit Shade at **0.986× (measured)** of hand-written WGSL, and Hermit's mean SDF steps are already **11.1 (measured)**.

---

## 2. Is 18k lines the right ceiling, or is a smaller core possible? Where is the compiler/library line?

**18.0k scc lines (`lean1.md`) is the right near-term ceiling for the full 9-crate Rust workspace, and ~14–15k (predicted) is achievable once flagship rendering/motion moves into `.loft` std — but `lean1` failed because it tried to golf Rust while keeping the wrong crate boundaries.**

- **Why the current 31.5k scc / 33.6k physical lines (measured) happened:**
  1. **Duplicated execution (~4,300 lines):** `loft-std` holds **~2,429 lines (measured)** of `wgpu` device/encoder code split away from `loft-runner` (**1,630 physical / 1,553 scc (measured)**), while `loft-cli/src/native` holds a parallel **~2,208 scc-line (measured)** renderer. `x2a` proved that wrapping both without deleting the parallel path actually *grew* execution to **6,717 lines (measured)**.
  2. **Repeated AST classification across 5 compiler passes (`lean1.md:49`):** `check.rs`, `eval.rs`, `ad.rs`, `emit.rs`, and `tier0_author.rs` each re-classify callees, operands, and stages independently, inflating `loft-passes` to **15,547 scc lines (measured)**.
  3. **Library recipes hardcoded in Rust:** `bsdf.rs`, `spectral_path.wgsl`, `world_medium.wgsl`, built-in lighting/schedule generators, and solver templates live in `loft-passes` instead of `.loft`.

- **The crisp line between Compiler (Rust) and Library (`.loft`):**
  - **Compiler (Rust) owns ONLY domain-agnostic mechanics:**
    1. Lossless E+ parse + spaces/units/ownership/staging checker (`loft-parse` + `check` → one resolved typed HIR).
    2. Symbolic forward AD + sound interval/Jacobian Lipschitz derivation (`ad`, `lip`, `eval`).
    3. Universal `Table + Kernel + Pass` lowering to Naga/WGSL/MSL + `Bundle` packing (`loft-passes` + `loft-format`).
    4. `loft-runner` (L0 — owning **all** `wgpu` code) and `loft-std` (**zero `wgpu`**, pure CPU twin + replay/layout helpers).
  - **Library (`.loft`) owns ALL rendering, shading, lighting, schedule, and motion recipes:** BSDF lobes, sky integration, woodblock/toon/PBR styles, path/vis-buffer/spectral schedules, gaits, and physics kernels.

- **Realistic target ceilings (counting ALL `wgpu`-touching code honestly, non-test + inline tests per `scc`):**
  - **Wave 1 (delete split-brain + unify checked HIR):** **≤ 18,000 scc lines (predicted)** — with `loft-std` at **0 `wgpu` deps**, `loft-runner ≤ 2,200`, `loft-cli/native ≤ 450`.
  - **Wave 2 (after whole-pass WGSL + `kernel` + `bake_tex` sampling let schedules/BSDFs move to `.loft`):** `loft-passes` drops from **8,000 → ~5,500–6,500**, total Rust workspace **~14,500–15,500 scc lines (predicted)**. Going below ~12k would mean deleting real paying features (lossless CST, AD/Lipschitz proofs, `.loftb`/`.loftlog`, CPU twin).

---

## 3. How should variety work for LLM-generated content?

**Material-family uber-shaders + `MaterialTable` + bindless textures for 95% of variety, backed by compile-on-demand + persistent pipeline cache for truly novel math. Explicitly kill the idea of a "material VM done right."**

| Approach | Compile cost per new material | Runtime GPU cost | LLM-variety ceiling | Verdict |
| :--- | ---: | ---: | :--- | :--- |
| **Material-family uber-shaders + packed `MaterialTable` + bindless** | **0 ms (measured/structural)** | **~0.0–0.1 ms overhead vs specialized (predicted)** | **10,000+ unique materials/textures** in ~6–10 PSOs (`StandardPbr`, `LayeredCoat`, `StylizedInk`, `DielectricGlass`, `Emissive`, `Foliage`) | **Build this as the default material path.** |
| **Compiled `kind` + content-hashed pipeline cache (async compile)** | **~70 ms cold (measured)**, **~2–5 ms warm (measured)** | **0 ms overhead (baseline)** | Unlimited novel SDFs, domain folds, custom optical laws | **Keep alongside uber-families** for bespoke geometry & novel math. |
| **Material bytecode VM ("Tier 0 done right")** | **~0.1–0.5 ms upload (measured)** | **+2.0–5.0 ms full-screen (measured on Tier 0)** | High theoretically, crippled by Metal register spills & branch divergence | **Reject for shading.** Keep Tier 0 *strictly* for sub-second live geometry previews during the 400 ms quiet window before async Tier 1 swap. |
| **Compiler auto-merging arbitrary kinds under a PSO cap** | 1 huge PSO (**200–600 ms cold (predicted)**) | High register-spill risk (switch over divergent BSDFs) | Fragile | **Defer / cut.** Uber-families already solve the PSO budget cleanly without creating a mega-switch compiler cliff. |

**Why this has the highest ceiling for LLMs:** Models can freely vary albedo, roughness, metallic, anisotropy, coat, thin-film thickness, emission, ink bands, hatching weights, and procedural/baked texture IDs across **thousands of objects** by emitting pure row data (`MaterialTable` rows, 64–128 B each) with **zero pipeline compiles**, while still dropping down to a custom `kind` whenever they invent genuinely new geometry or math.

---

## 4. What is the `style ↔ integrator` contract? Can an NPR style be integrator-agnostic?

**No — an NPR style is NOT truly integrator-agnostic, and pretending it is produces a leaky slogan. Split the contract cleanly into two honest interfaces.**

- **Why "swap any style with any integrator" is false:**
  - A physical path tracer (`reference`, `spectral_caustic`) integrates total incident radiance against a physical BSDF (`eval`/`sample`/`pdf`) and erases which light was "the sun", what the screen-space ink width is, or whether a shadow came from a silhouette edge.
  - Woodblock, toon, sketch, and papercraft (`std1.md:22–38`) explicitly depend on **decomposed lighting terms** (key-light direction & visibility, sky/indirect fill, local AO, screen-space footprint `footprint_m`, depth/ID discontinuities for ink outlines, and anchor coordinates for mottle/hatching).

- **The clean, falsifiable two-contract split:**
  1. **Physical Material Contract (`Bsdf`) — works on EVERY physical integrator (`realtime`, `reference`, `spectral_caustic`):**
     ```loft
     fn material(hit: SurfaceHit) -> Bsdf  // eval(wo, wi), sample(wo, u), pdf(wo, wi), emission
     ```
  2. **Stylized Surface Contract (`Style`) — works on ANY integrator that populates a `LightSummary`:**
     ```loft
     struct LightSummary {
       key_dir: d3@World,        // dominant light direction
       key_vis: f,               // 0..1 shadow/visibility term
       key_radiance: rgb,        // direct key radiance
       indirect_irradiance: rgb, // sky IBL in realtime, OR path-traced multi-bounce GI in reference!
       ao: f,
       footprint_m: f,           // explicit pixel-world width (replaces non-portable dFdx/dFdy!)
       depth_m: f,
       id: u,
     }
     fn shade_style(hit: SurfaceHit, light: LightSummary) -> rgb
     ```
  - **What this unlocks honestly:**
    - `woodblock` + `realtime`: gets `key_vis` from the shadow map and `indirect_irradiance` from the baked sky cube.
    - `woodblock` + `reference`: gets `key_vis` from ray-traced soft shadows and `indirect_irradiance` from **real path-traced bounce color bleed**, then quantizes/overprints ink cleanly on top.
    - `woodblock` + `spectral_caustic`: **compile-time contract mismatch** (refused cleanly instead of faked), because spectral caustics don't produce a `LightSummary`.
    - And notice `footprint_m` in `LightSummary` directly solves `std1.md`'s #1 portability bug (`sketch`/`papercraft`/`sea` using fragment-only `dFdx/dFdy` derivatives that break in compute and path passes).

---

## 5. Language: What construct makes the 96k-token corpus impossible to write?

**Rebuttal on critique 5: The 96,204-token median corpus is NOT a language-verbosity bug — it is a data-serialization bug.**

Look at `tok1.md:30–37` **(measured)**:
- Total corpus tokens across 187 scenes: **18,861,784 (measured)**
- Span from first sweep `emit` to final `build` brace: **18,023,476 (95.69% of the entire corpus, measured)**
  - Sampled ring centers & axes: **7,825,650 (41.5%, measured)**
  - Sampled keyframe deltas: **8,213,888 (43.6%, measured)**
  - Row paint & section exponents: **1,617,150 (8.6%, measured)**

As `tok1.md:107` explicitly notes, those 16M tokens are **baked per-vertex float arrays (sampled meshes & keyframes) dumped as ASCII literals into `.loft` source**, whereas the original seed generators were **all `< 5,511` tokens (measured)** before expansion!

**Therefore, a compile-error token cap or repeated-literal lint is the wrong lever:** it punishes symptoms and would reject legitimate large scenes. Three real constructs eliminate the 96k-token failure mode by construction:

1. **Separate binary table assets from `.loft` source (`import_table "rings.bin"` / `.glb` / `.loftb` data tables).** Sampled ring coordinates and keyframe deltas are *binary asset data*, never code. Code stays `< 3,000` tokens; sampled meshes stay in compact binary tables.
2. **Keep procedural scenes procedural (`build for` + row defaults + `ramp_weights`).** Regenerating the 170 corpus ports from their actual procedural generators instead of dumping expanded build outputs drops median scene size from **96,204 → ~1,500–4,000 tokens (predicted)** immediately — and even on existing sampled files, `tok1.md` proved **modal column defaults alone save 985,842 tokens (measured)** with **0 grammar changes**.
3. **Fix `ad.rs` constant-parameter propagation (`tok1.md:102`).** Currently the compiler rejects factoring shared sweep surfaces into a helper function with `E405` because `ad.rs` treats a helper's `const`/row-uniform exponent parameter as varying in `pow`! Allowing uniform/row-constant parameters in `ad.rs` unlocks **112,100 tokens (measured)** of genuine code reuse across 1,121 sweep definitions without any new syntax.
4. **Keep token budgets and repeated-block checks in `loft lint` / corpus CI gates, NOT as hard compiler errors.**

---

## 6. Native-first or web-first? What would you have us give up?

**Native-first, with WebGPU as a strict, first-class portable tier (`HostCaps::WEB_BASELINE`).**

- **Why native-first:** Pinning native to WebGPU's lowest common denominator (8 storage buffers/stage, no ray queries, no bindless, no 64-bit atomics, no matrix cores, no MetalFX) permanently caps Loft at 2019 visuals. Meanwhile, `x2e` just proved browser Three.js embedding already works zero-readback at **16.70 ms p95 / 1.20 ms CPU submission (measured)** on a single shared `GPUDevice`.
- **What we explicitly give up to hold both cleanly (no illusions):**
  1. **Give up bit-identical / feature-identical parity between Native High and Web.** Every scene/integrator declares a capability tier via `HostCaps`:
     - **Web tier:** packed buffers (`≤ 8` storage/stage), software BVH, compute/FSR upscaling, ALU shading.
     - **Native tier (still on `wgpu 30.0.1`!):** unlocks hardware RT (`EXPERIMENTAL_RAY_QUERY`), bindless textures (`MaterialTable`), 64-bit atomics, **Apple M4 matrix cores (`simdgroup_matrix`/`bfloat`) via `PASSTHROUGH_SHADERS` MSL**, and **MetalFX temporal upscaling via `PassRun::HostCallback` + `wgpu_hal::metal`**.
  2. **Give up writing a full `objc2-metal` backend right now.** The 3-rung `wgpu` acceleration bridge above unlocks hardware RT, matrix cores, bindless, and MetalFX in **~600–900 LOC (predicted)** instead of a **2,300–3,850 LOC (predicted)** second executor + dual CI matrix. Revisit a native `MetalBackend` *only* if Loft explicitly commits to SBT RT pipelines, mesh shaders, or multi-queue async compute.

---

## 7. Which of your 8 proposals would I cut, and what is missing?

### Cut or Narrow
1. **Cut Proposal 4's "token budget as a compile error" and "repeated-block lint as a compile error."** Replace with: (a) binary table import for sampled assets, (b) regenerating the 170 seeds as procedural `build for` generators + modal column defaults (**-986k tokens (measured)**), (c) fixing `ad.rs` row-constant params (**-112k tokens (measured)**), and (d) putting token caps in `loft lint` / corpus CI only.
2. **Cut "compiler auto-merging of kinds under a pipeline budget" from Proposal 3.** Ship explicit material-family uber-shaders + `MaterialTable` + bindless first; auto-merging arbitrary ASTs into switch-heavy megashaders risks register spills and compiler bloat.
3. **Narrow Proposal 1's sequencing:** Do **not** launch 4 parallel `lean1` rewrite lanes simultaneously again. Merge **One Execution Path + `wgpu`-free `loft-std`** *first* (deletes ~4k lines cleanly with one owner), then enforce `tools/check.sh lines` per crate as a ratchet so `loft-passes` shrinks monotonically as std moves to `.loft`.
4. **Keep `x2b` motion parity lean — don't ship a slow 900-line `u32` soft-float if a contraction barrier passes.** `x2b` already hit **481/481 Metal ticks (measured)** but missed the 1,000-crab cap at **0.180 ms vs 0.150 ms (measured)** because full software binary32 arithmetic taxed every motion op. Replacing it with a motion-only FMA contraction barrier (keeping shading untouched at `MTLMathModeFast`) should drop 1,000-crab dispatch to **≤ 0.06 ms (predicted)**.

### What Is Missing From Your 8 Proposals
1. **Fixing `shade_tiles` (`hit, px, world` + optional MSAA VisBuf) + wiring `EXPERIMENTAL_RAY_QUERY` into `World`.** This is the concrete engineering fix for Critique #1 / Table B (`58 → < 15 ms (predicted)` uncontended) and unlocks real M4 hardware ray tracing inside `wgpu`.
2. **Implementing the two-tier Picture Gate in code (`crates/loft-cli/src/gates.rs`).** The owner already ruled exactness is opt-in, yet the gate code still punishes stochastic integrators and harmless 1-code rounding diffs (`pf1` unroll). Add explicit `Deterministic(tol)` vs `Stochastic(PSNR/FLIP + temporal)` gate modes so integrators stop rejecting real speedups.
3. **The 3-rung `wgpu` hardware-acceleration bridge (before any `MetalBackend`).** Explicitly roadmap: (i) `EXPERIMENTAL_RAY_QUERY` in `World`, (ii) `PASSTHROUGH_SHADERS` MSL wrapper (`simdgroup_matrix`/`bfloat`), and (iii) `HostCallback` + `wgpu_hal` `MetalFX` upscaler — lifting 3 of the 4 beauty ceilings without leaving `wgpu`.
4. **One conductor (retire parallel bash loops).** Today's measurement noise (`Speed 12/12 → 6/12`, `AB_OLD_VS_NEW` at 100% GPU contention, 7 simultaneous Codex sessions across `cook.sh` + `x2*`) is an orchestration bug. Enforce one central concurrency pool + a quiet-GPU timing lock (blocking all builds/other lanes during Speed/Load scoring) before trusting any further performance gates.
