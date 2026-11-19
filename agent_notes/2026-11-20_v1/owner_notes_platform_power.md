# Owner's notes, 20 Nov 2026: quantized storage, agent-power extensions, naga, and the platform-power spec (verbatim)

More details to incorporate

--


<pasted_content id="ac93">
The clean way to support `bf16`, `fp8`, `i8`/`u8`, and `u4`/`i4`/`u2` without bloating `.loft` or breaking `wgpu` portability is to **separate storage precision from compute precision**:

* **Storage (`TableDecl`):** Any packed or quantized format (`bf16x2`, `i8x4`, `u8x4`, `u4x8`, `i4x8`, `u2x16`, `fp8x4`) sits in standard 32-bit-aligned `u32` storage buffers—so Layer 0 (`loft-runner`) needs **zero** special buffer types.
* **Compute (`loft-passes` + `std::quant`):** On `wgpu`/WebGPU, packed loads unpack in registers (or hit hardware 8-bit dot products); on future native `ShaderTarget::Msl` / `Ptx` / `Spirv` backends, the exact same packed tables bind directly to hardware `bfloat` and matrix units.

---

### Hardware Reality: `wgpu` / WebGPU Today vs. Native Backends Later

| Type | Storage Packing | `wgpu` / WebGPU Path (Works Today) | Native M4 Metal / CUDA Path (Future `Backend`) | Best Use in Loft |
| --- | --- | --- | --- | --- |
| **`bf16`** | 2 per `u32` | No native WGSL type yet; unpacks losslessly in 1 bitshift (`bitcast<f32>(x << 16u)`), packs with RNE shift. | Native `bfloat` ALU & `simdgroup_matrix` on Apple M2–M4 (Metal 3.1+); native Tensor Cores on CUDA/Vulkan. | Neural radiance caches, dynamic-range weights, HDR radiance probes without `f16` overflow. |
| **`i8` / `u8**` | 4 per `u32` | Hardware DP4A via WGSL `dot4I8Packed` / `dot4U8Packed` + `pack4xI8`/`unpack4xI8` (native in `wgpu`/WebGPU). | Native `int8` DP4A and `int8` Tensor Core MMA. | Quantized neural shaders, compact SDF bricks, voxel/material IDs, compressed normals. |
| **`u4` / `i4` / `u2**` | 8 (4-bit) or 16 (2-bit) per `u32` | Always packed everywhere (even CUDA/Metal have no scalar 4-bit registers); `extractBits` + group scale/zero-point dequant in registers/workgroup memory. | Same packed layout; feeds dequantized tiles straight into `simdgroup_matrix` / Tensor Core MMA. | Extreme-density neural weights, 3DGS SH coefficients, compressed voxel/SDF octrees, browser ML. |
| **`fp8`** (`e4m3`/`e5m2`) | 4 per `u32` | Software bit-unpack or 256-entry `var<workgroup>` LUT into `f16`/`f32`. | Native FP8 Tensor Cores on Blackwell/Hopper (CUDA/PTX). | High-throughput neural material/physics surrogates on cloud GPUs. |

---

### How It Fits Into Loft v1.2 (Zero Phase 1.5 Churn)

1. **Don't add scalar `u4` or `i8` variables to `.loft`'s syntax.** Nobody wants 4-bit loop counters, and WGSL doesn't have them. Keep scalar registers in `f` (`f32`), `f16`, `u` (`u32`), and `i` (`i32`), and expose quantized types strictly as **packed storage views and dot/dequant intrinsics** in `std::quant`:
```loft
# Table stores 8x u4 per u32 (plus per-group f16 scale); kernel unpacks or hits DP4A/MMA
let w_packed: u = weights[idx]               # 8x uint4 packed in one u32
let acc: i = dot4_i8(a_i8x4, b_i8x4)         # lowers to WGSL dot4I8Packed / native DP4A
let vals: v2 = unpack_bf16x2(pair_u32)       # lowers to (x << 16) on wgpu, native bfloat on MSL/PTX
let q8: v4 = unpack_u4_lo(w_packed) * scale  # 4-bit dequant in registers

```


2. **Add one `ElemFormat` enum to `TableDecl` in `loft-ir` (10 LOC, optional in Step 0):**
```rust
pub enum ElemFormat {
    F32, F16, Bf16, F64,
    U32, I32, U16, I16, U8, I8,
    PackedU4, PackedI4, PackedU2, Fp8E4M3, Fp8E5M2,
    Struct(StructLayout),
}

```


3. **Add `const PACKED_INT8_DOT = 1 << 8;` and `const COOP_MATRIX = 1 << 9;` to `GpuFeatures` in `HostCaps**` so `loft-passes` can emit `dot4I8Packed`/`dot4U8Packed` automatically when supported and fall back cleanly when not.

Everything stays 100% compatible with `wgpu` and browser WebGPU right now, and when you later build a neural shader pack, compressed SDF brick pool, or native Metal/CUDA backend, the exact same `.loft` tables and `std::quant` intrinsics lower straight onto native `bfloat`, DP4A, and matrix hardware without rewriting a single buffer schema.
</pasted_content id="ac93">

--


<pasted_content id="ac93">
Six high-leverage details we haven't covered yet would make Loft dramatically stronger for **agentic RSI loops, creative/physics RL, and real game production**—and none of them bloat the core.

---

### 1. Vectorized Multi-World Batching (`batch_size` / Tiled Atlas Rendering)

Right now `Runner` evaluates one scene instance per `encode_into`, and training runs `loft clip` one process at a time. When you run RL rollouts, evolutionary art search, or batched physics environments, launching 1 GPU submission per world leaves 90% of an M4 or cloud GPU idle.

* **The addition:** Allow `Runner::load_bundle_batched(bundle, n_envs)` to allocate `TableDecl` with an `env_stride`, pass `env_id` into `FrameUniform` / `GlobalId`, and render `N` camera views into a single tiled 2D atlas (or `2DArray` texture) in **one** `RenderBundle` + **one** compute wave.
* **Why it matters:** Stepping 512 physics/creature worlds or rendering 16–64 candidate scene mutations in one GPU pass gives a **10–30× throughput jump** for RL training and automated aesthetic search with zero extra shader modules.

### 2. First-Class Agent Observability (AOVs, Heatmaps & Static Shader Telemetry)

In an autonomous RSI loop, the ceiling is the quality of the feedback signal. Right now an agent gets final pixels, frame time, and motion verdicts—it is still mostly blind to *why* a shader is slow or *why* an image looks off.

* **Standard diagnostic AOVs (`--aov`):** Any `VisRec` or raytraced schedule automatically supports outputting diagnostic buffers on demand: `albedo`, `normal`, `depth`, `kind_row_id`, `sdf_steps` (sphere-trace iteration heatmap), and `lipschitz_ratio` (actual gradient norm vs. derived $L$, immediately exposing where one tiny feature like claw teeth is taxing the whole shape).
* **Static `naga` cost telemetry in `loft profile --json`:** Alongside GPU pass milliseconds, emit per-module counts of ALU ops, transcendental ops, storage loads/stores, texture fetches, and `var<workgroup>` bytes directly from `naga::Module`. Agents optimizing speed stop guessing blindly and know immediately whether a pass is bandwidth-bound, ALU-bound, or step-count-bound.

### 3. Load-Time / On-Dirty Procedural 2D/3D/Cube Texture Baking + Auto-Mipmaps

`scan` currently bakes 1D LUTs at load time, and `bake_brick` bakes sparse SDFs, but there is no general primitive for baking procedural **2D textures, 3D volumes, or cubemaps** with hardware mipmaps.

* **The addition:** Let `TableDecl` mark a `Texture2D | Texture3D | TextureCube` with `Clock::Load` or `Clock::OnDirty` plus `generate_mips: true` (one tiny 40-line compute mip-chain pass in `loft-runner`).
* **Why it matters:** Unlocks prefiltered IBL sky cubemaps, high-res baked calligraphy/wetness/height masks (like the wet-paper ink bleeding in that spectral glass study), 3D noise/cloud lookup volumes, and baked AO—paying ~1 ms once at load and sampling at hardware trilinear speed every frame.



### 4. Zero-Copy External Buffer & Texture Adoption

We made `Runner::attach(device, queue)` adopt the host's `wgpu::Device`, which is half the embedding story. The other half is sharing memory without copying through the CPU.

* **The addition (~30 LOC in `loft-runner`):**
```rust
pub fn bind_external_buffer(&mut self, inst: InstanceId, table: TableId, buf: Arc<wgpu::Buffer>) -> Result<(), RunnerError>;
pub fn bind_external_texture(&mut self, inst: InstanceId, table: TableId, tex: Arc<wgpu::Texture>) -> Result<(), RunnerError>;
pub fn export_buffer(&self, inst: InstanceId, table: TableId) -> Option<Arc<wgpu::Buffer>>;

```


* **Why it matters:** Keel can hand Loft its live depth buffer directly; Three.js can bind a Loft-computed `POSE` or vertex buffer straight into a `StorageBufferAttribute`; and an external PyTorch/MLX/CUDA neural model or wave solver (via shared native handles on a future backend) can write directly into a Loft `TableId` at zero copy cost.

### 5. Sparse GPU-to-Host Event Outbox + Single-Pixel Picking

Full-table async readbacks (`read_table_async`) are the wrong tool for game triggers and interactive editors. A game doesn't want to download 800 KB of `POSE` every frame just to know when to play a footstep sound or register a sword clash.

* **The addition:**
* **`OutboxBuf`:** A tiny fixed-cap ring buffer (e.g., 4 KB) where any `law`, `solver`, or `kernel` can emit sparse structured events (`emit_event(tag, instance_id, payload_vec4)`) via an atomic counter, drained on host via `Runner::drain_events(inst)`.
* **`Runner::pick(inst, px)`:** Copies a single 12-byte pixel from `VisBuf` + `DepthBuf` to return `(kind_id, row_id, world_pos, local_uv)` for interactive editing, ray-clicking, and agent spatial queries.



### 6. Zero-Compile `param` Sweeps & Portable `.loftlog` Replay Files

`param` is already in `loft.lark` and `ParamsBuf` already sits in `@group(0)`, so make it a first-class exploration and debugging tool:

* **Live parameter reflection (`Runner::params()` / `set_param()`):** Lets UI sliders or an optimization agent sweep lighting angles, roughness, stiffness, camera FoV, or wave amplitude at **0 ms compile cost** (even faster than Tier 0) while respecting declared `in lo..hi` bounds.
* **Deterministic `.loftlog` traces:** Serialize `(bundle_hash, param_snapshot, initial_seed, input_event_ring)` into a tiny (~2–20 KB) binary/JSON log. Any rare physics glitch, failed Truth verdict, or standout creative rollout becomes a 1-command exact reproducer (`loft render scene.loft --replay bug.loftlog`).

---

### Where These Fit (Without Touching `chain3.sh` Today)

| Addition | Effort | When to Wire It |
| --- | --- | --- |
| **External buffer/texture bind** (`bind_external_*`) + **`set_param`** | ~45 LOC in `loft-runner` | Easy Step-0 or Phase 5 Keel hook (zero compiler impact). |
| **Static `naga` cost counters** in `loft profile --json` | ~80 LOC in `loft-passes` | Phase 2 or 4 (immediate win for optimization agents). |
| **AOV `--aov` flags** (`depth`, `normal`, `id`, `sdf_steps`, `lipschitz_ratio`) | ~120 LOC in `loft-cli` / `engines` | Phase 4 (built right on top of `VisRec`). |
| **Load-time 2D/3D/Cube texture bake + auto-mips** | ~100 LOC in `loft-runner` / `loft-ir` | Phase 4 (pairs naturally with IBL sky + procedural masks). |
| **Sparse `OutboxBuf` events + `pick(px)**` | ~90 LOC in `loft-runner` | Phase 3/5 (pairs with motion impacts + Keel embedding). |
| **Vectorized multi-world batching (`env_stride`) + `.loftlog**` | ~200 LOC in `loft-runner` / `loft-ir` | Post-Phase 5 (right when scaling RL / synthetic training). |
</pasted_content id="ac93">

--


<pasted_content id="ac93">
## What `naga` Is and Why We Keep Bringing It Up

**`naga`** is the pure-Rust shader compiler and translation library built inside the `wgpu` project (it is what powers `wgpu`, Firefox/Gecko's WebGPU, Servo, and Deno—analogous to Google's C++ `Tint` inside Chromium/Dawn).

When you hand a shader to `wgpu`, GPU drivers don't understand WGSL:

* Apple Metal wants **MSL** (`.metal` text)
* Vulkan wants **SPIR-V** (binary)
* DirectX 12 wants **HLSL** (then compiled to DXIL via DXC)
* OpenGL / WebGL2 wants **GLSL**
* Browser WebGPU wants **WGSL**

`naga` sits in the middle with a fast, cache-friendly, arena-allocated intermediate representation (**`naga::Module`**):

```text
  [Frontends]                       [Middle]                         [Backends]
  WGSL text  ──┐                                                ┌──► MSL    (Apple M4 Metal)
  SPIR-V     ──┼──► naga::Module ──► naga::valid::Validator ────┼──► SPIR-V (Vulkan)
  GLSL       ──┤    (typed IR)       (safety & capability       ├──► HLSL   (DX12)
  Loft IR!   ──┘                      proofs + spans)           ├──► WGSL   (Browser WebGPU + `loft show`)
                                                                └──► GLSL   (WebGL2 fallback)

```

### Why `naga` Matters So Much to Loft

1. **It decouples Loft's `.loft` syntax from WGSL.** Because `wgpu` accepts `wgpu::ShaderSource::Naga(Cow<'_, naga::Module>)` directly (via the `naga-ir` feature), Loft's compiler (`crates/loft-passes`) can lower E+ `.loft` straight into a `naga::Module` in memory—skipping WGSL text generation and re-parsing completely on native builds, while calling `naga::back::wgsl` only when emitting for browser WebGPU or `loft show`.
2. **It kills double-validation overhead.** In your legacy Rust path, `loft` validated generated WGSL strings with `naga` *and then* `wgpu` parsed and validated those exact strings a second time. Emitting `naga::Module` directly and validating once removes that duplicate cost.
3. **It lets CPU-only Sol lanes prove GPU shaders are legal without touching Metal.** Every parallel Codex Sol lane runs `naga::valid::Validator` on CPU in milliseconds—catching type errors, bad bind-group slots, illegal barriers, and invalid address spaces before Astra ever takes the GPU lock.
4. **It gives us free static shader telemetry.** Because `naga::Module` is a plain Rust arena of typed expressions and statements, a 90-line walker can inspect every compiled shader for ALU counts, transcendentals, memory loads/stores, texture fetches, and shared-memory usage—giving optimization agents hard numbers instead of guesses.
5. **Its limits explain our quirks.** Whenever `naga` has a backend bug (like `pack2x16unorm` garbling `u` in `naga::back::msl`, or `binding_array` function-argument crashes in SPIR-V) or lacks a feature (like Metal `bfloat` / `simdgroup_matrix`), that is a `naga` ceiling—which is why v1.2 added `ShaderTarget::{NagaIr, Wgsl, Msl, Spirv, Ptx}` and `wgpu`'s `Passthrough` path so future native backends can bypass `naga` when you want bare-metal hardware ops.

---

# Engineering Handoff: `docs/spec/06_PLATFORM_POWER_EXTENSIONS.md`

**Status:** Normative extension to Loft v1.0 (`00`–`05`) + `PATCH_V1_2.md`.

**Scope:** Unconstrained meta-engine ABI seams, zero-copy host interop, sparse GPU event outbox & picking, procedural 2D/3D/Cube texture baking with auto-mipmaps, agent observability (AOVs + static `naga` telemetry), vectorized multi-world batching, and `.loftlog` deterministic replay.

**Rule for `chain3.sh`:** Do **not** stall Phase 1.5. Land **Tier A** (ABI struct fields/traits only, ~180 LOC) in Step 0 before Phase 2 codegen freezes; land **Tier B** and **Tier C** strictly in their assigned phases (§9).

---

## 1. Consolidated Layer 0/1 ABI & Grammar Additions (Tier A — Step 0 Freeze)

These are the exact remaining struct/enum/grammar updates to lock in `crates/loft-ir`, `crates/loft-runner`, and `docs/spec/loft.lark` during Step 0 so Phases 2–5 and future domain packs (`std::fluid`, `std::splat`, `std::neural`, `std::diff`, `std::ecs`) never trigger an ABI rewrite.

### 1.1 `docs/spec/loft.lark` Additions (4 Lines)

Add `struct_decl`, `type_alias`, `param_decl` refinement, and `kernel_decl` (parsed in Phase 1.5/4, zero ambiguity):

```lark
struct_decl: "struct" UIDENT "{" (_NL? col (_SEP col)* _SEP?)? "}"
type_alias:  "type" UIDENT "=" type_expr
kernel_decl: "kernel" LIDENT "(" params? ")" ("allow" "(" ident_list ")")? block
// Include struct_decl, type_alias, kernel_decl in `top_item`.

```

### 1.2 `crates/loft-ir` Core Types (Frozen in Step 0)

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

* **Backend Trait (`crates/loft-runner/src/backend.rs`):** Wrap `Runner`'s execution surface in `pub trait ExecutionBackend` with `WgpuBackend` as the sole v1.0 implementation. All future native Metal/CUDA/WASM-`web-sys` runners implement this trait without modifying `loft-ir` or `loft-cli`.
* **Strict Desugaring Invariant:** `crates/loft-runner` and `crates/loft-ir` remain **100% domain-agnostic** (`rg -i -w 'foot|gait|creature|trunk|pbr|shadow|sea|splat|mpm' crates/loft-{runner,ir}` must stay empty).

---

## 2. Zero-Copy External Memory & Live `param` Reflection

### 2.1 Zero-Copy External Buffer / Texture Adoption

Allows Keel (native Rust `wgpu`), Three.js (`WebGPURenderer` via `GPUDevice`), or external PyTorch/MLX/CUDA pipelines to share live GPU memory with Loft at zero CPU copy cost.

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

**Bind-Group Invalidation Rule:** Calling `bind_external_*` or `resize_table` increments `inst.resource_epoch`. Before encoding any pass whose `PassBinding` references a table with `table.epoch > cached_bg.epoch`, `Runner` rebuilds only that pass's `wgpu::BindGroup` (never pipelines or `RenderBundle`s unless vertex/index buffers changed).

### 2.2 Live `param` Reflection & Zero-Compile Sweeps

Every `param name: ty = default in lo..hi` in `.loft` compiles into a slot in `ParamsBuf` (`@group(0)` binding `2`) and a metadata record in `Bundle.params`:

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

---

## 3. Sparse GPU-to-Host Event Outbox & Single-Pixel Picking

### 3.1 `OutboxBuf` (Sparse Gameplay / Diagnostic Triggers)

Replaces expensive full-table GPU readbacks for sparse events (footsteps, parry impacts, claw collisions, solver warnings, fracture triggers).

* **Buffer Layout (`std430`, default 16 KB = 16 B header + 511 events × 32 B):**

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

* **`.loft` Intrinsic (legal inside `law`, `solver`, `kernel`):**

```loft
emit_event(tag: Sym, instance_id: u, site_id: u, payload: v4)

```

Lowers to:

```wgsl
let slot = atomicAdd(&outbox.write_count, 1u);
if (slot < outbox.capacity) { outbox.events[slot] = OutboxEvent(tag, inst, site, frame.tick, payload); }
else { atomicAdd(&outbox.dropped, 1u); }

```

* **Host Drain (Double-Buffered Staging Ring, Zero Stalls):**
`Runner` maintains a 3-slot `MAP_READ` staging ring. At frame end, if `outbox` was bound by any pass, `encode_into` copies `OutboxBuf` → `staging[frame % 3]` and clears `write_count` to `0` via `clear_buffer`. `Runner::drain_events(inst) -> (Vec<OutboxEvent>, u32 /*dropped*/)` reads the completed ring slot from `frame - 2` asynchronously without blocking the GPU.

### 3.2 Single-Pixel Picking (`Runner::pick`)

For interactive editors, agent spatial inspection, and mouse clicking without CPU raycasting:

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

*(In headless CLI tools like `loft show --pixel x,y`, `pick_sync` polls `device.poll(Maintain::Wait)` immediately.)*

---

## 4. Load-Time & On-Dirty Procedural 2D/3D/Cube Texture Baking + Auto-Mipmaps

Generalizes 1D `scan` LUTs to procedural 2D masks (wet-paper calligraphy/ink bleed, heightfields), 3D noise/cloud volumes, and prefiltered IBL sky cubemaps.

### 4.1 `.loft` Syntax & `TableDecl` Extension

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

### 4.2 Auto-Mipmap Compute Pass (`crates/loft-runner/src/mipgen.rs`, ~60 LOC)

* Single cached compute pipeline per texture format class (`Rgba16Float`, `R16Float`, `Rgba8Unorm`) using `8x8x1` workgroups (and `8x8x6` for cubemap array layers, `4x4x4` for 3D).
* Downsamples level `k` (`textureLoad` 2×2 tap box filter, or 2×2×2 for 3D) → level `k+1` (`textureStore` on storage view) for `k = 0 .. mip_count - 2`.
* Runs during `load_bundle` for `BakeClock::Load`, and at the head of `encode_into` only when a watched `ParamId` changes (`BakeClock::OnDirty`). Zero per-frame cost once clean.

---

## 5. First-Class Agent Observability: AOVs & Static `naga` Cost Telemetry

### 5.1 Standard Diagnostic AOVs (`--aov`)

Every schedule producing a `VisRec` or `Hit` automatically supports diagnostic visualization passes via `loft render scene.loft --aov <mode>` (and `loft clip --aov ...`), compiled as an alternate `present` / `shade` pass without touching scene code:

| `--aov` Mode | Output Encoding | What Agents Diagnose With It |
| --- | --- | --- |
| `beauty` *(default)* | Tonemapped sRGB | Final picture / aesthetic reward |
| `albedo` | Linear base albedo (unlit) | Texture/material color bugs isolated from lighting |
| `normal` | `0.5 * n_world + 0.5` | Inverted windings, AD vs finite-diff normal glitches |
| `depth` | 8-bit log distance (matches R0g) | Geometry clipping, near/far precision |
| `id` | Deterministic hash color of `(kind_id, row_id)` | Instance batching, unintended kind splits |
| `sdf_steps` | False-color `steps / max_steps` (blue→green→red) | Grazing-angle step explosions, loose AABB proxies |
| `lipschitz_ratio` | `‖∇f(p)‖ / L_derived` at hit (0..1+) | **Red (>1.0)** proves a broken bound; **deep blue (<0.3)** pinpoints where one small feature (e.g. claw teeth) inflated global $L$ and slowed empty-space marching |
| `overdraw` | Additive `1/16` per proxy fragment | Unsorted proxy overlap / overdraw waste |

### 5.2 Static `naga::Module` Cost Telemetry (`crates/loft-passes/src/telemetry.rs`, ~95 LOC)

Runs automatically during `loft build` and `loft profile --json` by walking every `naga::Function` in each emitted `naga::Module` (zero GPU required, runs in `< 1 ms`):

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

* **Walk algorithm:** Recurse over `module.entry_points[*].function.body` (plus transitively called `module.functions`), inspecting `naga::Statement` and `naga::Expression` arenas once per handle. Weight loop bodies optionally by static trip bound when known (`weighted_alu_est`).
* **Gate integration:** `loft profile --json` emits `modules: Vec<ModuleTelemetry>` alongside `gpu_pass_ms`. Agents immediately see *why* a pass took `4.2 ms` (e.g., `transcendentals: 48, storage_loads: 64, call_sites: 5`) instead of guessing.

---

## 6. Vectorized Multi-World Batching (`env_stride` & Atlas Rendering)

Enables running `N` independent worlds/rollouts (e.g., `N = 64` creative mutations or `N = 512` RL physics environments) in a **single GPU submission** with zero extra shader modules or pipelines.

### 6.1 Memory & Indexing Model

* **Host API:**
```rust
impl Runner {
    pub fn load_bundle_batched(&mut self, bundle: &Bundle, n_envs: u32) -> Result<BatchInstanceId, RunnerError>;
    pub fn push_input_env(&mut self, batch: BatchInstanceId, env_id: u32, slot: InputId, tick: u64, val: InputValue<'_>);
    pub fn set_param_env(&mut self, batch: BatchInstanceId, env_id: u32, id: ParamId, val: &[f32]) -> Result<(), RunnerError>;
    pub fn reset_env(&mut self, batch: BatchInstanceId, env_id: u32, seed: u64) -> Result<(), RunnerError>;
}

```


* **Table Allocation:**
* `TableLifetime::Static` (`BODY`, baked `scan` LUTs, static meshes) is **shared** across all `N` environments (`1×` memory).
* `DynamicArena`, `PingPong`, `StateBuf`, `PoseBuf`, `ParamsBuf`, `InputsBuf`, and `VerdictBuf` allocate `n_envs * base_stride` bytes.


* **Shader Indexing:**
* `FrameUniform` gains `env_count: u32` and `atlas_grid: vec2<u32>` (e.g., `8×8` tiles for `64` envs).
* Compute passes map `env_id = GlobalId.y` (or `GlobalId.x / rows_per_env`) and offset every mutable table index by `env_id * env_stride`.
* **Atlas Rendering (1 `RenderBundle` for all `N` cameras):** For `n_envs` rendered views (e.g., `64` views of `256×256` inside one `2048×2048` atlas):
* `cull_indirect` runs across `(env_id, row_id)` against `cameras[env_id]`, writing compacted `(env_id, row_id)` pairs into `visible_rows`.
* Vertex / proxy shaders project using `cameras[env_id]` and map NDC `[-1, 1]²` into tile `(env_id % cols, env_id / cols)` inside the atlas texture (clipping fragments outside their tile scissor).
* Result: **64 environments rendered in the exact same 15–19 GPU commands as 1 environment**, giving **10–30× rollout throughput** on Apple M4 and cloud GPUs.





---

## 7. Portable `.loftlog` Deterministic Replay Files

Turns any physics failure, missed Truth verdict, or standout creative rollout into a tiny, self-contained, exact reproducer.

### 7.1 `.loftlog` Format (Binary `LOFTLOG1` or `--format json`)

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

* **CLI Integration:**
* `loft render scene.loft --record bug.loftlog` — captures all host inputs & `set_param` calls.
* `loft render scene.loft --replay bug.loftlog --out repro.png` — replays bit-identically on the same device (or verifies within Tier tolerance cross-device).
* `loft check scene.loft --replay bug.loftlog` — re-runs all Truth verdicts along the exact recorded trace and fails with exit code `2` if any hold regresses.



---

## 8. Phased Rollout Plan & Lane Assignments (Zero Disruption to `chain3.sh`)

| Extension | Target Phase | Owner Crate(s) | Pred. LOC | Pass / Fail Verification Gate |
| --- | --- | --- | --- | --- |
| **Tier A: ABI Locks** (`PassBinding`, `GpuFeatures`, `ElemFormat`, `TableLifetime`, `PassLoop`, `ShaderTarget`, `ExecutionBackend` trait, `struct`/`type` in `loft.lark`) | **Step 0 Freeze** *(Astra)* | `loft-ir`, `loft-runner`, `loft.lark` | **~180** | `cargo check --target wasm32-unknown-unknown -p loft-runner` passes; `loft.lark` parses all `.loft` files; `rg` core-domain-word gate empty. |
| **Live `param` reflection + `bind_external_***` | **Step 0 / Phase 2** *(Sol L1)* | `loft-runner`, `loft-passes` | **~90** | Sweeping roughness/sun `param` takes `0 ms` compile; external `wgpu::Buffer` write visible in next frame with 0 CPU copy. |
| **Static `naga` Telemetry (`telemetry.rs`)** | **Phase 2** *(Sol L1)* | `loft-passes`, `loft-cli` | **~95** | `loft profile --json` reports exact ALU/transcendental/load/workgroup counts; catches synthetic 5× BSDF inline mutant. |
| **Sparse `OutboxBuf` + `Runner::pick_async**` | **Phase 3** *(Sol L2 — Motion Core)* | `loft-runner`, `loft-passes` | **~110** | Boss Hit 1 emits exact `ClawHit` & `FootSlip` events to host with `dropped == 0`; `pick_async` on Washer returns exact `kind_id/row_id` matching oracle (`9,664`/`12,080`). |
| **AOVs (`--aov`) + Load/Dirty Procedural Texture Bake + Auto-Mips** | **Phase 4** *(Sol L3 — Engines/Post)* | `loft-runner`, `loft-passes`, `loft-cli` | **~220** | `--aov lipschitz_ratio` flags synthetic over-bound mutant (`>1.0` red); 2048² procedural mask + 128² IBL cube bake & mip-chain run once at load (`≤ 2 ms` **predicted**), `0 ms`/frame thereafter. |
| **`.loftlog` Replay + `.loftb` Zero-Compiler Test** | **Phase 5** *(Sol L2 — Embed)* | `loft-runner`, `loft-cli` | **~120** | Recorded crab scramble replays with `0`-bit state diff; `.loftb` loads in standalone runner binary without linking `loft-passes`. |
| **Vectorized Multi-World Batching (`env_stride` + Atlas)** | **Phase 5.5 / Post-v1.0** *(RL Scaling)* | `loft-runner`, `loft-ir`, `loft-passes` | **~240** | `64` parallel crabs render into `2048×2048` atlas in identical GPU command count (`≤ 19` cmds) at `≥ 8×` aggregate FPS/env vs sequential execution (**predicted**). |

### Final Note for Astra Before Executing `chain3.sh`

1. Fold **Tier A** (`~180 LOC`) into Step 0's freeze commit right alongside `PATCH_V1_2.md` and `loft-abi`.
2. Keep `wasm32` compile checking (`cargo check --target wasm32-unknown-unknown -p loft-runner`) active on every merged wave.
3. Do **not** implement Phase 5.5 vectorized batching or future packs (`std::fluid`, `std::splat`, `std::neural`, `std::diff`) during `chain3.sh`—the Step 0 ABI locks guarantee they drop in cleanly as ordinary libraries and passes once Phases 1.5–5 ship green pictures and numbers.
</pasted_content id="ac93">

--

Use a sub agent to write down and record hat useufl here
And take all details and incporate