# Mixed precision

`--precision mixed` is an opt-in for scene builds and renders. It also works with `frames`, `clip`, `profile` and
`dev`. Pass `"precision":"mixed"` in a `loft serve` request. A mixed build writes format 4 with f32 descriptors and
the alternate shader chunks and buffer initializers. The loader checks the device once and applies the changes
together. An adapter without the feature selects `f32-fallback`. On a capable device the caller can force f32
from the same directory. Every successful serve answer carries the precision alongside `tier` and the frame hashes.

```sh
bun src/cli/main.ts build examples/mixed_precision.ts --precision mixed --out out/copper
bun src/cli/main.ts render --artefact out/copper --views three --out out/copper.webp
```

The scene's paint reads `tabulate("copper_paint", fn, 256, "static-shading")`. This schedule permits binary16 storage
for a fixed shading lookup. Samples are validated in `[-1, 1]`. The lookup widens on read, with its index and
interpolation in f32. Adjacent f32 tables retain their alignment and binding. The same schedule can hold a normal
map or bounded material channels. The renderer also uses native half arithmetic for the bloom mask's colour
product, grain's luminance and the soft medium's light visibility weights.

## Precision and cost

Binary16 samples in `[-1, 1]` incur at most `2^-12` absolute storage error before interpolation. A sample drops from
16 to 8 bytes before any trailing alignment. The copper palette therefore drops from 4,096 to 2,048 bytes.
The f32 and mixed versions share their bindings and draw recipes. The combined artifact stores both
initializers, 6,144 bytes for the palette on disk.

All geometry, camera matrices, marcher distances, time and depth use f32. Geometric normals and sharp BRDF terms
keep f32. At the minimum GGX roughness, the term `1 + (n·h)^2 (alpha^2 - 1)` is only `6.25e-6` at a normal incidence.
Rounding the cosine to half there can make it exactly one well before incidence, while evaluating the subtraction
in half can make the denominator vanish. Medium source buffers keep f32 for positions, inverse radii, seeds,
optical depths and HDR light. The current format interleaves the bounded albedos with those values. The medium's
f16 light weights factor out the slowest exponential in f32. The half sums have a constant floor, so converting
the remaining bounded terms to half cannot zero the light tail before HDR scaling. A CPU sweep of optical depth
from 0 to 1,000 in steps of 0.01 with IEEE half rounding found relative errors up to 0.117% for sunlight and
0.104% for skylight. Recurrences
keep their static layout for replay and bake even when they read an eligible shading lookup.

## Apple M4, Dawn Metal

1280×800 at 1 s, 31 paired GPU timestamp samples, alternating order after four discarded rounds. Each timestamp
sum covers the ordinary pose and render passes. It excludes CPU work, readback and the wait between submits.
The timestamp quantum was 0.065536 ms. The Mac was shared during the run. Time and lossless pixel results come
from [bench.ts](bench.ts). Each WebP q95 pair shows f32 on the left and mixed on the right at 384×240 per panel.

| Scene | GPU median f32 → mixed, ms | GPU p10–p90 f32 / mixed, ms | GPU buffer bytes f32 → mixed | Total logical bytes f32 → mixed | Max RGB delta / 255 | Changed pixels |
|---|---:|---|---:|---:|---:|---:|
| [Serpent with paper grain](creature.webp) | 3.539 → 3.539 | 3.277–5.571 / 3.211–4.063 | 110,868 → 110,868 | 52,334,868 → 52,334,868 | 1 | 841 |
| [PBR copper with bloom](pbr.webp) | 5.767 → 5.767 | 2.294–9.568 / 2.753–8.323 | 72,976 → 70,928 | 52,296,976 → 52,294,928 | 2 | 9,046 |
| [Open scene with Gaussian puffs](medium.webp) | 3.801 → 4.260 | 0.983–15.270 / 0.983–20.578 | 95,960 → 95,960 | 48,223,960 → 48,223,960 | 1 | 1,310 |

Logical memory is derived from the selected buffer descriptors plus attachments, the output and the readback buffer.
It excludes driver padding, compiler memory, temporary timestamp allocations and shader register use. Every alpha
byte matched. The creature and PBR medians tied. The medium had a higher mixed median and the same p10 in a
noisy run. The reduced lookup is the measured benefit of mixed mode, so f32 stays the default.

The GPU regression test checks that the compact initializer and its shaders work together, that the colour shift
at 256×160 is at most two levels, and that a device created without the optional feature returns
`f32-fallback` with byte-identical f32 output.

For f32 parity, capture the unmodified Serpent, Material Spheres and Open frames at 640×400 and compare the raw
RGBA against the same views from this change. All three matched byte for byte. The Serpent and Material Spheres
build directories also matched byte for byte. Open uses a JavaScript table and is rendered through the in-memory
host. The f32 WGSL for bloom and grain also matched the baseline exactly.

`loft serve` returned the same mixed RGBA hash for a scene request and a directory request at 256×160. Forcing f32
from that directory returned `precision:"f32"` and the expected f32 hash. The viewer served the mixed bundle.
The Rust/wgpu CLI compiled and rendered the same file with `precision mixed` on Metal. The example's triangle check measured 9,217, and its build, file render and four-sample still passed.

## Full-suite status

The focused GPU regression, typecheck and loader closure passed. The standard full suite reported the all-example
pipeline test's 180-second timeout and the Crab pose test's 120-second timeout before SIGTERM. Crab passed on
an isolated retry in 35.64 seconds. The isolated all-example check also hit its extended 600-second deadline.
A fresh full run received SIGTERM as well. The shared suite lock subsequently refused another test invocation with
exit 75 and directed it to the suite owner. These runs provide no complete suite verdict.

## Remaining

Splitting bounded albedos out of automatic medium tables could reduce their bandwidth. The current mixed path
preserves that buffer layout and quantizes the lighting weights. Native half speed depends on the GPU. This run
covers Apple M4 only.
