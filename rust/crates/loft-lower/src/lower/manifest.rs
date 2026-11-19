//! WebGPU descriptors from src/host/manifest.ts, in the order src/host/lower.ts writes their keys.
use crate::artefact::{
    AuthorLines, Camera, Cull, Detail, Extent, InputSlot, Layer, OrderedMap, ParamSlot, Stats, TableFillKind,
    TextureSize,
};
use serde::Serialize;

pub(super) type Id = usize;
pub(super) const FORMAT: &str = "loft-artefact";
pub(super) const COLOR_FORMAT: &str = "rgba8unorm";
pub(super) const DEPTH_FORMAT: &str = "depth32float";
pub(super) const SCENE_DEPTH_FORMAT: &str = "r32float";
pub(super) const LIGHT_FORMAT: &str = "rgba16float";
pub(super) const SAMPLES: usize = 4;

#[derive(Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "UPPERCASE")]
pub(super) enum Stage {
    Vertex,
    Fragment,
    Compute,
}

impl Stage {
    pub(super) fn name(self) -> &'static str {
        match self {
            Self::Vertex => "vertex",
            Self::Fragment => "fragment",
            Self::Compute => "compute",
        }
    }
}

#[derive(Serialize)]
pub(super) struct BufferDesc {
    pub label: String,
    pub size: usize,
    pub usage: &'static [&'static str],
    #[serde(skip_serializing_if = "Option::is_none")]
    pub init: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct TextureDesc {
    pub label: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size: Option<TextureSize>,
    // Fixed compute textures write extent after label; render textures write size there.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extent: Option<Extent>,
    pub format: &'static str,
    pub sample_count: usize,
    pub usage: &'static [&'static str],
    #[serde(skip_serializing_if = "Option::is_none")]
    pub init: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct SamplerDesc {
    pub label: &'static str,
    pub mag_filter: &'static str,
    pub min_filter: &'static str,
    pub address_mode_u: &'static str,
    pub address_mode_v: &'static str,
}

#[derive(Clone, Copy, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(super) enum BufferType {
    Uniform,
    ReadOnlyStorage,
    Storage,
}

#[derive(Clone, Copy, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(super) enum SampleType {
    Float,
    UnfilterableFloat,
    Depth,
}

#[derive(Clone, Copy, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(super) enum SamplerType {
    Filtering,
    NonFiltering,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) enum LayoutResource {
    Buffer {
        #[serde(rename = "type")]
        ty: BufferType,
    },
    #[serde(rename_all = "camelCase")]
    Texture { sample_type: SampleType, view_dimension: &'static str, multisampled: bool },
    #[serde(rename_all = "camelCase")]
    StorageTexture { access: &'static str, format: &'static str, view_dimension: &'static str },
    Sampler {
        #[serde(rename = "type")]
        ty: SamplerType,
    },
}

#[derive(Serialize)]
pub(super) struct LayoutEntry {
    pub binding: usize,
    pub visibility: &'static [Stage],
    #[serde(flatten)]
    pub resource: LayoutResource,
}

#[derive(Serialize)]
pub(super) struct BindGroupLayoutDesc {
    pub label: String,
    pub entries: Vec<LayoutEntry>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct PipelineLayoutDesc {
    pub label: String,
    pub bind_group_layouts: Vec<Id>,
}

#[derive(Serialize)]
pub(super) struct ModuleDesc {
    pub label: String,
    pub chunks: Vec<Id>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct ComputePipelineDesc {
    pub label: String,
    pub layout: Id,
    pub module: Id,
    pub entry_point: String,
}

#[derive(Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) enum Resource {
    Buffer(Id),
    Texture(Id),
    Sampler(Id),
}

#[derive(Clone, Copy, Serialize)]
pub(super) struct BindGroupEntry {
    pub binding: usize,
    #[serde(flatten)]
    pub resource: Resource,
}

#[derive(Serialize)]
pub(super) struct BindGroupDesc {
    pub label: String,
    pub layout: Id,
    pub entries: Vec<BindGroupEntry>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct ShaderStageDesc {
    pub module: Id,
    pub entry_point: String,
    pub constants: OrderedMap<f64>,
}

#[derive(Serialize)]
pub(super) struct FragmentDesc {
    #[serde(flatten)]
    pub stage: ShaderStageDesc,
    pub targets: Vec<ColorTarget>,
}

#[derive(Serialize)]
pub(super) struct ColorTarget {
    pub format: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub blend: Option<BlendState>,
}

#[derive(Serialize)]
pub(super) struct BlendState {
    pub color: BlendComponent,
    pub alpha: BlendComponent,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct BlendComponent {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub operation: Option<&'static str>,
    pub src_factor: &'static str,
    pub dst_factor: &'static str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct Primitive {
    pub topology: &'static str,
    pub front_face: &'static str,
    pub cull_mode: Cull,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct DepthStencil {
    pub format: &'static str,
    pub depth_write_enabled: bool,
    pub depth_compare: &'static str,
}

#[derive(Serialize)]
pub(super) struct Multisample {
    pub count: usize,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct RenderPipelineDesc {
    pub label: String,
    pub layout: Id,
    pub vertex: ShaderStageDesc,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fragment: Option<FragmentDesc>,
    pub primitive: Primitive,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub depth_stencil: Option<DepthStencil>,
    pub multisample: Multisample,
}

#[derive(Serialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub(super) enum FrameWrite {
    Frame { buffer: Id },
    Inputs { buffer: Id },
    Params { buffer: Id },
}

#[derive(Serialize)]
#[serde(untagged)]
pub(super) enum Workgroups {
    X(usize),
    Xyz([usize; 3]),
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct Dispatch {
    pub label: String,
    pub pipeline: Id,
    pub bind_groups: Vec<Id>,
    pub workgroups: Workgroups,
}

#[derive(Serialize)]
pub(super) struct TextureCopy {
    pub from: Id,
    pub to: Id,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct RawDispatch {
    pub run: Dispatch,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub clear_buffers: Option<Vec<Id>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub clear_textures: Option<Vec<TextureCopy>>,
}

#[derive(Serialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub(super) enum StepCopy {
    Buffer { a: Id, b: Id, to: Id, bytes: usize },
    Texture { a: Id, b: Id, to: Id },
}

#[derive(Serialize)]
pub(super) struct StepRuns {
    pub start: RawDispatch,
    pub odd: RawDispatch,
    pub even: RawDispatch,
    pub count: usize,
    pub keep: usize,
    pub history: Vec<StepCopy>,
}

#[derive(Serialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub(super) enum ComputeRun {
    Once(RawDispatch),
    Steps(Box<StepRuns>),
}

#[derive(Serialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub(super) enum BuildResource {
    Buffer { id: Id, bytes: usize },
    Texture { id: Id },
}

#[derive(Serialize)]
pub(super) struct BuildOutput {
    pub name: String,
    #[serde(flatten)]
    pub resource: BuildResource,
}

#[derive(Default, Serialize)]
pub(super) struct Build {
    pub runs: Vec<RawDispatch>,
    pub outputs: Vec<BuildOutput>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct BufferCopy {
    pub from: Id,
    pub from_offset: usize,
    pub to: Id,
    pub bytes: usize,
}

#[derive(Serialize)]
pub(super) struct Replay {
    pub label: String,
    pub slot: usize,
    pub rate: f64,
    pub start: f64,
    pub steps: usize,
    pub every: usize,
    pub first: Dispatch,
    pub ping: Dispatch,
    pub pong: Dispatch,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct GridCopy {
    pub slot: usize,
    pub from: Id,
    pub from_offset: usize,
    pub at: usize,
    pub instances: usize,
}

#[derive(Serialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub(super) enum BoundsRead {
    None,
    Read { buffer: Id, grids: Vec<GridCopy> },
}

#[derive(Clone, Copy, Serialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub(super) enum Target {
    Texture { texture: Id },
    Output,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct ColorAttachment {
    pub view: Target,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resolve_target: Option<Target>,
    pub load_op: &'static str,
    pub store_op: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub clear_value: Option<[f64; 4]>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct DepthAttachment {
    pub view: Id,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub depth_clear_value: Option<usize>,
    pub depth_load_op: &'static str,
    pub depth_store_op: &'static str,
}

#[derive(Serialize)]
#[serde(tag = "kind", rename_all = "lowercase", rename_all_fields = "camelCase")]
pub(super) enum DrawCommand {
    Fixed {
        pipeline: Id,
        vertices: usize,
        instances: usize,
    },
    Planned {
        pipeline: Id,
        draw: usize,
        #[serde(skip_serializing_if = "Option::is_none")]
        instance_base: Option<usize>,
    },
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct RenderPassDesc {
    pub label: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<ColorAttachment>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub depth: Option<DepthAttachment>,
    pub bind_groups: Vec<Id>,
    pub commands: Vec<DrawCommand>,
}

#[derive(Serialize)]
pub(super) struct PlanDraw<'a> {
    pub label: &'a str,
    pub layer: Layer,
    pub vertices: usize,
    pub instances: usize,
    pub detail: &'a Detail,
}

#[derive(Serialize)]
pub(super) struct TableEntry<'a> {
    pub name: &'a str,
    pub buffer: Id,
    pub offset: isize,
    pub vec4s: usize,
    pub lanes: usize,
    pub fill: &'a TableFillKind,
}

#[derive(Serialize)]
pub(super) struct MixedBuffer {
    pub size: usize,
    pub init: String,
}

#[derive(Serialize)]
pub(super) struct MixedPrecision {
    pub kind: &'static str,
    pub fallback: &'static str,
    pub modules: OrderedMap<Vec<Id>>,
    pub buffers: OrderedMap<MixedBuffer>,
}

#[derive(Serialize)]
pub(super) struct Blob<'a> {
    pub name: &'a str,
    pub bytes: usize,
}

#[derive(Serialize)]
pub(super) struct FloatWrite {
    pub buffer: Id,
    pub floats: usize,
}

#[derive(Serialize)]
pub(super) struct Frame {
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub compute: Vec<RawDispatch>,
    pub writes: [FrameWrite; 3],
    pub pose: Vec<Dispatch>,
    pub bounds: BoundsRead,
    pub detail: FloatWrite,
    pub passes: Vec<RenderPassDesc>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub replay: Vec<Replay>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub copies: Option<Vec<BufferCopy>>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct Manifest<'a> {
    pub format: &'static str,
    pub version: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub precision: Option<MixedPrecision>,
    pub chunks: Vec<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub authored: Option<OrderedMap<Vec<AuthorLines>>>,
    pub blobs: Vec<Blob<'a>>,
    pub inputs: &'a [InputSlot],
    pub input_floats: usize,
    pub params: &'a [ParamSlot],
    pub param_floats: usize,
    pub cameras: &'a OrderedMap<Camera>,
    pub curves: FloatWrite,
    pub buffers: Vec<BufferDesc>,
    pub textures: Vec<TextureDesc>,
    pub samplers: Vec<SamplerDesc>,
    pub bind_group_layouts: Vec<BindGroupLayoutDesc>,
    pub pipeline_layouts: Vec<PipelineLayoutDesc>,
    pub modules: Vec<ModuleDesc>,
    pub compute_pipelines: Vec<ComputePipelineDesc>,
    pub render_pipelines: Vec<RenderPipelineDesc>,
    pub bind_groups: Vec<BindGroupDesc>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub build: Option<Build>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub load: Vec<ComputeRun>,
    pub frame: Frame,
    pub dynamic: Vec<Id>,
    pub plan: Vec<PlanDraw<'a>>,
    pub tables: Vec<TableEntry<'a>>,
    pub stats: &'a Stats,
}
