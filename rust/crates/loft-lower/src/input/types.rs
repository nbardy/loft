pub use loft_ir::graph::{Cull, Keep, Ty};
pub use loft_ir::wire::FnRef;

#[derive(Debug)]
pub struct SceneDecl {
    pub sun: [f64; 3],
    pub ground: Ground,
    pub clear: [f64; 3],
    pub outline: Outline,
    /// None is an open timeline.
    pub period: Option<f64>,
    pub screen_ink: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ground {
    Nothing,
    Plane,
    Sea,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outline {
    Hull,
    Screen,
}

#[derive(Debug)]
pub struct Camera {
    pub name: String,
    pub eye: [f64; 3],
    pub target: [f64; 3],
    pub lens: CameraLens,
}

#[derive(Clone, Copy, Debug)]
pub enum CameraLens {
    Perspective { fov_y: f64 },
    Orthographic { height: f64 },
}

#[derive(Debug)]
pub struct Thing {
    pub kind: ThingKind,
    pub name: String,
    pub payload: ThingPayload,
    pub tape: Emitted,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ThingKind {
    Surface,
    Relief,
    Ground,
    Sky,
    Sea,
    Strand,
    Telegraph,
    Smear,
    Field,
    Traced,
    Mesh,
    Copies,
    Impostors,
    Shader,
    Cast,
    Light,
    Shadows,
    Volume,
    Density,
    Glow,
    Lightning,
    Rain,
    Specks,
    Wake,
    Unowned(String),
}

#[derive(Debug)]
pub struct ThingPayload {
    pub words: Box<[u32]>,
    pub lits: Box<[f64]>,
    pub fns: Box<[FnRef]>,
    pub blobs: Box<[BlobRange]>,
}

#[derive(Clone, Copy, Debug)]
pub struct BlobRange {
    pub offset: u32,
    pub length: u32,
}

#[derive(Debug)]
pub struct Emitted {
    pub shaders: Box<[Shader]>,
    pub draws: Box<[DrawSpec]>,
    pub posts: Box<[Post]>,
    pub notes: Box<[String]>,
}

#[derive(Clone, Debug)]
pub struct Shader {
    pub key: String,
    pub code: String,
    pub depth_entry: Option<String>,
    pub mixed_code: Option<String>,
    /// The shaders' functions, already in the order TS gave them.
    pub libs: Box<[u32]>,
}

#[derive(Debug)]
pub struct Post {
    pub label: String,
    pub wgsl: String,
    pub mixed_wgsl: Option<String>,
}

#[derive(Debug)]
pub struct DrawSpec {
    pub label: String,
    pub shader: String,
    pub scene_pass_fragment: Option<String>,
    pub vertices: f64,
    pub instances: f64,
    pub phase: Phase,
    pub cull: Cull,
    pub depth_write: bool,
    pub blend: Blend,
    pub layer: Layer,
    pub scene_depth: Option<bool>,
    pub constants: Box<[(String, f64)]>,
    pub depth: Option<DrawDepth>,
    pub detail: DetailSpec,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    Background,
    Opaque,
    OnBodies,
    Water,
    OnWater,
    Transparent,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Blend {
    None,
    Alpha,
    Premultiplied,
    Add,
    AlphaMin,
    AlphaPull,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Layer {
    Scene,
    Construction,
}

#[derive(Debug)]
pub enum DrawDepth {
    Vertex,
    Entry(String),
}

#[derive(Debug)]
pub enum DetailSpec {
    Fixed,
    Grid { bound: u32, counts: Box<[[f64; 2]]>, point: u32, anchor: u32 },
    Projected { cell: f64, plane: f64, reach: f64, lift: f64 },
}

#[derive(Debug)]
pub struct Effect {
    pub stage: Stage,
    pub op: EffectOp,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stage {
    Compute,
    Things,
    Style,
    Passes,
    Cast,
    Layout,
    Rest,
    Details,
}

#[derive(Debug)]
pub enum EffectOp {
    Register(Box<[u32]>),
    Declare(u32),
    Lower(Program),
}

#[derive(Debug)]
pub struct Program {
    pub name: String,
    pub roots: Box<[u32]>,
    pub vars: Box<[(String, Rate)]>,
    pub runs: Rate,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Rate {
    Build,
    Frame,
    Copy,
    Item,
}

#[derive(Debug)]
pub struct EmitRequest {
    pub name: String,
    pub args: Box<[(String, Ty)]>,
    /// WGSL return spelling.
    pub ret: String,
    pub roots: Box<[u32]>,
    pub pick: u32,
}

#[derive(Debug)]
pub struct Lighting {
    pub sun: Sun,
    pub amount: SunAmount,
    pub sampling: Sampling,
    pub at: u32,
    pub xi: u32,
    pub normal: u32,
    pub lights: Box<[Arrival]>,
}

#[derive(Debug)]
pub enum Sun {
    Fixed([f64; 3]),
    Input { name: String, dir: [f64; 3] },
    Expression(u32),
}
#[derive(Clone, Copy, Debug)]
pub enum SunAmount {
    Full,
    Given(u32),
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sampling {
    Point,
    Sampled,
}

#[derive(Debug)]
pub struct Arrival {
    pub toward: u32,
    pub amount: u32,
    pub distance: Option<u32>,
    pub size: Option<u32>,
    pub shadow: Option<Shadow>,
}

#[derive(Clone, Copy, Debug)]
pub struct Shadow {
    pub point: u32,
    pub surface: u32,
}

#[derive(Debug)]
pub struct LampPass {
    pub light: u32,
    pub pass: String,
    pub at: u32,
    pub half: u32,
}

#[derive(Debug)]
pub struct ComputePass {
    pub kernel: u32,
    pub reads: Box<[ComputeRead]>,
}

#[derive(Debug)]
pub struct ComputeRead {
    pub symbol: String,
    pub kernel: u32,
    pub output: u32,
}

#[derive(Debug)]
pub struct Recurrence {
    pub kernel: u32,
    pub name: String,
    pub total: u32,
    pub start: f64,
    pub after: Box<[u32]>,
    pub replay: Option<Replay>,
}

#[derive(Clone, Copy, Debug)]
pub struct Replay {
    pub even: u32,
    pub odd: u32,
    pub step: u32,
}

#[derive(Debug)]
pub struct PassDef {
    pub name: String,
    pub texture: String,
    pub size: TextureSize,
    pub what: PassTarget,
    pub discovery_order: Option<u32>,
}

#[derive(Clone, Copy, Debug)]
pub enum TextureSize {
    View { scale: f64 },
    Fixed { width: u32, height: u32 },
}

#[derive(Debug)]
pub enum PassTarget {
    Scene { keep: Keep, camera: u32 },
    Screen { colour: u32, uv: String, shader: Option<Shader> },
}

#[derive(Debug)]
pub struct Authored {
    pub text: String,
    pub file: String,
    pub line: u32,
    pub what: String,
}

#[derive(Debug)]
pub struct Cast {
    pub wgsl: String,
    pub libs: Box<[u32]>,
}

#[derive(Debug)]
pub struct RawFlags {
    pub gpu_only: bool,
    pub raw_compute: Option<String>,
}
