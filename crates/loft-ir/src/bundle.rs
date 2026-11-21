//! Layer 0/1 contract. Groups 1–3 come exclusively from each pass declaration.
use std::collections::BTreeMap;

macro_rules! ids {
    ($($name:ident),+ $(,)?) => {$(
        #[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
        pub struct $name(pub u32);
    )+};
}
ids!(InstanceId, SymId, ValId);

macro_rules! record {
    ($(#[$attr:meta])* $name:ident {
        $($(#[$doc:meta])* $field:ident: $ty:ty),+ $(,)?
    } $($guard:ident)? $(/ $bulk:ident: $blob:ty)?) => {
        $(#[$attr])*
        pub struct $name {
            $($(#[$doc])* pub $field: $ty,)+
            $(pub $bulk: $blob,)?
        }
    };
}
macro_rules! sum {
    ($(#[$attr:meta])* $name:ident {
        $($tag:literal => $variant:ident $({$($field:ident: $ty:ty),+ $(,)?})? $(($value:ident: $item:ty))?),+ $(,)?
    }) => {
        $(#[$attr])*
        pub enum $name { $($variant $({$($field: $ty),+})? $(($item))?),+ }
    };
}

crate::bundle_schema!(ids, record, sum);

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
        const FLOAT32_ATOMICS = 1 << 10;
        const COOP_MATRIX_F16 = 1 << 11;
        const PASSTHROUGH_SHADERS = 1 << 12;
        const METAL_FX = 1 << 13;
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum DepthConvention {
    StandardZeroToOne,
    ReverseOneToZero,
}

#[derive(Clone, Copy, Debug)]
pub struct HostCaps {
    pub depth_convention: DepthConvention,
    pub msaa_samples: u8,
    pub features: GpuFeatures,
}

impl HostCaps {
    pub const WEB_BASELINE: Self = Self {
        depth_convention: DepthConvention::StandardZeroToOne,
        msaa_samples: 1,
        features: GpuFeatures::empty(),
    };
}

impl NativeCompute {
    pub fn feature(self) -> GpuFeatures {
        match self {
            Self::CooperativeF32 => GpuFeatures::COOP_MATRIX,
            Self::CooperativeF16 => GpuFeatures::COOP_MATRIX_F16,
            Self::Msl => GpuFeatures::PASSTHROUGH_SHADERS,
        }
    }

    pub fn target(self) -> ShaderTarget {
        match self {
            Self::CooperativeF32 | Self::CooperativeF16 => ShaderTarget::Wgsl,
            Self::Msl => ShaderTarget::Msl,
        }
    }
}

impl BindAccess {
    pub fn group(self) -> u8 {
        match self {
            Self::ReadStorage => 1,
            Self::ReadWriteStorage => 2,
            Self::SampledTex | Self::StorageTex | Self::BindlessTexArray | Self::Sampler | Self::AccelerationStructure => 3,
        }
    }
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InputId(pub u32);
pub enum InputValue<'a> {
    Level(&'a [f32]),
    Event { seq: u64, payload: &'a [f32] },
    PosePacket { tick: u64, data: &'a [f32] },
}

impl Resources {
    pub fn is_empty(&self) -> bool {
        self.inputs.is_empty()
            && self.textures.is_empty()
            && self.comparison_samplers.is_empty()
            && self.resets.is_empty()
            && self.cull_front.is_empty()
            && self.frame.is_none()
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
impl Bundle {
    pub fn native_modules(&self) -> std::collections::BTreeSet<ModuleId> {
        let mut remaining: Vec<_> = self.passes.iter().collect();
        let mut modules = std::collections::BTreeSet::new();
        while let Some(pass) = remaining.pop() {
            if let PassRun::NativeCompute { native, .. } = pass.run { modules.insert(native); }
            remaining.extend(pass.run.children());
        }
        modules
    }
}

impl PassRun {
    pub fn children(&self) -> &[PassDecl] {
        match self {
            Self::PassLoop { body, .. } => body,
            Self::Compute { .. } | Self::ComputeFusedTicks { .. } | Self::ComputeNonDetTicks { .. }
            | Self::RenderBundle { .. } | Self::GenerateMips { .. } | Self::NativeCompute { .. }
            | Self::HostCallback { .. } => &[],
        }
    }
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
