# Design goal: rendering

Read `docs/PHILOSOPHY.md` first. This is the design goal for the render and light sub meta engines: the interfaces, orchestration and library for writing render and light engines, and the example engines Loft ships in it. This page states what the renderer must be able to do and how it is judged, not how
to build it.

## What it draws, all in one frame

Each object is drawn by the fastest method that is exact for it, chosen per object (or per part), and all methods
share one depth, one light transport and one material model.

- **Analytic intersection** wherever a closed form exists: spheres, capsules, cones, boxes, planes, quadrics, tori,
  analytic sea and terrain where solvable. The cheapest exact answer, and the first choice.
- **Distance fields, demoscene style**: sphere tracing inside bounds, folding and domain repetition (with cell ids),
  smooth blends, relief on surfaces. Many-instance fields use spatial lookup, never an N-way union per step.
- **Triangles and raster**: meshes (imported and generated), analytic surfaces evaluated in the vertex stage
  (sweep, loft, lathe, extrude), instancing with culling and level of detail.
- **Ray tracing**: rays against acceleration structures over triangles and analytic primitives, for shadows,
  reflections, refraction and global illumination, where hardware or compute makes it pay.
- **Volumes and participating media**: fog, cloud, dust, smoke, water; marched or analytic (Gaussian) with one
  transmittance shared with surfaces.
- **Particles and points**: specks, sprays, splats, instanced from functions or simulation.

## Light and beauty

- Physically based materials at the state of the art: energy-conserving microfacet BRDFs, multiple scattering,
  clearcoat, sheen, transmission, subsurface, thin film, anisotropy, where each earns its cost.
- Light transport that converges to correct: direct light with many-light sampling, shadows from every method
  above, image-based and sky lighting, global illumination and reflections, with temporal reuse and denoising for
  real time, and a path-traced reference mode that the real-time mode is judged against.
- Camera and post: exposure, tone mapping, depth of field, motion blur, bloom, film grain, colour grading.
- Stylised looks (toon, ink, woodblock) are compositions of the same pieces, not separate renderers.

## Speed

- **Frame time is the measure**: p50 and p95 frame time at the target resolutions, on the target GPUs.
- **Load time does not grow with content**: shaders shared per kind, instances as data, pipelines cached on disk.
  Load-time precompute is allowed when it buys frame time.
- **Choose per site by measured cost**: analytic vs marched vs rasterised vs traced; inline vs call; unroll vs loop;
  specialise vs share. The author's code does not have to change for the choice to change.
- One path from authoring to GPU: compile in Rust, run on wgpu (Metal, Vulkan, DX12, WebGPU), embeddable on a
  device a game owns.

## Developer experience

- A scene is a few lines; a new primitive, BRDF, light estimator or pass is a few lines more, written as maths or
  WGSL, and plugs into the same depth, lights and materials as the built-ins.
- Every built-in (each primitive's intersector, each BRDF, the light transport, the pass list, the whole default
  renderer) is a public composition the consumer can read, copy and replace.
- Generated code is printable; errors point at the author's line; checks never say something false.
- Assets come in (meshes, textures, materials, motion), pictures and readbacks (depth, ids, picking) go out.

## Not goals

- A scene editor or asset pipeline.
- Back-compat with any existing engine's API.
- Features that cannot be replaced by the consumer.
