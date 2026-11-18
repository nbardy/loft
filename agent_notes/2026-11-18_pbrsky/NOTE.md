# PBR ambient occlusion and moving sun

## Writing a scene

```ts
style: pbr()                         // Default field AO and sky projection
style: pbr({ occlusion: () => f(1) }) // Open sky on every body
sun: movable([-0.6, 0.65, -0.65])    // The host supplies the direction
sun: vec3(mul(cos(mul(time, Math.PI)), -0.6), 0.65, -0.65)
```

The [ceramic relief](../../examples/materials/occlusion.ts) shows contact inside two channels cut in one signed-distance
field. The [disc](../../examples/materials/moving_sun.ts) uses a custom sky with the sun behind its face. The two
endpoints are visually amber and blue. The thin sunlit bevel helps separate the disc from the background. The
[front relief with AO](ao.webp), [control](no_ao.webp), [oblique relief](relief.webp), [amber](amber.webp) and
[blue](blue.webp) are 384×384 WebP q95 from raw GPU frames.

## Ambient occlusion

The default subtracts the hit's signed distance from four samples along the geometric normal. The offsets are
1/8, 1/4, 1/2 and 1 times 9% of the tracing proxy diagonal. Each deficit is
`clamp((h - (d(p + h*n) - d(p)) - pixel/2) / h, 0, 1)`. Weights of 0.08, 0.16, 0.24 and 0.32 leave at least 20%
environment visibility. A flat surface gives zero deficit. Near a groove wall, the wall caps the measured distance
at its lateral separation from the ray. At the front camera's tested floor position, this predicts 20–40% display
darkening under ACES. The hit subtraction removes a constant march bias and the pixel term suppresses subpixel
deficits.

The style applies the slot's clamped value to the diffuse and specular environment before adding direct light
and emissions. A traced field gives its distance expression to the style at the hit's level of detail. The full
field is available at the taps, including beyond the bounds of the brick's union mask. Meshes, meshed fields, copies
and custom tracers use the named `unoccluded` variant. The field approximation sees self-occlusion within one
authored field. Baked ambient lighting or a custom `occlusion(ctx)` can supply a different visibility law.

At 256×256, a 3×3 patch near the wall averaged **(112.7, 105, 98)** with AO and **(145, 137, 129)** with the slot
set to 1. Display luma dropped **23.2%**. The flat face gave **(145, 137, 129)** both ways. A front-lit,
black-sky control gave identical RGB with either slot.

At 512×512 on Metal through Dawn, alternating six rounds of 30 frames gave a median increase of **0.066 ms**.
Four pairs increased by one timer tick, one tied, and one decreased by one tick after a change in GPU clock
state. The derived cost is five field evaluations per shaded fragment and **0 bytes** of added GPU buffers.
The comparison uses the front camera and swaps only the occlusion part.

## Moving sun

The fixed path keeps the 1,024 Fibonacci samples and nine RGB harmonics. The changing direction runs the same
quadrature in one expression loop. The compiler's existing rate lowering puts the coefficients in the shared
GPU frame-values table, including in built artefacts. A sky independent of the sun keeps its static projection.
A custom sky takes motion through its `sun` argument. The built-in projection rejects captured time, params,
curves and tables in the sky law by name. A GPU-only sky can supply `clear` and its own environment. The
direction expression needs an above-horizon CPU-evaluable default at time 0. Cloud lighting keeps that default.

The disc's unlit face should approximate its sky's chromaticity. With a unit normal toward +Z, the diffuse kernel
integrates `0.7 + 0.3*d.z` to 0.9. Accounting for the 0.95 sRGB base and the small rough specular term predicts
**(115, 56, 19)** and **(21, 49, 119)** in 8-bit RGB. The measured patches were **(115, 55, 18)** and
**(20, 49, 119)**. The same loaded GPU artefact followed time and the same host-input artefact followed both
supplied directions. The fixed and moving representations agreed within one display level.

At 512×512, the moving example's GPU pose pass was **0.328 ms** in five of six rounds and **0.393 ms** in one.
The median increase across paired frames was **0.360 ms**. The artefact declares **176 bytes** of frame storage
against **16 bytes** for the fixed version, which includes the mandatory minimum binding. The nine RGB coefficients
account for **144 bytes** of padded storage. The example declares one GPU map and zero CPU tables. The fixed control
sets `sun: [-0.6, 0.65, -0.65]`. Both comparisons use `loaded.frameGpu(view, 30)` and report the least GPU timestamp
of the 30 frames in each round after warming the pipelines. The timer tick was 0.065536 ms.

## Verification

`pnpm typecheck`, the nine PBR tests, and CLI `check`, `build` and `render` on both examples passed.
`render --artefact` at t=1 produced the same frame hash as the moving-sun source (`c004e8af5ea3870e`).
The file-loader closure contains zero compiler modules.

The first `bun test --timeout 120000` run logged a 180-second timeout in the root-example pipeline sweep.
That sweep passed in isolation in 85.33 seconds. Both complete-suite attempts received SIGTERM (exit 143)
before printing final counts. A subsequent targeted check exited 75 with the Mac's shared test lock held by
the suite owner. The existing style-import audit reports five missing exports.
