# Integration and hand-off: follow-up TODO (20 Nov 2026)

How Loft gets into other engines (Three.js, Unity, glTF pipelines), from today's owner and lead conversation.
Ordered by value per cost. Each item is a lane with an acceptance test. The cook planner picks them up from OPEN.md
(see the pointer there).

## Background

- **What can't be handed off as data.** A Loft frame is a schedule of passes: cull, raster to the vis-buffer, tile
  classify, shade tiles, post. The path tracer, woodblock and toon swap passes. Those passes *are* the renderer, so
  a host gets them only by embedding Loft.
- **What can.** Assets, clips, pure shader functions and surface materials hand off. So does live motion, as a pose
  stream or as the CPU-twin library.
- **What exists.**
  - Native: the runner attaches to a host wgpu device, with zero-copy bind/export (`crates/loft-runner/src/interop.rs`),
    proven in Keel (Room Runners first light, branch only).
  - Web: `WebGpu::attach(GPUDevice)` and `load_bundle` (`crates/loft-std/src/web.rs`).
  - glTF import (Phase 5).

## 1. Embed: Three.js end-to-end demo (first; cheap, mostly built)
- **Build:** a minimal page on Three's `WebGPURenderer`, with `.loftb` loaded through the WASM runner on
  `renderer.backend.device`. Three draws first, then Loft runs its schedule into the same colour target against
  Three's depth. One light's parameters are passed from Three to Loft.
- **Accept:**
  - a Loft hermit crab and a Three cube occlude each other correctly in both directions;
  - one shared GPUDevice, no readback;
  - frame time reported;
  - it runs in Chrome on M4.
- **Note:** WebGPU only, no WebGL path.

## 2. Embed: Unity spike on Unity's own Metal device (small; decides the Unity design)
- **Build:** a native plugin that takes Unity's MTLDevice and command queue (`IUnityGraphicsMetal`), wraps them as a
  wgpu device through wgpu-hal's create-from-hal path (unsafe), and renders one Loft kind into a Unity
  RenderTexture.
- **Accept:** pixels from Loft appear in a Unity scene with no copy between devices. Otherwise, a written reason it
  can't work.
- **Fallback design if the spike fails:** Loft on its own device renders to shared textures (IOSurface on Mac,
  shared handles on Windows), a URP render feature composites them with depth, and fences keep the two in sync.

## 3. Embed: lighting consistency adapter (with 1 and 2)
- Pass the host's main lights (and optionally sky/IBL) into Loft params, and export Loft's emitters back. A small
  per-host adapter.
- **Accept:** a light moved in the host visibly lights Loft objects the same frame.

## 4. Hand off assets: `loft export --gltf` with a loss report
- **What it exports:**
  - meshes and analytic shapes, tessellated;
  - SDFs, meshed on the GPU with dual contouring or marching cubes at a chosen resolution;
  - standard materials, as glTF PBR plus clearcoat, sheen and transmission extensions;
  - procedural textures, baked with `bake_tex`;
  - lights, as `KHR_lights_punctual`;
  - authored animation, baked as skins plus clips;
  - optionally vertex animation textures, for deformation that is not skeletal.
- **The loss report:** every approximation or drop is listed, in Loft's honest-verdict style. That includes stylised
  integrators, spectral and custom light transport, live motion and constraints, and SDF detail below the meshing
  resolution.
- **Accept:** a hermit crab exported to GLB opens correctly in Blender, Three.js and Unity, with the loss report
  matching what is visibly missing.

## 5. Hand off materials: surface functions for host shaders
- Export pure functions (BSDF lobes, noise, procedural textures, SDF eval) and surface materials (position, normal
  and uv in; albedo, roughness, normal and emission out) through naga's backends:
  - a TSL `wgslFn` for Three.js;
  - an HLSL Custom Function for Unity Shader Graph;
  - a Custom node for Unreal.
- **Accept:** one Loft procedural material renders in Three.js and Unity within a stated tolerance of Loft's own
  render under matched lighting.

## 6. Hand off motion: the CPU twin as a library
- Package a scene's deterministic motion (body, state, laws, solvers) as a native and WASM library:
  `step(dt, inputs) -> joint transforms`. A host drives its own skeleton with it, so motion stays live and reactive
  while the host owns rendering.
- On a shared device, the alternative is to stream the GPU pose buffer (`export_buffer`) to the host's skinning.
- **Accept:** a Unity C# script steps the crab scramble through the library, and the joint transforms match Loft's
  GPU run bit for bit (the twin contract) over 481 ticks.

## 7. Merge Room Runners first light (owner's call)
- Keel branch at `~/git/_wt/keel-loft-rr1`. Open a PR to Keel or the Room Runners repo, and score the GPU p95 that
  was left unscored (Metal returned too few usable timestamps).

## References
- [backends and escape hatches](../../docs/proposals/2026-11-20_backends_and_escape_hatches.md)
- [07 platform extensions](../../docs/spec/07_PLATFORM_POWER_EXTENSIONS.md) (attach, bind/export)
- [Room Runners study](room_runner/STUDY.md)
- [notes and TODO](NOTES_AND_TODO.md)
