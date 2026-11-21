# cap1c · Material families

## Authoring

Declare the data as an ordinary query-free kind. The ID returned by `emit` is typed and stored in a drawable row. The material query reads the record once and returns the same staged BSDF used by the CPU evaluator, tiled shading and World. The family is an authored function. The compiler sees a closed lobe graph when the family is already known.

```loft
use stdlib::materials::families::{standard_pbr}
use stdlib::geometry::analytic::{ray_sphere}
use stdlib::sdf::solid::{sphere}

kind MaterialTable @World (base: rgb, rough: f, metal: f, grain: f, seed: f) {}
kind Ball @World (center: p3@World, material_id: MaterialTable) {
  visible(ro, rd) = ray_sphere(ro, rd, center, 0.5m)
  trace(p) = sphere(p - center, 0.5m)
  material(p, n) {
    let m = MaterialTable[material_id]
    standard_pbr(m.base, m.rough, m.metal, coords(p - center), m.grain, m.seed)
  }
}
build {
  let ceramic = emit MaterialTable(base: [0.7, 0.22, 0.08], rough: 0.35, metal: 0, grain: 4, seed: 9)
  emit Ball(center: [0, 0, 0]@World, material_id: ceramic)
}
```

## Predictions before the run

The fixed camera sees the same 1,000 analytic balls at 1280 × 800 for all counts. Four closed queries use identical geometry and classify masks. Each material contributes one 64-byte record. Traditional specialization would need one module and shade pipeline per used kind. Charge 10 ms to each additional distinct pipeline in the initial cold estimate and 10 µs to each additional active tile dispatch. The 100/1,000 specialization estimates are extrapolations since E520 caps primary/shadow kinds at 32. A data-row edit queues exactly 64 bytes and creates zero pipelines.

| Unique materials | Table bytes | Per-kind shade pipelines | Closed family shade pipelines | Predicted family shade p95 | Predicted full frame p95 |
| ---: | ---: | ---: | ---: | ---: | ---: |
| 10 | 640 | 10 | 4 | 1.12 ms | 2.20 ms |
| 100 | 6,400 | E520 (100) | 4 | 1.16 ms | 2.24 ms |
| 1,000 | 64,000 | E520 (1,000) | 4 | 1.26 ms | 2.34 ms |

The 2.20→2.34 ms prediction is a 6.4% increase. Shader text and cold keys at a common nonce should be identical across row counts for the closed families. Eleven raster, shade and fixed pipelines at 10 ms each predict 110 ms serial device compilation. Stage 0 adds one small record per unique material and always emits 1,000 drawable rows. I expect <1 s for the total cold load. The plan for the 200-material PBR/ink scene should have two shade pipelines. Shuffled IDs preserve coverage. Their incoherence can raise the shade p95 without affecting classify or raster. A quiet paired run should show ≤0.30 ms between generated and hand-written family shading, if both controls use identical data, sampling and closure. A 95% interval crossing a target leaves that target unresolved.

The Tier 0 control is the dirty geometry preview and promotion on a single traced kind. Its prior +0.24 ms measurements motivate a 0.2–0.5 ms prediction on the supported control. The visual comparison requires the same promoted geometry. Tier 0 is outside the material shade stage.

## Compiler and host contract

A query-free kind referenced by a drawable material query gets a storage binding for its packed rows. The kind ID used for `Runner::upsert_rows` is reported in `loft show --plan --json`, along with word offsets. Drawable kind and row IDs retain the Vis64 layout. The native API also accepts the table's name through `Loaded::upsert_materials`. The arena has the capacity of the emitted table. Materials can fill reserved rows with data uploads, and a capacity change currently requires loading a new bundle.

The shader family belongs to the material function called by each drawable's query. The six functions live in `stdlib/materials/families.loft` and use the existing staged lobe compiler. Several drawable kinds can use the same family, with a specialization per draw kind in the current plan. Dielectric transmission uses the spectral integrator's optics path. Direct lighting evaluates a reflective surface response.

Hosts that enable both native texture-array features can provide a complete binding array through `Runner::bind_external_textures`. The runner checks the array length and texture shape before replacing that resource. The portable authored fixture uses two independently mipped atlas pages and caps its sampled LOD at 2. The host fixture uses two `texture_2d_array` pages and the same indexed texels on the binding-array route. Route selection belongs to the host or authored schedule.

The two-family path uses the reference integrator's cosine proposal and PDF with the staged BSDF's cosine-weighted evaluation. The structural edit fixture replaces a bundle between encoded frames, using the runner's resource epoch to identify the replacement. Tier 0 remains the geometry preview and promotion route.

## Reproduction

From the lane checkout, after the lab build, run:

```sh
tools/gpu_lock.sh tools/quiet_gpu.sh uv run --no-project python tools/cap1c.py target/lab/loft /Users/nicholasbardy/git/loft/out/meta/cap1c
```

The driver records the source overrides, per-scene shader text and plans, cold keys, raw frame and pass samples, paired intervals, exported bundles and WebP stills. The hand control writes its pigment and band operations in WGSL and uses the same staged lobe compiler for evaluation. The 100/1,000 specialized entries are E520 controls.

## Baseline controls

The archived `v15_exec` CLI accepted the ten-kind control and reported 13 modules. It returned E520 for 100 and 1,000 specialized kinds and for 33 kinds with distinct sphere radii. At the proposal's 10 µs scheduling slope and 10 ms serial compile cost, the two uncapped extrapolations are 2.00 ms / 1.00 s and 11.00 ms / 10.00 s for shade p95 / cold shader compilation. Both grammar recognizers accepted the six-family library and all three fixtures after the family function names were put in Loft's lowercase syntax.

## Build and plan observations

The third compile completed the optimized workspace build, including the CLI and test executables. The 10-, 100- and 1,000-material plans each contain 1,000 drawable instances, four shade entries and seven modules. All seven shader hashes agree across the three counts. The PBR/ink scene uses two shade entries for 200 materials. The six-family scene uses six shade entries. The candidate also exported the paged-atlas and two-family transport bundles. The lane includes `origin/main` at `78ef244`. The merge brought in documentation and conductor changes, so these binaries still describe the same renderer and fixtures.

Generated shaders match the archived unified runtime across all seven configurations in `engines/gates.json`. The recipe sources and frozen pictures are intact. Static checks passed, including the Keel dependency pins, corpus provenance and 310/310 cases in both grammar recognizers.

The first locked host texture probe hit a capacity error before binding textures. Its output arena reserved sixteen words with the shared fixture's maximum of eight. The source now sets the maximum to sixteen. The optimized build predates that test-only correction, and the three-compile limit leaves the corrected probe unscored.

## M4 material timings

At 1280 × 800, each paired arm measured 100 frames after 30 warm-up frames. Four AB/BA pairs per comparison used fresh shader nonces for cold loads. All 24 paired arms passed the 0.150 ms triangle p95 limit. The quiet guard reported no build interference. The table pools each matching pair of observations. Intervals resample pairs and ten-frame blocks.

| A → B | Classify p95 Δ, 95% interval (ms) | Shade p95 Δ, 95% interval (ms) | Frame p95 A → B (ms) | Frame p95 Δ, 95% interval (ms) |
| --- | ---: | ---: | ---: | ---: |
| 10 → 1,000 materials | −0.017 [−0.030, −0.003] | −0.015 [−0.024, −0.006] | 6.938 → 6.474 | −0.464 [−0.647, −0.192] |
| Hand WGSL → generated, 1,000 | +0.018 [−0.009, +0.028] | +0.011 [−0.004, +0.021] | 6.918 → 6.933 | +0.015 [−0.435, +0.408] |
| Sorted → shuffled, 1,000 | −0.013 [−0.022, +0.005] | +0.001 [−0.020, +0.009] | 6.955 → 6.924 | −0.031 [−0.718, +0.280] |

The 10 → 1,000 frame growth interval was −9.30% to −2.77%, within the 10% limit. Generated Shade overhead held below +0.30 ms. At whole-frame scope the interval crosses +0.30 ms. The hand control shares the staged lobe compiler and differs in the hand-written WGSL pigment and band helpers. At 100 rows the standalone profile measured 0.221 ms Classify, 4.552 ms Shade and 6.853 ms whole-frame p95. The ten-kind specialization measured 0.218, 4.900 and 7.041 ms. Its lobe mix differs from the four-family control, so it illustrates the cost of independent specialization and supplies no direct matched delta.

The 1,000-row profiles measured 82 ms median source compilation, 710 ms median cold device load and 72 ms median warm device load. Source compilation plus cold device load ranged from 773 to 827 ms over twelve observations, with a 96.1% sign interval of 778–811 ms for the median. Device acquisition is outside both timers. The eleven device pipelines stayed fixed. Summed creation durations overlap because the runner creates pipelines in parallel. The ten-kind specialization recorded 1,150 ms cold device load and 132 ms source compilation. The 200-row fixture loaded in 528 ms including source compilation. The absolute shade and load predictions underestimated the observed costs.

The isolated budget observations use the seven-module, eleven-pipeline 1,000-row plan. Applying the Capsules-1 module ceiling of four with the pipeline ceiling held at eleven exceeded only modules. Applying the Capsules-1 pipeline ceiling of five with the module ceiling held at seven exceeded only pipelines. E520 still rejects the 33 distinct radii and the 100- and 1,000-kind controls. The plan reports runnable specialized kinds alongside the explicit family entries.

The first measurement invocation captured all 29 timing profiles before this FFmpeg build failed to select a WebP encoder. The capture tool now uses `cwebp -q 95`. The `--reuse-timings` invocation scores the same profiles and renders the stills.

## Host and workspace checks

The optimized workspace suite ran 33 test executables and recorded 180 passed, three failed and one ignored. The portable atlas matched source and host bundle pixels for both ID orders. The two-family path matched source and host pixels with both zero and two bounces, and two bounces increased the center red channel. The cosine proposal and its PDF are shared by both families in the reference integrator. The source compiler refused an invalid path return type with E003 and a numeric material ID with E108. During structural replacement, both queued frames retained their instance and visibility IDs, with a higher resource epoch and different color in the promoted bundle. CPU and GPU material lookup matched.

The live row upload passed the before/after visibility and depth comparison and created zero pipelines. The test observed a changed pixel, then panicked when it addressed the 4-byte packed ID plane as 8-byte Vis64. The corrected test reads one u32 per pixel. That test and the corrected host texture probe postdate the third compile. A separate source-data edit changed 145 pixels at 640 × 400, all on the selected kind and row. Its shader text and ID AOV matched the control exactly. Direct binding-array and array-layer execution remain unverified.

The existing Tier 0 geometry test promoted with 0/140,000 pixels above 1/255 and 0.084 ms hot upload. Its measured +0.564 ms p95 overhead exceeded the +0.500 ms threshold. This test runs a dirty geometry tape in its compute fixture. The family shaders use compiled staged lobes in raster and World.

## Pictures

All ten fixture stills are saved at WebP quality 95. The 10-, 100- and 1,000-row fixtures and the shuffled 1,000-row fixture had identical 1280 × 800 depth AOVs. The hand and generated 1,000-row beauty PNGs were byte-identical.

All 19 frozen image files matched their pinned RGBA hashes. All 19 source renders fit the existing Pictures tolerance of 1,024 pixels above 2/255. Eighteen renders matched the archived `v15_exec` captures exactly. In the remaining Coral Shrine frame, repeated baseline and candidate renders varied by 40 and 35 pixels respectively. Fresh baseline/candidate pairs differed by 24–51 pixels. The largest candidate difference from the frozen Shrine frame was 67 pixels.

[Timing samples, shader plans, bundles, workspace log and stills](../../../out/meta/cap1c/verified/)
