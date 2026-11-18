//! The bundle (plan §1.9): one file, `scene.loft`. A 16-byte header (magic `LOFT`, u32 format, u64 JSON length), the
//! serde JSON of `Bundle`, then each binary section (tables, WGSL text) at a 64-byte boundary. Numbers are carried as
//! bits (`F32`), so the bytes are the same on every platform and this crate does no float arithmetic.
//! Ids index the bundle's own lists (resources, kernels, modules, media, ...), or sections, or `names`.
use crate::error::{Error, Internal, LoadFault};
use crate::graph::{Site, Ty};
use serde::{Deserialize, Serialize};

const MAGIC: [u8; 4] = *b"LOFT";
const ALIGN: usize = 64;

pub type Id = u32;

/// An f32 as its bits.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct F32(pub u32);

/// The only format. Any other header value is `Load(FormatMismatch)`; an incompatible change adds `V2` and deletes
/// `V1` in the same commit unless a persisted consumer exists (plan §2.4).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Format {
    V1,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Bundle {
    pub format: Format,
    /// The Loft commit that wrote it.
    pub compiler: Box<str>,
    pub names: Box<[Box<str>]>,
    pub resources: Box<[Resource]>,
    pub kernels: Box<[Kernel]>,
    pub modules: Box<[Module]>,
    pub draws: Box<[Draw]>,
    pub passes: Box<[Pass]>,
    pub media: Box<[Medium]>,
    /// (name, camera root) per named camera.
    pub cameras: Box<[(Id, Id)]>,
    pub params: Box<[ParamDecl]>,
    pub inputs: Box<[InputDecl]>,
    pub sites: Box<[Site]>,
    /// Binary sections; the JSON carries only their lengths.
    #[serde(with = "lengths")]
    pub sections: Box<[Box<[u8]>]>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Resource {
    pub shape: Shape,
    pub init: Init,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Shape {
    Rows {
        rows: u32,
        lanes: u8,
    },
    Texture {
        extent: [u32; 3],
        format: TexFormat,
    },
    ViewSized {
        format: TexFormat,
        scale: F32,
    },
    /// The per-frame uniform.
    Frame,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum TexFormat {
    Rgba8Unorm,
    Rgba16Float,
    R32Float,
    Depth32Float,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Init {
    Zero,
    /// Filled at compile on the tape: a section.
    Bytes(Id),
    /// Filled by a kernel.
    By(Id),
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Kernel {
    pub writes: Box<[Id]>,
    pub reads: Box<[Id]>,
    pub body: Body,
    pub when: When,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Body {
    Map { module: Id, entry: Id, rows: u32 },
    Scan { module: Id, entry: Id, rows: u32 },
    Raw { module: Id, entry: Id, dispatch: [u32; 3] },
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum When {
    Load,
    Frame,
    Steps { dt: F32, from: F32, count: u32, keep: Keep },
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Keep {
    Latest,
    /// Every(1) = full history; Every(k) = checkpoints.
    Every(u32),
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Module {
    /// The section holding its WGSL text.
    pub wgsl: Id,
    pub origin: Origin,
    /// Written with the `lf` alias, which the host defines as f32 or f16 at load.
    pub precision_alias: bool,
    /// The resources it reads, in binding order: `@group(1) @binding(k)` is `reads[k]` (emit's `Emitted::reads`).
    pub reads: Box<[Id]>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Origin {
    Library,
    Author,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Draw {
    pub name: Id,
    pub layer: Layer,
    pub cover: Cover,
    /// (module, entry) of the paint function.
    pub shade: (Id, Id),
    pub raster: Raster,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Layer {
    Subject,
    Surroundings,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Cover {
    Raster { prim: RasterPrim, count: u32 },
    Ray { proxy: Proxy, find: Find },
    Screen,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum RasterPrim {
    Grid { u: u32, v: u32 },
    Mesh { res: Id },
    Quads,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Find {
    Hit,
    Medium(Id),
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Proxy {
    Box { min: [F32; 3], max: [F32; 3] },
    Instances { res: Id, local: [[F32; 3]; 2] },
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Raster {
    pub cull: bool,
    pub depth_write: bool,
    pub blend: Blend,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Blend {
    Opaque,
    Alpha,
    Add,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Pass {
    pub target: Target,
    pub camera: Id,
    /// draws[0]..draws[1]
    pub draws: [u32; 2],
    pub clear: Clear,
    pub compose: Compose,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Target {
    View,
    Texture(Id),
    Shadow { light: Id, map: Id },
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Clear {
    Load,
    Color([F32; 4]),
}
/// Outline and ground, composed once per pass (plan §1.5); (module, entry) where present.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Compose {
    pub outline: Option<(Id, Id)>,
    pub ground: Option<(Id, Id)>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Medium {
    /// (module, entry) of its `transmittance(p, dir) -> f32`.
    pub transmittance: (Id, Id),
    pub integrate: Integrate,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Integrate {
    March { steps: u16, shadow_steps: u16 },
    Gaussians { res: Id },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ParamDecl {
    pub name: Id,
    pub def: F32,
    pub min: F32,
    pub max: F32,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct InputDecl {
    pub name: Id,
    pub ty: Ty,
    pub def: Box<[F32]>,
}

/// Sections as their lengths in the JSON; `Bundle::read` fills the bytes.
mod lengths {
    use serde::{Deserialize, Deserializer, Serializer};
    type Sections = Box<[Box<[u8]>]>;
    pub fn serialize<S: Serializer>(s: &[Box<[u8]>], ser: S) -> Result<S::Ok, S::Error> {
        ser.collect_seq(s.iter().map(|b| b.len() as u64))
    }
    pub fn deserialize<'de, D: Deserializer<'de>>(de: D) -> Result<Sections, D::Error> {
        Ok(Vec::<u64>::deserialize(de)?.into_iter().map(|n| vec![0; n as usize].into()).collect())
    }
}

fn pad(out: &mut Vec<u8>) {
    out.resize(out.len().next_multiple_of(ALIGN), 0);
}

impl Bundle {
    pub fn write(&self) -> Result<Vec<u8>, Error> {
        let json = serde_json::to_vec(self).map_err(|e| Error::Internal(Internal::Json(e.to_string().into())))?;
        let mut out =
            Vec::with_capacity(16 + json.len() + self.sections.iter().map(|s| s.len() + ALIGN).sum::<usize>());
        out.extend_from_slice(&MAGIC);
        out.extend_from_slice(&1u32.to_le_bytes());
        out.extend_from_slice(&(json.len() as u64).to_le_bytes());
        out.extend_from_slice(&json);
        for s in &self.sections {
            pad(&mut out);
            out.extend_from_slice(s);
        }
        Ok(out)
    }

    pub fn read(bytes: &[u8]) -> Result<Bundle, Error> {
        let short = |need: usize| Error::Load(LoadFault::Truncated { need: need as u64, have: bytes.len() as u64 });
        let head = bytes.get(..16).ok_or_else(|| short(16))?;
        (head[..4] == MAGIC).then_some(()).ok_or(Error::Load(LoadFault::NotBundle))?;
        let format = u32::from_le_bytes([head[4], head[5], head[6], head[7]]);
        let len = u64::from_le_bytes([head[8], head[9], head[10], head[11], head[12], head[13], head[14], head[15]]);
        let end = usize::try_from(len).ok().and_then(|l| l.checked_add(16)).ok_or_else(|| short(usize::MAX))?;
        let json = bytes.get(16..end).ok_or_else(|| short(end))?;
        if format != 1 {
            #[derive(Deserialize)]
            struct Stamp {
                compiler: Box<str>,
            }
            let compiler = serde_json::from_slice::<Stamp>(json).ok().map(|s| s.compiler);
            return Err(Error::Load(LoadFault::FormatMismatch { found: format, expected: 1, compiler }));
        }
        let refuse = |e: serde_json::Error| Error::Load(LoadFault::Json(e.to_string().into()));
        // The declared section lengths must fit in the file before any is allocated.
        #[derive(Deserialize)]
        struct Lengths {
            sections: Box<[u64]>,
        }
        let need = serde_json::from_slice::<Lengths>(json)
            .map_err(refuse)?
            .sections
            .iter()
            .try_fold(end as u64, |at, n| at.checked_next_multiple_of(ALIGN as u64)?.checked_add(*n));
        need.filter(|n| *n <= bytes.len() as u64).ok_or_else(|| short(need.map_or(usize::MAX, |n| n as usize)))?;
        let mut b: Bundle = serde_json::from_slice(json).map_err(refuse)?;
        let mut at = end;
        for s in b.sections.iter_mut() {
            at = at.next_multiple_of(ALIGN);
            let stop = at.checked_add(s.len()).ok_or_else(|| short(usize::MAX))?;
            s.copy_from_slice(bytes.get(at..stop).ok_or_else(|| short(stop))?);
            at = stop;
        }
        // Bytes after the last section would give the same Bundle a second file and a second `Identity::build`.
        if at != bytes.len() {
            return Err(Error::Load(LoadFault::Trailing { end: at as u64, have: bytes.len() as u64 }));
        }
        Ok(b)
    }
}
