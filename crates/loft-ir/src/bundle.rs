//! Layer 0/1 contract. Groups 1–3 come exclusively from each pass declaration.
use std::collections::BTreeMap;
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct ModuleId(pub u32);
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct TableId(pub u32);
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct PassId(pub u32);
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct KindId(pub u32);
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct InstanceId(pub u32);
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct ParamId(pub u32);
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct SymId(pub u32);
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct ValId(pub u32);
bitflags::bitflags! {
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
    pub struct GpuFeatures: u32 {
        const TIMESTAMPS = 1 << 0;
        const PIPELINE_CACHE = 1 << 1;
        const SUBGROUPS = 1 << 2;
        const F16 = 1 << 3;
        const INT_ATOMICS = 1 << 4;
        const MULTI_DRAW_INDIRECT = 1 << 5;
        const BINDLESS_TEXTURES = 1 << 6;
        const RAY_QUERY = 1 << 7;
        const PACKED_INT8_DOT = 1 << 8;
        const COOP_MATRIX = 1 << 9;
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ShaderTarget {
    NagaIr,
    Wgsl,
    Msl,
    Spirv,
    Ptx,
    Wasm,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum QueueClass {
    Graphics,
    AsyncCompute,
    Transfer,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ElemFormat {
    F32,
    F16,
    Bf16,
    F64,
    U32,
    I32,
    U16,
    I16,
    U8,
    I8,
    PackedU4,
    PackedI4,
    PackedU2,
    Fp8E4M3,
    Fp8E5M2,
    Struct(u32),
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum BindAccess {
    ReadStorage,
    ReadWriteStorage,
    SampledTex,
    StorageTex,
    BindlessTexArray,
    Sampler,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum DepthConvention {
    StandardZeroToOne,
    ReverseOneToZero,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum IrTy {
    F32,
    Vec2,
    Vec3,
    Vec4,
    U32,
    Bool,
}

#[derive(Clone, Copy, Debug)]
pub struct HostCaps {
    pub depth_convention: DepthConvention,
    pub msaa_samples: u8,
    pub features: GpuFeatures,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TableLifetime {
    Static,
    DynamicArena { max_rows: u32 },
    Transient,
    PingPong,
    External,
}

#[derive(Clone, Debug)]
pub struct TableDecl {
    pub id: TableId,
    pub format: ElemFormat,
    pub lifetime: TableLifetime,
    /// Addressable rows consist of one or more whole storage words, including packed lanes.
    pub stride_bytes: u32,
    pub capacity: u32,
    pub initial: Box<[u8]>,
}
#[derive(Clone, Debug)]
pub struct StructLayout {
    pub stride_bytes: u32,
    pub offsets: Box<[u32]>,
}

#[derive(Clone, Debug)]
pub struct ModuleBlob {
    pub id: ModuleId,
    pub hash: [u8; 32],
    pub target: ShaderTarget,
    pub bytes: Box<[u8]>,
}

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub struct PassBinding {
    pub group: u8,
    pub slot: u32,
    pub resource: TableId,
    pub access: BindAccess,
}
impl BindAccess {
    pub fn group(self) -> u8 {
        match self {
            Self::ReadStorage => 1,
            Self::ReadWriteStorage => 2,
            Self::SampledTex | Self::StorageTex | Self::BindlessTexArray | Self::Sampler => 3,
        }
    }
}
#[derive(Clone, Debug)]
pub enum DispatchExtent {
    Fixed([u32; 3]),
    FromRowCount { table: TableId, workgroup_x: u32 },
    Indirect { buf: TableId, offset_bytes: u32 },
}
#[derive(Clone, Debug)]
pub struct IndirectDraw {
    pub module: ModuleId,
    pub vertex_entry: String,
    pub fragment_entry: String,
    pub args: TableId,
    pub offset_bytes: u32,
    pub vertex: Option<TableId>,
    pub index: Option<TableId>,
}
#[derive(Clone, Debug)]
pub struct TargetDesc {
    pub table: TableId,
}
#[derive(Clone, Debug)]
pub struct DepthDesc {
    pub table: TableId,
    pub write: bool,
}
#[derive(Clone, Debug)]
pub enum PassRun {
    Compute {
        module: ModuleId,
        entry: String,
        dispatch: DispatchExtent,
    },
    ComputeFusedTicks {
        module: ModuleId,
        entry: String,
        max_ticks: u32,
        instances: TableId,
    },
    RenderBundle {
        draws: Box<[IndirectDraw]>,
        color_targets: Box<[TargetDesc]>,
        depth: Option<DepthDesc>,
        msaa: u8,
    },
    PassLoop {
        max_iters: u32,
        cond_buf: Option<(TableId, u32)>,
        body: Box<[PassDecl]>,
    },
    GenerateMips {
        texture: TableId,
    },
}
#[derive(Clone, Debug)]
pub struct PassDecl {
    pub id: PassId,
    pub wave: u16,
    pub queue: QueueClass,
    pub bindings: Box<[PassBinding]>,
    pub run: PassRun,
}
#[derive(Clone, Debug)]
pub struct StateStoreDecl {
    pub table: TableId,
    pub layout_hash: [u8; 32],
    pub checkpoint_slots: u32,
}
#[derive(Clone, Debug)]
pub struct ParamMeta {
    pub id: ParamId,
    pub name: String,
    pub ty: IrTy,
    pub offset_bytes: u32,
    pub default: [f32; 4],
    pub range: Option<([f32; 4], [f32; 4])>,
}
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct OutboxHeader {
    pub write_count: u32,
    pub capacity: u32,
    pub dropped: u32,
    pub frame_idx: u32,
}
#[repr(C, align(16))]
#[derive(Clone, Copy, Debug)]
pub struct OutboxEvent {
    pub tag: u32,
    pub instance_id: u32,
    pub site_id: u32,
    pub tick: u32,
    pub payload: [f32; 4],
}
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct PickResult {
    pub hit: bool,
    pub kind_id: u32,
    pub row_id: u32,
    pub payload: u32,
    pub depth_ndc: f32,
    pub world_pos: [f32; 3],
}
#[derive(Clone, Copy, Debug)]
pub enum ImageFormat { Rg32Uint, Rgba16Float, Depth32Float }

#[derive(Clone, Copy, Debug)]
pub struct TextureDecl {
    pub table: TableId,
    pub format: ImageFormat,
    pub extent: [u32; 3],
    pub samples: u8,
    pub clear: Option<[f64; 4]>,
}

#[derive(Clone, Debug)]
pub struct FrameData {
    pub main: TableId,
    pub variants: Vec<(PassId, TableId)>,
    pub time_offset: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InputId(pub u32);
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InputKind { Level, Event, Packet }
#[derive(Clone, Copy, Debug)]
pub struct InputSlot {
    pub kind: InputKind,
    pub offset_words: u32,
    pub words: u32,
}
pub enum InputValue<'a> {
    Level(&'a [f32]),
    Event { seq: u64, payload: &'a [f32] },
    PosePacket { tick: u64, data: &'a [f32] },
}

#[derive(Clone, Debug, Default)]
pub struct Resources {
    pub inputs: Vec<InputSlot>,
    pub textures: Vec<TextureDecl>,
    pub comparison_samplers: Vec<TableId>,
    pub resets: Vec<TableId>,
    pub cull_front: Vec<PassId>,
    pub frame: Option<FrameData>,
}
impl Resources {
    pub fn is_empty(&self) -> bool {
        self.inputs.is_empty() && self.textures.is_empty() && self.comparison_samplers.is_empty()
            && self.resets.is_empty() && self.cull_front.is_empty() && self.frame.is_none()
    }
}

#[derive(Clone, Debug, Default)]
pub struct Bundle {
    pub resources: Resources,
    pub modules: Vec<ModuleBlob>,
    pub tables: Vec<TableDecl>,
    pub struct_layouts: Vec<StructLayout>,
    pub rows: BTreeMap<KindId, TableId>,
    pub state: Option<StateStoreDecl>,
    pub params: Vec<ParamMeta>,
    pub passes: Vec<PassDecl>,
}
#[derive(Clone, Debug)]
pub enum Op {
    Intrinsic { name: SymId, args: Box<[ValId]> },
}
#[derive(Default)]
pub struct IrModule {
    pub ops: Vec<Op>,
}
pub type PassRegistry = BTreeMap<SymId, fn(&mut IrModule) -> crate::Res<()>>;
