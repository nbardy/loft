# Loft v1.2 ABI and plan patch (20 Nov 2026, from the research scientist)

Supersedes the fixed `@group(1..3)` ABI in `02_CORE_ABI_AND_LOWERING.md` and the matching parts of `05` and `06`.
Goal: strip the remaining engine assumptions out of Layers 0 and 1, and replace hard compiler walls with graduated,
honest opt-outs. Then MLS-MPM fluids, Gaussian splatting, Radiance Cascades or open-world streaming can be built
inside Loft.

## 1. Pass-derived bind groups and HostCaps (lands in Step 0)
- Only `@group(0)` is fixed: `0: uniform FrameUniform`, `1: storage<read> InputsBuf`, `2: storage<read> ParamsBuf`.
- `@group(1)` (read-only storage), `@group(2)` (read-write storage) and `@group(3)` (sampled and storage textures,
  samplers, optional bindless array) are derived per pass from `PassDecl.bindings`, and cached by layout hash in the
  runner. Layer 0 no longer knows PoseBuf, ShadowMap or VisBuf: they are library tables. Delete the hard-coded
  4-buffer / 8-texture limits.

```rust
bitflags::bitflags! { pub struct GpuFeatures: u32 {
  const TIMESTAMPS = 1<<0; const PIPELINE_CACHE = 1<<1; const SUBGROUPS = 1<<2; const F16 = 1<<3;
  const INT_ATOMICS = 1<<4; const MULTI_DRAW_INDIRECT = 1<<5; const BINDLESS_TEXTURES = 1<<6; } }
pub struct HostCaps { pub depth_convention: DepthConvention, pub msaa_samples: u8, pub features: GpuFeatures }
pub enum BindAccess { ReadStorage, ReadWriteStorage, SampledTex, StorageTex, BindlessTexArray, Sampler }
pub struct PassBinding { pub group: u8, pub slot: u32, pub resource: TableId, pub access: BindAccess }
pub struct PassDecl { pub id: PassId, pub wave: u16, pub bindings: Box<[PassBinding]>, pub run: PassRun }
pub enum TableLifetime { Static, DynamicArena { max_rows: u32 }, Transient, PingPong }
impl Runner {
  pub fn upsert_rows(&mut self, inst: InstanceId, kind: KindId, start_row: u32, bytes: &[u8]) -> Result<(), RunnerError>;
  pub fn set_row_count(&mut self, inst: InstanceId, kind: KindId, active_rows: u32) -> Result<(), RunnerError>;
  pub fn resize_table(&mut self, inst: InstanceId, table: TableId, new_capacity: u32) -> Result<(), RunnerError>;
  // wired in Phase 3: runs a row-local pass on the CPU twin synchronously (frame-1 gameplay, rollback)
  pub fn step_pass_cpu(&mut self, inst: InstanceId, pass: PassId, rows: Range<u32>, ticks: u32) -> Result<&[u8], RunnerError>;
}
```
`HostCaps` exposes the native capabilities. RenderBundle plus one `drawIndirect` per kind stays as the portable
fallback.

## 2. Fixes
- Boss crab STATE (std430) is **2,928 B**: a 176 B header, 8×80 B feet, 2×32 B manipulators and 64×32 B antennae. It
  is not 2,368 B.
- QUIRKS additions: never iterate a `HashMap` in shader emit (use `BTreeMap`/`IndexMap`). Never pass a
  `binding_array` texture element directly as a WGSL function argument (naga SPIR-V crash; wgpu #6283, #8520).

## 3. Additive surface extensions (land at the Phase 4 freeze; do not block Phases 1.5–3)
- `unproved(step_scale: f) { ... }` inside `trace` or `surface` bypasses E211 and E307, uses explicit step scaling and
  4-tap finite-difference normals, and stamps the kind's truth record `Unproved(AuthorOptOut)`.
- `kernel name(params) allow(atomics, workgroup, subgroups, dynamic_loops) { ... }`: typed GPU compute outside
  `law`/`solver` that keeps spaces, units and typed tables. Integer and fixed-point atomics stay `Det`; float
  atomics tag the table `NonDet`.
- `mix_bsdf(a, b, t)`: a horizontal blend alongside the vertical `+` stack. Both monomorphize statically.
- A custom `schedule` may rebind `type Radiance` (default `rgb`; e.g. `vec4f` for 4-wavelength spectral or Stokes).
- Domain features (`body`, `plan`, `retarget`, `VisRec`, `Bsdf`) lower 100% into Layer 1 Table + Kind + PassDecl.
  There are zero privileged semantics in Layers 0 and 1.

## 4. Phase 5 scope
Drop glTF export (keep import and retargeting). Add the `.loftb` zero-compiler round trip, the wasm32 runner gate
(active from Step 0), and `out/meta/training_contract.json` with the exit-7 canary.

## 5. GPU-driven indirect compute, the ray-query bit, and state-preserving hot swap (Step 0)
- Add `const RAY_QUERY = 1 << 7;` to `GpuFeatures`. It is consumed in Phase 5 (hardware ray queries).
- Extend `DispatchExtent` in loft-ir. `Indirect` unlocks GPU-driven wavefront path tracing, sparse active-brick
  culling and GPU-compacted particle/MPM workgroups, on native and browser alike.
  ```rust
  pub enum DispatchExtent {
      Fixed([u32; 3]),
      FromRowCount { table: TableId, workgroup_x: u32 },
      Indirect { buf: TableId, offset_bytes: u32 }, // 12-byte DispatchIndirectArgs; (0,0,0) is a GPU no-op
  }
  ```
- `Runner::hot_swap_module` (and `hot_swap_bundle`) applies to `law`, `solver` and `schedule` modules as well as
  render kinds. Tier 1 compiles asynchronously (~100 ms). If the `StateStoreDecl` layout hash is unchanged, the live
  `StateBuf` and checkpoint rings survive the swap. They reset cleanly only when the state's fields changed.

## 6. Unconstrained meta-engine seams (reserved in Step 0; their library packs come after v1.0)
Loft's Layer 0/1 substrate is domain-agnostic. Procedural 3D, lighting and creature motion are **Flagship Pack #1**,
not an architectural ceiling. Reserve these four seams in `loft-ir` / `loft-runner` (~70 LOC) without building the
packs that will use them during Phases 1.5–5:

1. **Pluggable backends and targets.** `ModuleBlob { id, hash, target: ShaderTarget, bytes }`, where `ShaderTarget` is
   `NagaIr | Wgsl | Msl | Spirv | Ptx | Wasm`. The runner implements an `ExecutionBackend` trait covering attach,
   load_bundle, upsert_rows, encode_into, step_pass_cpu and so on. v1.0 has `WgpuBackend` only; native Metal, CUDA
   or WASM backends plug in later. External memory interop (adopt and export raw buffers) goes through the same trait.
2. **GPU-driven pass loops and queue hints.** `PassDecl.queue: QueueClass` (`Graphics | AsyncCompute | Transfer`) is
   ignored by `WgpuBackend`. `PassRun` gains `PassLoop { max_iters, cond_buf: Option<(TableId, u32)>, body: Box<[PassDecl]> }`.
   It stops early when `cond_buf[offset] == 0`, which covers CFL substeps, wavefront tracing and Poisson solves. On
   wgpu it unrolls to bounded indirect dispatches, where an empty iteration writes (0,0,0).
3. **Kind-agnostic tables.** A `TableDecl` needs no owning `KindId`. Any library can allocate SoA component arrays,
   brick pools or weight tensors through `TableDecl` + `PassBinding`.
4. **Open IR and strict desugaring.**
   - `Op::Intrinsic { name: SymId, args: Box<[ValId]> }` holds future MMA, atomics and subgroup ops.
   - Every Layer 2 pass (AD, intervals, law fusion, emit, …) is a standalone `Fn(&mut IrModule) -> Result<()>` in a
     pass registry.
   - Every flagship construct (`body`, `law`, `solver`, `plan`, `Bsdf`, `VisRec`) lowers strictly to universal
     `Table + Kernel + Pass` IR in `loft-passes`, and is never special-cased in `loft-runner`.

Deferred packs (`std::fluid` MLS-MPM/CFD, `std::splat` 3D/4DGS, `std::diff` reverse-mode vjp, `std::neural` MMA,
`std::spectral`, `std::ecs`) ship later as ordinary `.loft` libraries and optional Layer 2 passes, with zero Layer 0/1
rewrites. They use the same terse surface: `build fn` constructs tables and rows, and `schedule` wires the passes.
None of them is built during Phases 1.5–5.
