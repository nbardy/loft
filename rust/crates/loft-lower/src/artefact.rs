//! The compile.json boundary (src/artefact.ts, tools/port/sections.ts `plain`).
use serde::de::{self, MapAccess, Visitor};
use serde::ser::SerializeMap;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::{fmt, marker::PhantomData};

/// A JSON object in the compiler's insertion order.
#[derive(Clone, Debug, PartialEq)]
pub struct OrderedMap<T>(pub Vec<(String, T)>);

impl<T> Default for OrderedMap<T> {
    fn default() -> Self {
        Self(Vec::new())
    }
}

impl<T> OrderedMap<T> {
    pub fn get(&self, name: &str) -> Option<&T> {
        self.0.iter().find(|(key, _)| key == name).map(|(_, value)| value)
    }
}

impl<T: Serialize> Serialize for OrderedMap<T> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut map = serializer.serialize_map(Some(self.0.len()))?;
        for (key, value) in &self.0 {
            map.serialize_entry(key, value)?;
        }
        map.end()
    }
}

impl<'de, T: Deserialize<'de>> Deserialize<'de> for OrderedMap<T> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct Ordered<T>(PhantomData<T>);
        impl<'de, T: Deserialize<'de>> Visitor<'de> for Ordered<T> {
            type Value = OrderedMap<T>;

            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("an ordered JSON object")
            }

            fn visit_map<A: MapAccess<'de>>(self, mut input: A) -> Result<Self::Value, A::Error> {
                let mut pairs = Vec::with_capacity(input.size_hint().unwrap_or(0));
                while let Some(pair) = input.next_entry()? {
                    pairs.push(pair);
                }
                Ok(OrderedMap(pairs))
            }
        }
        deserializer.deserialize_map(Ordered(PhantomData))
    }
}

const BASE64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

#[derive(Serialize, Deserialize)]
struct EncodedArray<'a> {
    #[serde(rename = "type")]
    ty: &'a str,
    base64: &'a str,
}

fn read_array<'de, D: Deserializer<'de>>(d: D, ty: &str, width: usize) -> Result<Vec<u8>, D::Error> {
    let raw = EncodedArray::deserialize(d)?;
    if raw.ty != ty {
        return Err(de::Error::custom(format!("expected {ty}, got {}", raw.ty)));
    }
    let digit = |c| match c {
        b'A'..=b'Z' => Ok(c - b'A'),
        b'a'..=b'z' => Ok(c - b'a' + 26),
        b'0'..=b'9' => Ok(c - b'0' + 52),
        b'+' => Ok(62),
        b'/' => Ok(63),
        _ => Err(de::Error::custom("invalid base64 digit")),
    };
    let (groups, remainder) = raw.base64.as_bytes().as_chunks::<4>();
    let mut bytes = Vec::with_capacity(groups.len() * 3);
    for (i, &[a, b, c, e]) in groups.iter().enumerate() {
        let last = i + 1 == groups.len();
        let (a, b) = (digit(a)?, digit(b)?);
        bytes.push(a << 2 | b >> 4);
        match (c, e) {
            (b'=', b'=') if b & 15 == 0 && last => {}
            (c, b'=') if last => {
                let c = digit(c)?;
                if c & 3 != 0 {
                    return Err(de::Error::custom("invalid base64 padding"));
                }
                bytes.push(b << 4 | c >> 2);
            }
            (c, e) => {
                let (c, e) = (digit(c)?, digit(e)?);
                bytes.extend([b << 4 | c >> 2, c << 6 | e]);
            }
        }
    }
    if !remainder.is_empty() || !bytes.len().is_multiple_of(width) {
        return Err(de::Error::custom(format!("invalid {ty} byte length")));
    }
    Ok(bytes)
}

fn write_array<S: Serializer>(s: S, ty: &str, bytes: &[u8]) -> Result<S::Ok, S::Error> {
    let mut base64 = String::with_capacity(bytes.len().div_ceil(3) * 4);
    let digit = |n: u8| char::from(BASE64[usize::from(n)]);
    for group in bytes.chunks(3) {
        let a = group[0];
        let b = group.get(1).copied().unwrap_or(0);
        let c = group.get(2).copied().unwrap_or(0);
        base64.push(digit(a >> 2));
        base64.push(digit((a & 3) << 4 | b >> 4));
        base64.push(if group.len() >= 2 { digit((b & 15) << 2 | c >> 6) } else { '=' });
        base64.push(if group.len() == 3 { digit(c & 63) } else { '=' });
    }
    EncodedArray { ty, base64: &base64 }.serialize(s)
}

/// Little-endian bytes. Uploads carry the bits through without converting to host floats.
#[derive(Debug, Clone)]
pub struct Float32Array(pub Vec<u8>);

impl Float32Array {
    pub fn len(&self) -> usize {
        self.0.len() / 4
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl<'de> Deserialize<'de> for Float32Array {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        read_array(d, "Float32Array", 4).map(Self)
    }
}

impl Serialize for Float32Array {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        write_array(s, "Float32Array", &self.0)
    }
}

#[derive(Debug, Clone)]
pub struct Uint8Array(pub Vec<u8>);

impl<'de> Deserialize<'de> for Uint8Array {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        read_array(d, "Uint8Array", 1).map(Self)
    }
}

impl Serialize for Uint8Array {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        write_array(s, "Uint8Array", &self.0)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum Camera {
    Perspective { eye: [f64; 3], target: [f64; 3], fov_y: f64 },
    Orthographic { eye: [f64; 3], target: [f64; 3], height: f64 },
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum NumTy {
    F32,
    Vec2,
    Vec3,
    Vec4,
}

impl NumTy {
    pub fn floats(self) -> usize {
        match self {
            Self::F32 => 1,
            Self::Vec2 => 2,
            Self::Vec3 => 3,
            Self::Vec4 => 4,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::F32 => "f32",
            Self::Vec2 => "vec2",
            Self::Vec3 => "vec3",
            Self::Vec4 => "vec4",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InputSlot {
    pub name: String,
    pub ty: NumTy,
    pub slot: isize,
    pub def: Vec<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParamSlot {
    pub name: String,
    pub slot: isize,
    pub def: f64,
    pub min: f64,
    pub max: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DynamicBuffer {
    pub name: String,
    pub binding: usize,
    pub vec4s: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StaticBuffer {
    pub name: String,
    pub binding: usize,
    pub data: Float32Array,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PoseStage {
    pub label: String,
    pub wgsl: String,
    pub invocations: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum TableFillKind {
    Map { level: usize },
    Scan { level: usize },
    Cpu { why: String },
    Replay { recurrence: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum Kept {
    Every,
    Checkpoints { every: usize },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecurBake {
    pub wgsl: String,
    pub statics: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecurReplay {
    pub slot: usize,
    pub first: PoseStage,
    pub ping: PoseStage,
    pub pong: PoseStage,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Recurrence {
    pub name: String,
    pub entries: usize,
    pub lanes: usize,
    pub rate: f64,
    pub from: f64,
    pub total: usize,
    pub first: usize,
    pub steps: usize,
    pub start: f64,
    pub kept: Kept,
    pub states: usize,
    pub data: Float32Array,
    pub bake: RecurBake,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub replay: Option<RecurReplay>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TableInfo {
    pub name: String,
    pub buffer: String,
    pub offset: isize,
    pub vec4s: usize,
    pub lanes: usize,
    pub fill: TableFillKind,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TablePlace {
    pub buffer: String,
    pub offset: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Level {
    pub cells: [usize; 2],
    pub vertices: usize,
    pub cell: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum Detail {
    Fixed,
    Grid { slot: usize, bound: TablePlace, radius: f64, levels: Vec<Level> },
    // detailSlots spreads the projected spec, then appends its slot.
    Projected { cell: f64, plane: f64, reach: f64, lift: f64, slot: usize },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DrawPass {
    Opaque,
    Transparent,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Cull {
    None,
    Back,
    Front,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Blend {
    None,
    Alpha,
    Premultiplied,
    Add,
    AlphaMin,
    AlphaPull,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Layer {
    Scene,
    Construction,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Draw {
    pub label: String,
    pub detail: Detail,
    pub shader: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scene_pass_fragment: Option<String>,
    pub vertices: usize,
    pub instances: usize,
    pub pass: DrawPass,
    pub cull: Cull,
    pub depth_write: bool,
    pub blend: Blend,
    pub layer: Layer,
    pub constants: OrderedMap<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scene_depth: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub depth: Option<DrawDepth>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum DrawDepth {
    Vertex,
    Entry { entry: String },
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum TextureSize {
    View { scale: f64 },
    Fixed { width: usize, height: usize },
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PassKeep {
    Colour,
    Depth,
    Light,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PassLeftOut {
    pub label: String,
    pub why: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase", rename_all_fields = "camelCase")]
pub enum PassWhat {
    Scene { keep: PassKeep, camera: TablePlace, draws: Vec<usize>, left_out: Vec<PassLeftOut> },
    Screen { shader: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArtefactPass {
    pub name: String,
    pub size: TextureSize,
    pub reads: Vec<String>,
    pub what: PassWhat,
}

/// Group 3 holds every pass's binding. A pass binds a stand-in for its own texture and the ones drawn after it.
pub fn pass_bindings_wgsl(passes: &[ArtefactPass]) -> String {
    use std::fmt::Write;
    let mut text = "\n@group(3) @binding(0) var loft_pass_sampler: sampler;\n".to_owned();
    for (i, pass) in passes.iter().enumerate() {
        writeln!(text, "@group(3) @binding({}) var loft_passtex_{}: texture_2d<f32>;", i + 1, pass.name)
            .expect("writing to a String");
    }
    text
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PostPass {
    pub label: String,
    pub shader: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthorLines {
    pub from: usize,
    pub lines: usize,
    pub file: String,
    pub line: usize,
    pub what: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthorText {
    pub text: String,
    pub file: String,
    pub line: usize,
    pub what: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Extent {
    D2([usize; 2]),
    D3([usize; 3]),
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ComputeFormat {
    R32Float,
    Rgba8Unorm,
    Rgba16Float,
    Rgba32Float,
}

impl ComputeFormat {
    pub fn name(self) -> &'static str {
        match self {
            Self::R32Float => "r32float",
            Self::Rgba8Unorm => "rgba8unorm",
            Self::Rgba16Float => "rgba16float",
            Self::Rgba32Float => "rgba32float",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum ComputeShape {
    Buffer { entries: usize, lanes: usize },
    Texture { size: Extent, format: ComputeFormat },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComputeSteps {
    pub start: String,
    pub count: usize,
    pub rate: f64,
    pub keep: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum ComputeWhen {
    Build,
    Load,
    Frame,
    Steps(ComputeSteps),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComputeResource {
    #[serde(flatten)]
    pub shape: ComputeShape,
    pub key: String,
    pub symbol: String,
    pub bytes: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub steps: Option<ComputeWhen>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompiledPass {
    pub name: String,
    pub when: ComputeWhen,
    pub writes: Vec<(String, ComputeResource)>,
    pub reads: Vec<(String, ComputeResource)>,
    pub entry: String,
    pub dispatch: [usize; 3],
    pub wgsl: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComputeBinding {
    pub binding: usize,
    pub resource: ComputeResource,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompiledCompute {
    pub passes: Vec<CompiledPass>,
    pub resources: Vec<ComputeBinding>,
    pub wgsl: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Mixed {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scene_wgsl: Option<String>,
    pub shaders: OrderedMap<String>,
    pub statics: OrderedMap<Uint8Array>,
    pub stages: OrderedMap<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum Loop {
    Open,
    Every { seconds: f64 },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Stats {
    pub max_triangles: f64,
    pub draws: usize,
    pub curve_rows: usize,
    pub stations: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Artefact {
    pub version: usize,
    pub clear: [f64; 3],
    pub inputs: Vec<InputSlot>,
    pub input_floats: usize,
    pub params: Vec<ParamSlot>,
    pub curves: Float32Array,
    pub dynamic: Vec<DynamicBuffer>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub statics: Option<Vec<StaticBuffer>>,
    pub scene_wgsl: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mixed: Option<Mixed>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notes: Option<Vec<String>>,
    pub shaders: OrderedMap<String>,
    pub draws: Vec<Draw>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub post: Option<Vec<PostPass>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub passes: Option<Vec<ArtefactPass>>,
    pub stages: Vec<PoseStage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub compute: Option<CompiledCompute>,
    pub tables: Vec<TableInfo>,
    pub cameras: OrderedMap<Camera>,
    #[serde(rename = "loop", skip_serializing_if = "Option::is_none")]
    pub clock: Option<Loop>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub recurrences: Option<Vec<Recurrence>>,
    pub stats: Stats,
    /// The compiler's authored-text registry, if the input producer carries it across the boundary.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub authored: Option<Vec<AuthorText>>,
}
