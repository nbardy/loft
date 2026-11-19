# Loft platform power extensions

This specification takes precedence over [PATCH_V1_2.md](PATCH_V1_2.md), [06_ADDENDUM.md](06_ADDENDUM.md)
and 00–05 where they overlap. Step 0 freezes the shared ABI and grammar. Phase 1.5 keeps its existing scope.
The rollout in §9 assigns implementation and acceptance gates.

The outbox intrinsic is `emit_event(...)`. The keyword `emit` continues to construct rows in the build stage.

## 1. Step 0 ABI and grammar

Step 0 freezes the [execution declarations](../../crates/loft-ir/src/bundle.rs), the
[backend interface](../../crates/loft-runner/src/backend.rs) and the [normative grammar](loft.lark).
The [golden vectors](../../crates/loft-abi/tests/golden.rs) specify field offsets, visibility lanes
and deterministic output bits. For Philox spatial hashes, floor each coordinate to a signed cell
and use its two’s-complement bits. The upper 24 output bits divided by 2²⁴ give a uniform f32 in [0, 1).

### 1.1 `loft.lark`

Add `struct_decl`, `type_alias`, `kernel_decl` and `bake_tex_decl` to the alternatives in `top_item`.
The typed kernel keeps the capability and determinism rules in PATCH_V1_2 §3.

```lark
struct_decl: "struct" UIDENT "{" (_NL? col (_SEP col)* _SEP?)? "}"
type_alias:  "type" UIDENT "=" type_expr
kernel_decl: "kernel" LIDENT "(" params? ")" ("allow" "(" ident_list ")")? block
// Include struct_decl, type_alias, kernel_decl in `top_item`.

```

The texture syntax in §5 and the existing optional `param` range use these productions.

```lark
param_decl:  "param" LIDENT ":" type_expr "=" expr ("in" expr ".." expr)?
bake_tex_decl: "bake_tex" UIDENT bake_tex_dim LIDENT "on" bake_tex_clock "mips" "(" ("true" | "false") ")" "(" LIDENT ":" type_expr ")" block
bake_tex_dim: GRID_DIM | TEX_DIM_3D | "cube" GRID_DIM
bake_tex_clock: "load" | "dirty" "(" ident_list ")"
TEX_DIM_3D: /[0-9]+x[0-9]+x[0-9]+/
```

Cube dimensions must be square. `dirty(...)` names live parameters. A `param` declares a typed default and
may declare bounds with `in lo..hi`. Its runtime representation and range enforcement are in §3.2.

### 1.2 Core types

`HostCaps.features` carries the full `GpuFeatures` mask. Freeze the declarations below in `loft-ir` and
the records in §§3–4 in Step 0. Bind groups 1–3 are derived per pass. PATCH_V1_2 §1 defines the fixed group 0.

```rust
bitflags::bitflags! {
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
    pub struct GpuFeatures: u32 {
        const TIMESTAMPS          = 1 << 0;
        const PIPELINE_CACHE      = 1 << 1;
        const SUBGROUPS           = 1 << 2;
        const F16                 = 1 << 3;
        const INT_ATOMICS         = 1 << 4;
        const MULTI_DRAW_INDIRECT = 1 << 5;
        const BINDLESS_TEXTURES   = 1 << 6;
        const RAY_QUERY           = 1 << 7;
        const PACKED_INT8_DOT     = 1 << 8;  // WGSL dot4I8Packed / dot4U8Packed
        const COOP_MATRIX         = 1 << 9;  // reserved for subgroupMatrix / simdgroup_matrix
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ShaderTarget { NagaIr, Wgsl, Msl, Spirv, Ptx, Wasm }

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum QueueClass { Graphics, AsyncCompute, Transfer }

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ElemFormat {
    F32, F16, Bf16, F64,
    U32, I32, U16, I16, U8, I8,
    PackedU4, PackedI4, PackedU2, Fp8E4M3, Fp8E5M2,
    Struct(u32), // index into Bundle.struct_layouts
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TableLifetime {
    Static,
    DynamicArena { max_rows: u32 },
    Transient,
    PingPong,
    External,    // adopted zero-copy from host via bind_external_*
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum BindAccess {
    ReadStorage, ReadWriteStorage,
    SampledTex, StorageTex, BindlessTexArray, Sampler,
}

pub struct PassBinding {
    pub group: u8,         // 1 = RO storage, 2 = RW storage, 3 = textures/samplers
    pub slot: u32,
    pub resource: TableId,
    pub access: BindAccess,
}

pub enum DispatchExtent {
    Fixed([u32; 3]),
    FromRowCount { table: TableId, workgroup_x: u32 },
    Indirect { buf: TableId, offset_bytes: u32 }, // 12B DispatchIndirectArgs
}

pub enum PassRun {
    Compute { module: ModuleId, entry: String, dispatch: DispatchExtent },
    ComputeFusedTicks { module: ModuleId, entry: String, max_ticks: u32, instances: TableId },
    RenderBundle { draws: Box<[IndirectDraw]>, color_targets: Box<[TargetDesc]>,
                   depth: Option<DepthDesc>, msaa: u8 },
    PassLoop { max_iters: u32, cond_buf: Option<(TableId, u32)>, body: Box<[PassDecl]> },
    GenerateMips { texture: TableId },
}

```

`TableDecl` carries its `ElemFormat`. `Struct(u32)` indexes `Bundle.struct_layouts`.
`TableLifetime::External` marks storage adopted from the host (§3.1).
`PassRun::GenerateMips` dispatches the mip chain in §5. `COOP_MATRIX` reserves the cooperative matrix capability.

In `loft-runner/src/backend.rs`, freeze the `ExecutionBackend` trait over the execution surface specified in
PATCH_V1_2 §6, including attach, bundle load, row updates, encoding, CPU stepping and external memory interop.
`WgpuBackend` is the sole v1.0 implementation. `ModuleBlob.target` uses `ShaderTarget`.

`loft-passes` desugars domain operations to the shared IR. The Layer 0/1 domain-word gate must find no matches.

```sh
rg -i -w 'foot|gait|creature|trunk|pbr|shadow|sea|splat|mpm' crates/loft-{runner,ir}
```

## 2. Storage precision and `std::quant`

Storage format is set by `TableDecl`. Packed values live in ordinary 32-bit-aligned `u32` storage buffers.
The scalar compute types for quantized arithmetic are the existing `f` (`f32`), `f16`, `u` (`u32`) and
`i` (`i32`) and their vectors. Quantized widths are storage views. PATCH_V1_2 §7 governs the default
`f32` precision, gated `f16` compute and the platform limits on `f64`.

| Element format | Packed view | Portable lowering |
| --- | --- | --- |
| `Bf16` | `bf16x2`, two 16-bit lanes per word | Widen each lane with `bitcast<f32>(x << 16u)`. Pack with round-to-nearest-even. |
| `I8`, `U8` | `i8x4`, `u8x4`, four bytes per word | Use WGSL packed dot operations when supported. Otherwise unpack, multiply and sum in 32-bit registers. WGSL pack/unpack operations include `pack4xI8` / `unpack4xI8` and their unsigned counterparts. |
| `PackedU4`, `PackedI4`, `PackedU2` | `u4x8`, `i4x8`, `u2x16` | Extract lanes with `extractBits`, sign-extending `i4`. Apply group scale and zero point when dequantizing in registers or workgroup memory. |
| `Fp8E4M3`, `Fp8E5M2` | `fp8x4`, four lanes of the selected format | Decode with bit operations or a 256-entry workgroup lookup table to `f32` or gated `f16`. |

`std::quant` exposes packed operations over `u` words. `dot4_i8(a, b)` returns an `i` and
`dot4_u8(a, b)` returns a `u`. Under `GpuFeatures::PACKED_INT8_DOT` they lower to `dot4I8Packed` and
`dot4U8Packed` respectively. The unpack-and-dot lowering gives the same integer sums on devices without
that capability. `unpack_bf16x2` returns the low and high halfwords widened to `v2`.
`unpack_u4_lo` and `unpack_u4_hi` each return four lanes as `v4`, from the low and high 16 bits
respectively, in order of increasing bit position. The example applies the scale after unpacking.

```loft
# Table stores 8x u4 per u32 (plus per-group f16 scale); kernel unpacks or hits DP4A/MMA
let w_packed: u = weights[idx]               # 8x uint4 packed in one u32
let acc: i = dot4_i8(a_i8x4, b_i8x4)         # lowers to WGSL dot4I8Packed / native DP4A
let vals: v2 = unpack_bf16x2(pair_u32)       # lowers to (x << 16) on wgpu, native bfloat on MSL/PTX
let q8: v4 = unpack_u4_lo(w_packed) * scale  # 4-bit dequant in registers

```

The same storage layout can feed native bfloat, packed dot or matrix instructions on future
`Msl`, `Ptx` and `Spirv` targets. The v1.0 runner uses the portable wgpu path and needs only
its ordinary buffer types.

## 3. Zero-copy host interop and live parameters

### 3.1 External resources and bind-group invalidation

The host and runner share GPU handles on the attached device. A buffer must meet the table's minimum
size and required usages. A texture must match format, dimensions, sample count and required usages.
An incompatible binding returns `RunnerError::AbiMismatch`. Export exposes Loft GPU handles for direct
host binding.

```rust
// crates/loft-runner/src/interop.rs (~45 LOC)
impl Runner {
    /// Replaces a TableId's backing storage with a host-owned wgpu::Buffer.
    /// Validates size >= table.min_bytes and required wgpu::BufferUsages; returns RunnerError::AbiMismatch otherwise.
    pub fn bind_external_buffer(
        &mut self, inst: InstanceId, table: TableId, buf: Arc<wgpu::Buffer>,
    ) -> Result<(), RunnerError>;

    /// Replaces a texture TableId (e.g. host DepthBuf, live video frame, heightmap) with a host wgpu::Texture.
    /// Validates format, dimensions, sample_count, and TextureUsages.
    pub fn bind_external_texture(
        &mut self, inst: InstanceId, table: TableId, tex: Arc<wgpu::Texture>,
    ) -> Result<(), RunnerError>;

    /// Exports an Arc handle to a Loft-allocated GPU buffer (e.g. POSE, compacted vertices, MPM particles)
    /// so Three.js StorageBufferAttribute or Keel render passes can bind it directly.
    pub fn export_buffer(&self, inst: InstanceId, table: TableId) -> Option<Arc<wgpu::Buffer>>;
    pub fn export_texture(&self, inst: InstanceId, table: TableId) -> Option<Arc<wgpu::Texture>>;
}

```

A successful `bind_external_*` or `resize_table` increments `inst.resource_epoch` and stamps the affected
table with that epoch. Before encoding a pass, if any of its `PassBinding` entries references a table
with `table.epoch > cached_bg.epoch`, rebuild the affected pass's bind group. Pipeline creation remains
independent of the resource epoch. Rebuild a cached `RenderBundle` only when its vertex or index buffers change.

### 3.2 Live `param` reflection

Every `param` occupies a slot in `ParamsBuf` at group 0, binding 2, with a metadata entry in `Bundle.params`.
The reflected metadata covers the declaration's type, default, optional range and byte offset. Finite values
are clamped to their declared range when present. The next encode uploads the dirty parameter shadow buffer
in one write and schedules any dependent procedural bakes.

```rust
pub struct ParamMeta {
    pub id: ParamId,
    pub name: String,
    pub ty: IrTy,          // F32 | Vec2 | Vec3 | Vec4 | U32 | Bool
    pub offset_bytes: u32, // std430 offset in ParamsBuf
    pub default: [f32; 4],
    pub range: Option<([f32; 4], [f32; 4])>,
}

impl Runner {
    pub fn params(&self, inst: InstanceId) -> &[ParamMeta];
    /// Clamps value to declared `in lo..hi` (or returns Err on NaN/Inf), updates shadow buffer,
    /// marks `params_dirty = true` for single `queue.write_buffer` upload at next `encode_into`.
    /// 0 ms compile cost; triggers `Clock::OnDirty` procedural texture re-bakes if dependent.
    pub fn set_param(&mut self, inst: InstanceId, id: ParamId, val: &[f32]) -> Result<(), RunnerError>;
}

```

## 4. Sparse event outbox and single-pixel picking

### 4.1 `OutboxBuf`

The default 16 KiB allocation holds a 16 B header and 511 32 B event slots.
The records use the following `std430` layout.

```rust
#[repr(C)]
pub struct OutboxHeader {
    pub write_count: u32,  // atomic<u32> incremented by GPU; reset to 0 after staging copy
    pub capacity: u32,     // max slots (e.g. 511)
    pub dropped: u32,      // atomic<u32> overflow counter (never silently drops)
    pub frame_idx: u32,
} // 16 bytes

#[repr(C)]
pub struct OutboxEvent {
    pub tag: u32,          // interned symbol ID (e.g. `FootPlant`, `ClawHit`, `WaveBreak`)
    pub instance_id: u32,
    pub site_id: u32,
    pub tick: u32,
    pub payload: [f32; 4], // e.g. world impact pos.xyz + impulse magnitude
} // 32 bytes

```

The intrinsic is legal in `law`, `solver` and `kernel`.

```loft
emit_event(tag: Sym, instance_id: u, site_id: u, payload: v4)

```

Lower the intrinsic to an atomic slot claim. Slots at or above capacity increment the reported overflow count.

```wgsl
let slot = atomicAdd(&outbox.write_count, 1u);
if (slot < outbox.capacity) { outbox.events[slot] = OutboxEvent(tag, inst, site, frame.tick, payload); }
else { atomicAdd(&outbox.dropped, 1u); }

```

Use three `MAP_READ` staging slots. At frame end, when a pass bound the outbox, `encode_into` copies it to
`staging[frame % 3]` and clears `write_count` with `clear_buffer`. The header identifies the producing frame
and reports drops for that frame. `Runner::drain_events(inst) -> (Vec<OutboxEvent>, u32 /*dropped*/)`
asynchronously takes the completed slot for `frame - 2`, without blocking GPU work.
The populated prefix contains `min(write_count, capacity)` events.

### 4.2 Picking

Picking reads sample 0 of the requested pixel and unprojects it with the frame's inverse view-projection matrix.

```rust
pub struct PickResult {
    pub hit: bool,
    pub kind_id: u32,
    pub row_id: u32,
    pub payload: u32,       // packed (u,v) | tri_id | cell_hash
    pub depth_ndc: f32,
    pub world_pos: [f32; 3],
}

impl Runner {
    /// Copies 1 texel (sample 0) from VisBuf (8 B) + DepthBuf (4 B) into a 256-byte staging buffer
    /// (aligned to wgpu::COPY_BYTES_PER_ROW_ALIGNMENT), unprojects `world_pos` via `FrameUniform.inv_vp`,
    /// and invokes `cb` once mapped.
    pub fn pick_async(&mut self, inst: InstanceId, px: [u32; 2], cb: Box<dyn FnOnce(PickResult) + Send>);
}

```

`pick_sync` returns the same `PickResult` for headless callers such as `loft show --pixel x,y` by waiting
immediately with `device.poll(Maintain::Wait)`.

## 5. Procedural texture baking and mip generation

`bake_tex` declares a 2D texture, a 3D volume or a cube. Its body evaluates the texel from the typed
coordinate. The load clock runs once during `load_bundle`. The dirty clock re-evaluates at the head of
`encode_into` when a watched parameter changes. `mips(true)` schedules `PassRun::GenerateMips` after
the bake.

```loft
# Bakes once at load (or re-bakes in <1 ms when a referenced `param` changes):
bake_tex InkMask 2048x2048 r16float on load mips(true) (uv: v2) {
  eval_cursive_sdf(uv)
}

bake_tex SkyIbl cube 128x128 rgba16float on dirty(sun_dir) mips(true) (dir: d3@World) {
  sky(SceneWorld, dir)
}

bake_tex CloudNoise 64x64x64 r8unorm on load mips(false) (uvw: v3) {
  fbm3(uvw * 4.0)
}

```

```rust
pub enum TexDim { D2(u32, u32), D3(u32, u32, u32), Cube(u32) }

pub struct TextureBakeDecl {
    pub table: TableId,
    pub dim: TexDim,
    pub format: wgpu::TextureFormat,
    pub clock: BakeClock, // Load | OnDirty(Box<[ParamId]>)
    pub generate_mips: bool,
    pub module: ModuleId,
    pub entry: String,
}

```

Cache the mip compute pipeline by texture format class (`Rgba16Float`, `R16Float`, `Rgba8Unorm`) and
dispatch the dimensional variant with workgroups of `8x8x1` for 2D, `8x8x6` for cube layers or
`4x4x4` for 3D. For each level `k = 0 .. mip_count - 2`, `textureLoad` reads level `k` with
a 2×2 box filter (2×2×2 for 3D) and `textureStore` writes level `k+1` through a storage view.
Mip generation runs with the bake. A clean texture incurs zero bake or mip work per frame.

## 6. AOVs and static shader telemetry

### 6.1 Diagnostic AOVs

Schedules producing `VisRec` or `Hit` support
`--aov beauty|albedo|normal|depth|id|sdf_steps|lipschitz_ratio|overdraw` on `loft render` and
`loft clip`. The selected mode compiles as an alternate present or shade pass using the same scene.

| Mode | Output encoding |
| --- | --- |
| `beauty` (default) | Tonemapped sRGB. |
| `albedo` | Linear, unlit base albedo. |
| `normal` | `0.5 * n_world + 0.5`. |
| `depth` | 8-bit log distance matching R0g. |
| `id` | Deterministic hash color of `(kind_id, row_id)`. |
| `sdf_steps` | False-color `steps / max_steps`, blue through green to red. |
| `lipschitz_ratio` | `‖∇f(p)‖ / L_derived` at the hit. Red above 1.0 identifies a violated bound. Deep blue below 0.3 indicates a locally loose bound. |
| `overdraw` | Additive `1/16` per proxy fragment. |

### 6.2 `ModuleTelemetry`

Collect static counters during `loft build` and `loft profile --json`. The collection runs on
the CPU and is predicted to take under 1 ms.

```rust
#[derive(Serialize)]
pub struct ModuleTelemetry {
    pub module_id: String,
    pub wgsl_lines: u32,
    pub alu_ops: u32,            // Binary/Unary/Math add, mul, fma, dot, cross
    pub transcendentals: u32,    // Sin, Cos, Tan, Exp, Log, Pow, Atan2, Sqrt
    pub storage_loads: u32,      // Expression::Load from AddressSpace::Storage
    pub storage_stores: u32,     // Statement::Store to AddressSpace::Storage
    pub texture_samples: u32,    // Expression::ImageSample / ImageLoad
    pub workgroup_bytes: u32,    // Sum of AddressSpace::WorkGroup global variable sizes
    pub max_loop_depth: u32,     // Static nesting depth of Statement::Loop
    pub branch_count: u32,       // Statement::If + Statement::Switch
    pub call_sites: u32,         // Catches accidental 5x BSDF/field inlining blowups!
}

```

Walk each entry point's function body and the transitively called functions. Inspect statement bodies
and expression arenas once per handle in each function. A statically known loop bound may also
contribute to `weighted_alu_est`. `loft profile --json` includes `modules: Vec<ModuleTelemetry>`
alongside `gpu_pass_ms`. The counters measure emitted IR structure. GPU pass timings measure
execution.

### 6.3 Why Loft emits `naga::Module` directly

Naga is wgpu's Rust shader translation and validation library. Its typed `Module` IR can be written
as MSL for Metal, SPIR-V for Vulkan, HLSL for DX12, GLSL for GL or WGSL for browser WebGPU.

```text
  [Frontends]                       [Middle]                         [Backends]
  WGSL text  ──┐                                                ┌──► MSL    (Apple M4 Metal)
  SPIR-V     ──┼──► naga::Module ──► naga::valid::Validator ────┼──► SPIR-V (Vulkan)
  GLSL       ──┤    (typed IR)       (safety & capability       ├──► HLSL   (DX12)
  Loft IR!   ──┘                      proofs + spans)           ├──► WGSL   (Browser WebGPU + `loft show`)
                                                                └──► GLSL   (WebGL2 fallback)

```

`loft-passes` lowers Loft IR directly to `naga::Module`. Native wgpu consumes it through
`wgpu::ShaderSource::Naga` with the `naga-ir` feature, skipping the generated WGSL text round trip.
`naga::valid::Validator` checks emitted modules in CPU-only lanes, and the same arenas supply telemetry.
The WGSL writer serves WebGPU and `loft show`. The reserved native targets allow future backends to
use instructions outside Naga's supported shader surface.

## 7. Batched multi-world execution (POST-v1.0, Phase 5.5)

This section is reserved for Phase 5.5. Do not implement it in the v1.0 pipeline.

```rust
impl Runner {
    pub fn load_bundle_batched(&mut self, bundle: &Bundle, n_envs: u32) -> Result<BatchInstanceId, RunnerError>;
    pub fn push_input_env(&mut self, batch: BatchInstanceId, env_id: u32, slot: InputId, tick: u64, val: InputValue<'_>);
    pub fn set_param_env(&mut self, batch: BatchInstanceId, env_id: u32, id: ParamId, val: &[f32]) -> Result<(), RunnerError>;
    pub fn reset_env(&mut self, batch: BatchInstanceId, env_id: u32, seed: u64) -> Result<(), RunnerError>;
}

```

- `TableLifetime::Static` tables are shared by all environments. Dynamic arenas, ping-pong tables
  and mutable state, pose, parameter, input and verdict tables allocate `n_envs * env_stride` bytes,
  with `env_stride` equal to the per-environment stride.
- `FrameUniform` carries `env_count: u32` and `atlas_grid: vec2<u32>`. Compute derives `env_id`
  from `GlobalId.y` or `GlobalId.x / rows_per_env`. Every mutable table index is offset by
  `env_id * env_stride` in bytes.
- One compute wave culls `(env_id, row_id)` against `cameras[env_id]` and compacts visible pairs.
  One `RenderBundle` projects with that camera, maps NDC to atlas tile
  `(env_id % cols, env_id / cols)` and clips fragments to the tile. The views share a tiled 2D atlas
  or a 2D array texture. For example, 64 cameras at 256² fit in an 8×8 atlas of 2048².
  The batch shares shader modules and pipelines across environments.

## 8. Deterministic `.loftlog` record and replay

A log stores the bundle identity, device signature, fixed step, initial seed, parameter snapshot,
checkpoint cadence and input trace. Binary version 1 uses magic `LOFTLOG1`. JSON is available with
`--format json` when recording.

```rust
#[derive(Serialize, Deserialize)]
pub struct LoftLog {
    pub format_version: u32,           // 1
    pub bundle_hash: [u8; 32],         // BLAKE3 of .loftb or canonical source graph
    pub device_signature: String,      // adapter name + driver version (flags cross-device tolerance mode)
    pub dt_s: f32,                     // fixed step (e.g. 1.0 / 120.0)
    pub initial_seed: u64,
    pub params: Vec<(String, [f32; 4])>,
    pub checkpoints_every: u32,
    pub inputs: Vec<LoggedInput>,      // sorted by (tick, seq): Level | Event | PosePacket
    pub expected_final_identity: Option<[u8; 32]>,
}

```

- `loft render scene.loft --record bug.loftlog` captures host inputs and live `set_param` calls.
  Record parameter changes in the timed input trace so replay applies them in `(tick, seq)` order.
- `loft render scene.loft --replay bug.loftlog --out repro.png` replays bit-identically on the same
  device. On a different device it verifies within the applicable Tier tolerance.
- `loft check scene.loft --replay bug.loftlog` reruns Truth verdicts along the recorded trace.
  A regressed hold exits with code `2`.
- Phase 5 also retains the standalone `.loftb` load gate from PATCH_V1_2 §4 and 06 §14.
  The runner binary must load the built bundle without linking `loft-passes`.

## 9. Phased rollout and pass/fail checks

Keep the Step 0 wasm32 check active on every merged wave. GPU acceptance runs in the integration
gates. Lanes use CPU fixtures and Naga validation.

| Extension | Phase and brief | Owner crates or grammar | Predicted LOC | Pass/fail check |
| --- | --- | --- | --- | --- |
| Tier A ABI locks (§1 and shared records in §§3–4) | Step 0 · `s0` | `loft-ir`, `loft-runner`, `loft.lark` | ~180 | `cargo check --target wasm32-unknown-unknown -p loft-runner` passes, `loft.lark` parses every `.loft` file, and the core domain-word gate finds zero matches. |
| Live `param` reflection and zero-copy bind/export (§3) | Phase 2 · `l23a`, `i23` | `loft-runner`, `loft-passes` | ~90 | Sweeping roughness and sun parameters costs 0 ms compile. A host write to an external `wgpu::Buffer` is visible in the next frame with zero CPU copies. |
| Static Naga telemetry (§6.2) | Phase 2 · `l23a`, `i23` | `loft-passes`, `loft-cli` | ~95 | `loft profile --json` reports exact ALU, transcendental, load and workgroup counts on a known module. The gate catches a synthetic 5× BSDF inline mutant. |
| Outbox and picking (§4) | Phase 3 · `l23b`, `i23` | `loft-runner`, `loft-passes` | ~110 | Boss Hit 1 drains the exact `ClawHit` and `FootSlip` events with `dropped == 0`. Washer picks return the oracle's exact `kind_id` and `row_id` for the 9,664- and 12,080-sample fixtures. |
| `std::quant` (§2) | Phase 4 · `l4b`, `i4` | `loft-passes`, CPU twin, `std::quant` | — | The CPU twin matches expected packed-dot and unpack results. Naga validates the emitted dot and unpack intrinsics, including the packed-dot capability and portable paths. |
| AOVs, bake clocks and mip generation (§§5–6) | Phase 4 · `l4c`, `i4` | `loft-runner`, `loft-passes`, `loft-cli` | ~220 | The synthetic Lipschitz-bound mutant renders red where `lipschitz_ratio > 1.0`. A 2048² procedural mask and 128² IBL cube bake and generate mips once at load in ≤2 ms total (predicted), with 0 ms/frame of baking and mip generation thereafter while clean. |
| `.loftlog` and zero-compiler `.loftb` load (§8) | Phase 5 · `l5g`, `i5` | `loft-runner`, `loft-cli` | ~120 | The recorded crab scramble replays with a 0-bit state diff. A standalone runner loads the `.loftb` without linking `loft-passes`. |
| Batched worlds (§7) | Phase 5.5 · **POST-v1.0** | `loft-runner`, `loft-ir`, `loft-passes` | ~240 | Future gate. Render 64 parallel crabs into a 2048×2048 atlas with the same GPU command count as one environment (≤19 commands) and ≥8× aggregate throughput versus sequential runs (predicted). |
