# X2c · Hermit sky irradiance

**ESCALATE.** The native compiler and CLI can produce this dirty cubemap.
Authored Shade has no way to read it on base `1194779`. The prerequisite is
typed access to the baked texture and a filtering sampler in the consuming
Shade pass. That requires changes outside the lane’s authored file scope.

## Design and prediction

At each 64² cubemap texel, sum the existing 12 Fibonacci directions’ linear
dome color times the clamped cosine to the texel normal. Keep the existing
0.85 exposure and 4π/12 solid-angle weight. The base costs 294,912 dome
evaluations and the full RGBA16F mip chain occupies 262,128 bytes. The
expected clock cost was seven dispatches on invalidation and zero steady
state. Diffuse would sample mip zero at the world normal and apply the
existing 1/π factor in Shade. The glossy response needs a radiance map and
its roughness-dependent filter. The diffuse convolution has already removed
the detail needed for the narrow coat. The direct sun,
shadow, emission and background would keep their original evaluators.
Budgeting 0.30–0.45 ms for the replacement sky terms against PF1’s 1.542 ms
ablation predicted 1.09–1.24 ms of Shade savings and visible changes over
1/255. That forecast remains untested.

## Measured

Apple M4 Metal, 1280 × 800, Hermit-20, 30 warmups and 300 timed orbit frames
per arm with the ring timer under the GPU lock. The triangle controls were
0.096/0.096 ms against the 0.350 ms contention cutoff.

| GPU ms | Main | Bake-only probe |
| --- | ---: | ---: |
| Shade p50 | 2.248 | 2.257 |
| Shade p95 | 2.883 | 2.851 |
| Frame p50 | 4.024 | 4.059 |
| Frame p95 | 4.990 | 4.799 |
| Steady commands | 13 | 13 |

The probe declared `bake_tex SkyIrradiance cube 64x64 rgba16float on
dirty(sun_x, sun_y, sun_z) mips(true)`. The actual texture had six layers
and seven levels. The first frame, a clean frame, a `sun_x` edit and a
clean frame recorded **24 / 13 / 20 / 13** commands. The initial shadow
pass accounts for four of the extra first-frame commands. The scalar
parameter edit tests invalidation of the probe; the existing schedule
supplies sun direction at Stage 0.

Three lossless 1280 × 800 beauty comparisons at 0, 4 and 9 s were byte
identical, with **0 pixels over 1/255** for the bake-only probe. The q95
baseline stills are in `out/meta/x2c/approval/`. No integrated after frame
exists for owner review. No references were changed
or re-approved.

## Missing binding

The bake emitter only declares a storage output for its own dispatch.
The native loader builds Shade with Vis64, HDR, tile lists, shadow
depth and a comparison sampler at group 2 slots 0–4. It creates the baked
textures separately, after preparing Shade. Portable program lowering
also rejects programs with bakes pending typed resource descriptors.

The probe added a reachable `textureSampleLevel` to the generated Ground
shader with `texture_cube<f32>` and `sampler` declarations at group 2
slots 5 and 6. Naga validated the module. wgpu reported
a validation error during native loading at `Device::create_compute_pipeline` for Ground.
`ResourceBinding { group: 2, binding: 5 }` was absent from the pipeline
layout. This held even with a successfully allocated native bake resource.
The next lane needs to carry a bake table reference into Shade and bind
its sampled cube view and filtering sampler before preparing the pipeline.

The clean workspace check, exact source SHA, probe sources, raw profiles,
binding diagnostic and image hashes are archived in
[`out/meta/x2c/`](../../../out/meta/x2c/).
