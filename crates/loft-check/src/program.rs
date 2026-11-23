//! GPU-independent records shared by the compiler passes.

/// Fixed binding contract between the passes' generated WGSL and the runner (plan §1 layer 0).
pub mod abi {
    /// Every module: group 0, binding 0, the `Frame` uniform (`FRAME_BYTES`): camera, inverse, eye, sun, size,
    /// cold (x nonce guard, y 1 for the sun's orthographic shadow view, z 1 when a shadow map is bound), sun camera.
    pub const FRAME_BYTES: u64 = 256;
    /// Kind modules, group 1: 0 rows (`array<f32>`), 1 visible row ids (`array<u32>`), 2 the kind's `KindParams`,
    /// 3 the program's scan tables (`array<f32>`).
    /// Shade, group 2: 0 Vis64 (`texture_2d<u32>`), 1 HDR colour (storage `rgba16float`, write), 2 tile lists,
    /// 3 the sun's shadow depth (`texture_depth_2d`), 4 its comparison sampler.
    /// Twin, group 3: 0 samples in (`array<vec4<f32>>`), 1 results out (`array<vec4<f32>>`, 4 per sample).
    pub const KIND_PARAMS_BYTES: u64 = 16;
    /// Words of the hidden row columns the compiler appends: derived AABB lo (3), hi (3), Lipschitz L (1).
    pub const HIDDEN_WORDS: u32 = 7;
    /// Pixels per tile side; classify and shade use 8×8 workgroups.
    pub const TILE: u32 = 8;
    /// The Vis64 word meaning "nothing drawn".
    pub const EMPTY: u32 = u32::MAX;
    /// Kinds per program: one bit each in the classify mask, 8 bits in Vis64 (kind:8 | row:24).
    pub const MAX_KINDS: usize = 32;
    pub const MAX_ROWS: u32 = 1 << 24;
    /// The cold-load guard: every entry starts `if frame.cold.x == LOFT_NONCE`; the uniform is always 0, so a
    /// distinct nonce changes the code the driver compiles and nothing it does.
    pub const NONCE_PLACEHOLDER: &str = "4294967295u";
}

/// One kind's storage table and how it is drawn.
#[derive(serde::Serialize, serde::Deserialize)]
pub struct KindTable {
    pub name: String,
    pub module: usize,
    /// f32 words per row, hidden columns included.
    pub stride: u32,
    #[serde(with = "words")]
    pub rows: Vec<f32>,
    pub n_rows: u32,
    /// The derived Lipschitz bound per row (also in the hidden column), for reports.
    pub lipschitz: Vec<f32>,
    pub draw: Draw,
    /// Kinds (indices into the program's kinds) whose rows this kind's queries read, bound at group 1, slot 4 + j.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub reads: Vec<u32>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[derive(serde::Serialize, serde::Deserialize)]
pub enum Draw {
    /// `query visible`: closed-form ray hit in the proxy's fragment.
    Analytic,
    /// `query trace` only: sphere traced with the row's derived L.
    Traced,
    /// `query surface`: a `nu × nv` grid evaluated in the vertex stage; the record carries (u, v).
    Surface { nu: u32, nv: u32 },
    Mesh { vertices: u32 },
}

impl Draw {
    /// Vertices of one row's draw: the proxy box, or two triangles per grid cell.
    pub fn vertices(self) -> u32 {
        match self {
            Draw::Analytic | Draw::Traced => 36,
            Draw::Surface { nu, nv } => 6 * nu * nv,
            Draw::Mesh { vertices } => vertices,
        }
    }
}

/// One row's derived world AABB, for the shared cull.
#[derive(Clone, Copy, Debug)]
#[derive(serde::Serialize, serde::Deserialize)]
pub struct Instance {
    pub lo: [f32; 3],
    pub hi: [f32; 3],
    pub kind: u32,
    pub row: u32,
}


/// A fixed-step dispatch and its packed row and verdict layouts.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct Dispatch {
    #[serde(default, skip_serializing_if = "loft_parse::ast::Determinism::is_det")]
    pub tier: loft_parse::ast::Determinism,
    pub module: usize,
    pub entry: String,
    pub workgroup_x: u32,
    pub input: DispatchRows,
    pub bindings: Vec<DispatchBinding>,
    pub fields: Vec<(String, u32)>,
    pub layout_hash: [u8; 32],
    pub checks: Vec<String>,
    /// Tables the laws and checks read (`[K]` parameters), in binding order (`DispatchResource::Read(j)`).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub reads: Vec<DispatchRead>,
    /// Runs one tick per dispatch, in program order with the other lockstep dispatches (a cross-kind read).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub lockstep: bool,
}

/// One table a fixed-step dispatch reads. A read of the dispatch's own kind (`previous`) sees the rows as the last
/// tick left them, through a copy the `snapshot` entry takes before each tick; any other read binds the live table.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct DispatchRead {
    pub kind: String,
    /// The kind's rows when no dispatch writes them: a drawn kind's table, or the packed words of a data kind.
    pub input: DispatchRows,
    pub previous: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum DispatchResource { Rows, Arguments, Results, Outbox, Luts, Read(u32), Previous }

#[derive(Clone, Copy, Debug, serde::Serialize, serde::Deserialize)]
pub struct DispatchBinding {
    pub group: u32,
    pub slot: u32,
    pub resource: DispatchResource,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub enum DispatchRows {
    Drawable(usize),
    Table {
        name: String,
        n_rows: u32,
        stride: u32,
        #[serde(with = "words")]
        words: Vec<f32>,
    },
}

impl DispatchRows {
    pub fn view<'a>(&'a self, kinds: &'a [KindTable]) -> Option<(&'a str, u32, u32, &'a [f32])> {
        match self {
            Self::Drawable(index) => kinds.get(*index).map(|k| (k.name.as_str(), k.n_rows, k.stride, k.rows.as_slice())),
            Self::Table { name, n_rows, stride, words } => Some((name, *n_rows, *stride, words)),
        }
    }
}

/// A storage word can contain an integer bitcast to a float, including a NaN
/// payload. Decimal float serialization would lose that row's identity.
pub mod words {
    use serde::{Deserialize, Deserializer, Serialize, Serializer};
    pub fn serialize<S: Serializer>(v: &[f32], s: S) -> Result<S::Ok, S::Error> {
        v.iter()
            .map(|x| x.to_bits())
            .collect::<Vec<_>>()
            .serialize(s)
    }
    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<f32>, D::Error> {
        Ok(Vec::<u32>::deserialize(d)?
            .into_iter()
            .map(f32::from_bits)
            .collect())
    }
}
