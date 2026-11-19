# Phase 4 · L3

## Predicted evidence

- The closed box produces 1.50, 1.75 and 1.99998474 over 1, 2 and 16 diffuse bounces at 1,024 samples. Every sampled path stays inside and the cosine factor cancels its PDF.
- The compensated white-metal sphere returns 0.50 at both gate pixels. The GGX single-scatter energy and its multiple-scatter complement sum to one under constant incident radiance.
- ReSTIR DI on 1,000 lights estimates the discrete light sum to within 1%. A reservoir retains the selected target and counts zero-weight proposals. Reused weights account for both the old and new targets.
- A hit on the scale-two mutant has gradient norm 2 against the scale-one recorded bound, hence a red pixel. An ordinary sphere remains near ratio 1.
- The load fixture schedules two bakes and 18 mip dispatches (11 for 2048² plus seven for 128² cube). The first dirty bake runs once. An unchanged watched vector or an unrelated write schedules zero work. GPU timing acceptance is ≤2 ms at load and zero bake work per clean frame.

## Engine and platform milestone

- Engine selection via `use engines::reference`, `use engines::woodblock`, `use engines::spectral_caustic` and render/clip `--engine` on default schedules. The RGB path loop uses cosine-weighted continuation, staged BSDF evaluation, sampled delta lights, and emissive-hit and sky terms. The spectral path uses four stratified heroes, per-wavelength Cauchy/Snell/Fresnel and absorption, and a fixed-point photon deposition kernel. Woodblock uses banded pigment, hatches, an ID outline and paper grain.
- Render and clip select one of eight AOVs during compilation. Depth follows the R0g log encoding, traced Vis packs step count only for the steps diagnostic, and overdraw uses a depth-free additive proxy pass. The fixture's analytic hit stays fixed while a live edit deliberately invalidates its recorded field bound.
- The parser types 2D, 3D and cube bake bodies. Lowering emits a compute pass and an optional `GenerateMips`. The native executor allocates the textures, runs load passes before returning, and runs dirty bakes at the frame head based on watched scalar parameter bits. Mip pipelines cache by format and dimension and bind disjoint levels. The generic shared Bundle runner has yet to adopt the native texture declarations.
- Existing Passes hooks changed in `lib.rs`, `ad.rs`, `defaults.rs`, `emit.rs`, `program.rs`, `provenance.rs` and `sexp.rs`. The lane adds `aov.rs`, `paths.rs`, `path_twin.rs`, the path WGSL fragments and fills `bake_tex.rs`. It extends the existing parser AST, lexer and parser, fills Runner's `mips.rs`, and wires CLI `render.rs`, `clip.rs`, `main.rs`, the shared PNG writer and the native executor. Authored code is in the three engine files, `lib/{post,restir,point_lamp}.loft`, and the two fixtures under `scenes/`. CPU proofs are `tests/engines_cpu.rs`, `tests/reference/engines.rs` and the Runner/CLI unit tests. No frozen or golden files changed.

## Results and remaining integration

- CPU closed furnace (1,024 paths each) was **1.50, 1.75, 1.9999847**. White furnace was **0.4999804** at centre and **0.49995854** at rim, on the analytic compensated-metal closure. This measures the path estimator on CPU, with L2's staged GPU material and joint media still an integration point. The 1,000-light reuse test was **546.326449 / 546.250005**, **+0.0140%**.
- `tools/dev.sh check loft-cli`, 37 focused Passes tests, two mip/clock tests and the CLI AOV test passed. Naga validated all emitted modules for the fixtures and all 15 dimension/format mip variants. Earley and llguidance accepted all eight authored files. The L2-owned media test is still skipped on this branch.
- Reference currently uses the compiler's staged trace/material switch and row bounds. L1's BVH and L2's joint volume integration are separate in the frozen base. Direct-light reservoirs merge independent proposal sets at the hit. Cross-frame history reuse, bloom and temporal post remain open. The path schedule refuses raster-only parametric surfaces.
- Integrator commands under the GPU lock:
  - `loft render scenes/lipschitz_bound_mutant.loft --aov lipschitz_ratio --live mutant_scale=2 --size 256x256 --out out/meta/i4/lipschitz.png`. The sphere should be red. Repeat at the default live value for the control.
  - `loft render scenes/bake_load_fixture.loft --size 64x64 --out out/meta/i4/bake.png`. Check the printed load-bake GPU timing and 20 dispatches. Use `Loaded::frame` command counts to isolate the first dirty dispatch and its four mips, unchanged frames and an update of `density`. A write to `unrelated` should add zero dispatches. The 2 ms target is still unmeasured.
  - Preserve both AOV selection and L2's GPU capability argument in the merged compiler compile entry points. Keep L2's emission and quantization hooks in `emit.rs`. On merging the Phase 2/3 platform lane, preserve both CLI dispatch routes and its live-parameter plumbing.

## Stretch study

- `scenes/spectral_caustic_study.loft` supplies the reference schedule's explicit pass list with the spectral sample, nested glass/liquid/air boundaries, droplets, dark wood, a cubic Bézier ink bake and a thin lens. Wetness darkens the paper and broadens the ink. The material uses `mix_bsdf` to blend both evaluated lobes. The bake and paper evaluate the same stroke SDF. The paper currently evaluates it procedurally, and the baked texture is available to the native host.
- `material_shape.rs` records static lobe and mix structure for WGSL specialization. `check.rs` no longer refuses this lowering. Preserve L2's CPU blend and emission treatment on integration. The study reuses the shelf's integer photon pass and AgX conversion.
- The study passed Naga validation for every emitted module. Its CPU scan confirmed that the bubble center lies inside both transmissive layers, and its compiled shader contains the BSDF mix and integer deposition. The focused Passes run is now **38 passed** and both grammar implementations accepted the study.
- Render under the GPU lock with `loft render scenes/spectral_caustic_study.loft --size 512x512 --samples 2048 --out out/meta/i4/spectral.webp`. GPU convergence, the bake-speed target and diagnostic pixels are integration measurements.
