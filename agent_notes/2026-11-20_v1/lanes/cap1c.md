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
