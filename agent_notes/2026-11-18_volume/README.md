# `draw.volume`

Loft can now march an arbitrary extinction expression inside a world-space box. The dust example combines an
ellipsoidal envelope with drifting billows and finer noise. The folded variant iterates four coordinate folds and
lights the resulting seams with a blue lamp. The material and style are separate from the density expression.

```ts
draw.volume("dust", (p, t) => max(0, noise3(sub(p, vec3(t, 0, 0)))), {
  bounds: { min: [-3, 0, -2], max: [3, 4, 2] },
  steps: 64,
  shadow: { kind: "march", steps: 6 },
  albedo: [0.98, 0.72, 0.46],
  g: 0.64,
});
```

The renderer reuses the soft medium's 12-triangle box and the host's opaque-depth resolve. It marches uniform
strata along the visible ray and hashes the pixel for a stable offset. Each cell is integrated analytically under
constant extinction and source colour. Opacity can end the march at `T ≤ 1/1024`. The shadow march takes midpoint
samples of the same density to the light or the box exit. Volume styles see phase-weighted arrivals and separate
visibilities. Toon bands the shadow tint, woodblock chooses inks by luminance, and the default adds emission to
single scattering. Beer–Lambert attenuation and the phase function are inherited. The integration with the light
fold, swappable looks and existing transparent frame is specific to Loft. [Henyey and Greenstein (1941)](https://doi.org/10.1086/144246)

## GPU measurements

Apple M4 10-core GPU, Dawn/Metal, `game` at 1280×720 and t = 1. The normal frame column is the least GPU time of
25 warm frames. The draw column is the best block mean in the separate attribution pass. The dust scene uses one
sun, two gradient noise octaves per density evaluation, and no pose tables.

| Variant | View samples | Shadow samples per lit cell | Normal ms/frame | Dust draw ms |
|---|---:|---:|---:|---:|
| `dust64Clear` | 64 | 0 | 19.92 | 20.86 |
| `dust128Clear` | 128 | 0 | 41.55 | 45.48 |
| `base` | 64 | 6 | 61.01 | 61.52 |
| `dust128` | 128 | 6 | 145.36 | 129.50 |

```sh
bun src/cli/main.ts profile examples/volume_dust.ts --variant base --view game --size 1280x720 --time 1 --frames 25
```

The derived bound is `P*N*(1 + S*L)` density evaluations for `P` covered pixels, `N` view samples, `S` shadow
samples and `L` lights. The approximation that doubling `N` would double frame time was close in the unshadowed
case (2.09×). With shadows the measured ratio was 2.38×. The shadowed 128-step dust is a still-quality setting on
this GPU. Per volume there are zero dedicated textures, storage buffers or per-frame tables. For a scene that
first enters the split path because of this volume, the derived texture payload is 6.45 MB at 720p (one full-size
`r32float` depth resolve and three `r8unorm` clearance textures), before driver alignment and allocation overhead.

## Verification

- Two GPU tests compare output to analytic Beer–Lambert and Henyey–Greenstein solutions within two 8-bit levels.
  The tests cover a slab clipped by opaque geometry, a camera inside the box, animated extinction, bright emission,
  the finite lamp distance, forward and backward scattering, and repeated-frame equality.
- Typecheck, example `check` for all five variants, and `build` and `render --samples` for dust and folded fog
  passed. The dust folder loaded without the compiler and matched all 921,600 RGBA bytes at 640×360.
- Rebuilding `examples/shell.ts`, which has no volumes, produced a byte-identical folder against the branch baseline.
- The full suite reached 124 passes before its process was terminated at 15 minutes. The all-examples pipeline
  check had timed out at 180 seconds and passed in isolation in 80.92 seconds. Resuming from the interrupted pose
  file yielded 44, 48, and 49 passes across the three batches. Across all 64 files, 264 distinct tests passed.
- A concurrent macOS test-lock change appeared in `tests/setup.ts` during the resumed checks. It came from
  outside this lane.
- The style-import audit reports five pre-existing imports in papercraft and PBR. The loader closure check passed.
- Stills are 640×360, four samples each, encoded from PNG with `cwebp -q 95`.
  [Dust](dust.webp) · [Folded fog](folded.webp) · [Woodblock](folded-woodblock.webp)

## Limits

Submit separate transparent draws in back-to-front order. Interpenetrating volumes, Gaussian media and glazed
surfaces need per-ray interleaving for correct compositing. Self-shadowing only integrates the current field.
Opaque lamp shadow maps can darken samples through the existing light fold. The sea clip uses the existing
above-water approximation. Rust/wgpu execution was outside this verification run. The file bundle uses existing
passes and WebGPU blend factors and keeps its format version.
