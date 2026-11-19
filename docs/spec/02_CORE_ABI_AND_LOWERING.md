## 1. Layer 0: Runner Interface (`crates/loft-runner`, ≤ 1,000 LOC)

Embeddable on a caller-owned `wgpu::Device` (native Metal/Vulkan/DX12 and `wasm32` WebGPU); zero forced clears, zero synchronous readbacks in the frame loop.

```rust
pub struct HostCaps {
    pub depth_convention: DepthConvention, // StandardZeroToOne | ReverseOneToZero
    pub supports_timestamps: bool,
    pub supports_pipeline_cache: bool,
    pub msaa_samples: u8,                  // 1 or 4
}

pub enum InputValue<'a> {
    Level(&'a [f32]),
    Event { seq: u64, payload: &'a [f32] }, // deduplicated by seq, consumed once on its tick
    PosePacket { tick: u64, data: &'a [f32] },
}

pub struct FrameMetrics {
    pub gpu_pass_ms: Box<[(PassId, f32)]>,
    pub total_gpu_ms: f32,
    pub commands: u32,
    pub modules: u32,
    pub pipelines: u32,
    pub load_ms: f32,
}

impl Runner {
    pub fn attach(device: Arc<wgpu::Device>, queue: Arc<wgpu::Queue>, caps: HostCaps) -> Self;
    pub fn load_bundle(&mut self, bundle: &Bundle) -> Result<InstanceId, RunnerError>;
    pub fn hot_swap_tape(&mut self, inst: InstanceId, kind: KindId, tape: &[u32]) -> Result<(), RunnerError>;
    pub fn hot_swap_module(&mut self, inst: InstanceId, module: ModuleId, wgsl: &str) -> Result<(), RunnerError>;
    pub fn push_input(&mut self, inst: InstanceId, slot: InputId, tick: u64, val: InputValue<'_>);
    pub fn seek(&mut self, inst: InstanceId, target_tick: u64) -> Result<(), RunnerError>;
    pub fn encode_into(&mut self, inst: InstanceId, encoder: &mut wgpu::CommandEncoder,
                       target: &wgpu::TextureView, depth: Option<&wgpu::TextureView>,
                       time_s: f64, dt_s: f32) -> Result<FrameIdentity, RunnerError>;
    pub fn last_metrics(&self, inst: InstanceId) -> FrameMetrics;
    pub fn read_table_async(&self, inst: InstanceId, table: TableId, cb: Box<dyn FnOnce(&[u8]) + Send>);
    pub fn read_verdicts_async(&self, inst: InstanceId, cb: Box<dyn FnOnce(&[VerdictRecord]) + Send>);
}

```

## 2. Frozen Conventions & Bind Group ABI

* **Coordinates:** Right-handed, **Y-up**, local object forward **`+Z`**, camera looks along **`-Z`**, column-major `mat4x4f`, quaternions `vec4f(x, y, z, w)`, screen `(0, 0)` top-left, parametric normal `normalize(cross(du, dv))`.
* **Bind Groups (immutable contract between `loft-runner` and `loft-passes`):**

| Group | Bindings | Update Rate | Contents |
| --- | --- | --- | --- |
| **`@group(0)`** | `0`: `uniform FrameUniform`<br>

<br>`1`: `storage<read> InputsBuf`<br>

<br>`2`: `storage<read> ParamsBuf` | Per-frame | `time`, `dt`, `tick`, `frac`, `frame_idx`, `sample_idx`, `view_size`, `cam` (`view`, `proj`, `inv_vp`, `eye`, `reverse_z`), per-tick `input_ring` |
| **`@group(1)`** | `0`: `storage<read> RowTables`<br>

<br>`1`: `storage<read> BodyTables`<br>

<br>`2`: `storage<read> BvhNodes`<br>

<br>`3`: `storage<read> LutTables` | Per-load / row edit | std430 kind rows (via `kind_base_offset`), `BODY` rows, `Bvh2` nodes + leaf refs, baked `scan` & Kulla–Conty LUTs |
| **`@group(2)`** | `0`: `storage<read_write> StateBuf`<br>

<br>`1`: `storage<read_write> PoseBuf`<br>

<br>`2`: `storage<read_write> IndirectBuf`<br>

<br>`3`: `storage<read_write> VerdictBuf` | Per-step / per-pass | Live/baked `STATE`, 32-byte `POSE` joints (or custom motion tables), compacted `visible_rows` + `DrawIndirectArgs`, lock-free `VerdictRecord` slots |
| **`@group(3)`** | `0..7`: sampled textures (`VisBuf`, `DepthBuf`, `ShadowMap`, `HdrBuf`, `PrevHdr`, `Assets`)<br>

<br>`8`: linear sampler, `9`: nearest/comparison sampler | Per-pass | Transient schedule targets & imported assets |

## 3. Layer 1: Core Data Structures (`crates/loft-ir`, ≤ 1,500 LOC)

### 3.1 `VisRec` (4× MSAA `rgba16uint` + `depth32float` = 12 B/sample)

```text
word0 (u32 from lanes 0..1): [31..23] kind_id (9 bits, max 512 kinds; 0 = background)
                             [22]     ink_bit (1 bit)
                             [21..0]  row_id  (22 bits, max 4,194,304 rows/kind)
word1 (u32 from lanes 2..3): cover-owned payload (packed by hand; never pack2x16unorm due to Metal naga bug):
  - grid surface:  lane2 = u_u16, lane3 = v_u16
  - triangle mesh: word1 = tri_id (u32)
  - proxy ray/SDF: word1 = ray_t_f32_bits or cell_hash (u32)

```

### 3.2 `Bvh2` Acceleration Nodes (32 B/node) & `VerdictRecord` (32 B/slot)

```rust
#[repr(C)]
pub struct BvhNode { pub lo: [f32; 3], pub left_or_first: u32, pub hi: [f32; 3], pub count: u32 }
#[repr(C)]
pub struct LeafRef { pub kind_id: u32, pub row_id: u32 }

#[repr(C)]
pub struct VerdictRecord {
    pub constraint_id: u32, pub instance_id: u32, pub site_id: u32, pub tick: u32,
    pub tag: u32,         // 0=Skipped, 1=Held, 2=Missed, 3=Unknown
    pub reason_code: u32, // 0=None, 1=IterCap, 2=NonFinite, 3=MissingSurface, 4=MissingFrame, 5=SolveFailed
    pub value: f32,       // margin | error | residual
    pub law_hash_lo: u32, // invalidates cached verdict if law changes
}

```

## 4. Layer 2: Key Lowering Rules (`crates/loft-passes`, ~10k LOC Modular)

1. **Pose-factored bound & $L$ derivation:** Strip `pose: xf@S>World` before hashing shape geometry. Derivations scale with distinct local shapes (`11` at both 1 and 20 crabs **measured**), not instance count. Cache derivations on disk keyed by `(ir_hash, shape_row_bytes)` to cut warm CPU load from `230 ms` (**measured**) to `< 30 ms` (**predicted**).
2. **Staged `Bsdf` monomorphization:** `material` lobe sums (`ggx + sss + coat`) lower at compile time into four static WGSL functions per kind (`K_eval`, `K_sample`, `K_emit`, `K_albedo`) with zero heap closures and one call site per shade loop (keeping Metal cold compile at ~`70 ms`/pipeline serial **measured**).
3. **Field-disjoint law fusion + `(wave, kind)` bucketing:** Topologically sort laws into dependency waves. Any sequence of row-local laws on the same kind `K` with disjoint `writes` fuses sequentially into one `advance(from_tick, to_tick, keep)` compute entry (trip bound 64, zero atomics, explicit `fma()`). Same entry serves `Baked` and `Live` homes for 0-bit per-device parity.
4. **Indirect draw + `RenderBundle`:** Single `cull` dispatch compacts visible `row_id`s per kind and writes 16-byte `DrawIndirectArgs`. Primary visibility (and shadow maps) execute pre-recorded `RenderBundle`s with 1 `setPipeline` + 1 `drawIndirect` per kind.
5. **Single `naga` validation + name mangling:** Build `naga::Module` directly, mangle WGSL reserved words used as row fields (e.g. `target` $\to$ `r_target`, fixing Phase 1's catch), validate once, pass `ShaderSource::Naga` natively and emit WGSL text for WebGPU / `loft show`.
6. **Build-time `plan` solver:** Uniform cubic B-spline (`N = 24` knots, `M = 96` collocation points), analytical sparse Hessian, exact 3-DOF ballistic flight elimination, augmented Lagrangian (`max 40` iters, `tol = 1e-5`); failures emit `Verdict::Unknown(SolveFailed)`.
7. **Tier 0 Tape VM (edit-time, single dirty kind only):** `var reg: array<vec4f, 32>` (512 B/thread, zero spill), 56 3-address 32-bit ops (`[op:6][dst:5][a:5][b:5][imm11:11]`), kind-bucketed dispatch (uniform PC across all 32 SIMD lanes), `400 ms` quiet promotion via `create*PipelineAsync`, tolerance `|T0 - T1| ≤ 1e-5` (fields) and `≤ 1/255` on ≥ `99.9%` shaded pixels. Motion never runs in Tier 0.
8. **S-expression IR codec (`sexp.rs`, ~250 LOC):** Lossless serializer/reader over Layer 1 `Bundle`/SSA IR (`loft ir scene.loft --sexp` / `loft build scene.sexp`) for agent-driven compiler/IR transformations.

## 5. The Five CI Gates

| Gate | Pass Condition |
| --- | --- |
| **Pictures** | All expected frames present (missing frame fails). Geometry/depth pass **R0g**; light passes converged `reference` (95% CI ≤ `0.005`, interior rel bias ≤ `1%`). |
| **Twin** | Motion: `0`-bit diff live vs bake per device (30/60/120 Hz + 6-tick stall). Fields: `≤ 1e-5`; AD normals `≤ 1e-4`; lit materials `≤ 2e-5` (60° off specular peak). |
| **Speed** | Quiet M4 `p95` frame time and `commands = O(kinds + waves)` within scene budget. |
| **Load** | Modules, pipelines, cold/warm load within budget; **module text byte-identical between 1 and 20 instances**. |
| **Truth** | Every default and scene `check` returns `Held` unless explicitly marked `expect Missed` / `expect Unknown`. |

---
