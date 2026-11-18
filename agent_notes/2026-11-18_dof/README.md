# Depth of field and film grain

The copper-sphere study now focuses on the front of the hammered ball, with a circular aperture and fine monochrome
grain. The reference and refocused stills show the same geometry, lighting and camera at 720×405.

- [Reference](baseline.webp)
- [Depth of field and grain](refocus.webp)

## Scene code

```ts
const look = pbr();
const style = {
  ...look,
  post: [...look.post, depthOfField({ focus: 1.75, focalLength: 0.16, fStop: 1.8 }), filmGrain()],
};
```

The two functions return `PostSpec`, so order and composition work like the other style passes. `focus` is distance
along the forward axis. The example uses world units for both distances and renders through the existing host and
manifest. The accumulated still lens draws many aperture views around the camera's target, while this lens uses
the resolved image and depth from one view. Use one lens per image.

## Optical model and measurements

The thin-lens law gives an image distance `v` from `1/v = 1/f - 1/z`. The blur footprint at the focused image
plane `vₛ` is a disc with radius `pupil × |1 - vₛ/v|`, with `pupil = f/(2N)`. The camera's focused projection
scale supplies `vₛ` in pixels. Substitution gives signed pixel radius `fy × pupil × (1/S - 1/z)`. The orthographic
projection uses a telecentric cone with slope `pupil/S`. The reciprocal-depth law and scatter-as-gather are inherited.
Loft gathers locally for the receiving layer and estimates source coverage in the same pass with area-stratified
antipodal taps. Both gathers use matched colour and depth texels. [Hammon, Practical Post-Process Depth of Field](https://developer.nvidia.com/gpugems/gpugems3/part-iv-image-effects/chapter-28-practical-post-process-depth-field).

The 320×720 GPU chart predicted an 8.001 px radius on the far knife edge. The measured line-spread variance gave
7.416 px, within the 0.8 px tolerance. At the sharp depth edge, the two neighbouring pixels measured 64 and 223
out of 255, the two card colours. The chart also verified partial coverage on both sides of a defocused silhouette
in the near and far fields, and zero foreground spill past its footprint. Midgrey grain at amount 0.08 measured
5.85 byte levels RMS against 5.89 predicted. A frame repeats byte-for-byte at the same time after a different time.
A separate orthographic chart measured 7.416 px against 8.000 px predicted for the telecentric lens.

At 1280×720 on the Apple M4 GPU (Dawn/Metal), `loft profile` over 24 frames measured 3.342 ms/frame for depth of
field and 0.098 ms/frame for film grain, using the profiler's best four-frame block for each pass. The normal frame
with both effects took 4.19 ms GPU in that run. Three separate eight-frame timing blocks gave best normal frames
of 4.00 to 6.03 ms with the effects and 1.25 to 3.47 ms without the chain. The machine's load varied during the
run, so individual pass attribution and best whole frames give distinct timings.

Derived memory at 1280×720 is 13.18 MiB of extra single-sampled textures over the example with no post chain. The
existing schedule allocates two RGBA8 intermediates (7.03 MiB), R32F scene depth (3.52 MiB), and three R8 clearance
attachments (2.64 MiB). The passes share those textures throughout the chain. Each blurred pixel does up to 73
depth loads and 73 colour loads. In-focus pixels skip the 32 receiver taps. Film grain does one colour sample.

The example passed `check` at 109,474 triangles against a 110,000 triangle budget and built a version 1 artefact.
At 720×405 both the source and files-only renders reported RGBA hash `1021d163030e6735`. The four-sample still
also rendered with both passes over the average. The two comparison WebPs were encoded at quality 95 directly
from lossless PNG renders.

`pnpm typecheck`, the two lens GPU tests and the artefact loader closure check passed. The full-suite process
received SIGTERM after reaching `grad.fd.test.ts`. Before the interruption, the top-level example pipeline test
hit its 180 s timeout and the heron cache check took 125 ms against a 50 ms limit. Both passed on individual
reruns. The resumed run also received SIGTERM. The test harness then refused further runs (exit 75) because a
separate suite holds the Mac's shared test lock. Full-suite verification remains open.

The optional style-import audit still reports its four existing imports (`calmSea`, `albedoOf`,
`horizonClear`, `leaves`). The audit now permits the internal `SceneError` diagnostic used by the new constructors.

## Limits

Fog, clouds, spray, sea and the built-in flat ground take the resolved opaque depth behind them, or infinity at
clear depth. A single opaque depth and a composited colour can only approximate visibility at partially covered
edges and behind a defocused silhouette. The 16 px radius cap at 720p and the fixed tap count bound the work. Wider
blur and independent transparent-layer focus would require more scene information or multiple aperture views.
