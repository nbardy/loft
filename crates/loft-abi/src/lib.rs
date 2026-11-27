//! Shared std430 wire records (06 §8 and 02 §3). Integers occupy integer lanes.
mod advance;
pub use advance::{AdvanceArgs, advance};

pub mod det;
pub mod rand;
pub const DET_WGSL: &str = include_str!("det.wgsl");

/// Node zero is the root of a nonempty tree. With count == 0 the two children
/// are consecutive nodes at left_or_first and left_or_first + 1. Otherwise
/// count consecutive LeafRefs start at left_or_first (indices, never bytes).
#[repr(C, align(16))]
#[derive(Clone, Copy, Debug, Default)]
pub struct BvhNode {
    pub lo: [f32; 3],
    pub left_or_first: u32,
    pub hi: [f32; 3],
    pub count: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LeafRef {
    pub kind_id: u32,
    pub row_id: u32,
}

/// Wire values for `VerdictRecord::tag` (02 §3.2).
#[repr(u32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VerdictTag {
    Skipped = 0,
    Held = 1,
    Missed = 2,
    Unknown = 3,
}

/// Wire values for `VerdictRecord::reason_code`.
#[repr(u32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VerdictReason {
    None = 0,
    IterCap = 1,
    NonFinite = 2,
    MissingSurface = 3,
    MissingFrame = 4,
    SolveFailed = 5,
}

// Raw u32 lanes allow checking a GPU record before interpreting either discriminant.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct VerdictRecord {
    pub constraint_id: u32,
    pub instance_id: u32,
    pub site_id: u32,
    pub tick: u32,
    pub tag: u32,
    pub reason_code: u32,
    pub value: f32,
    pub law_hash_lo: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(transparent)]
pub struct VisRec(pub [u16; 4]);
impl VisRec {
    pub fn pack(kind: u32, ink: bool, row: u32, payload: u32) -> Option<Self> {
        if kind >= 512 || row >= 1 << 22 {
            return None;
        }
        let id = kind << 23 | u32::from(ink) << 22 | row;
        Some(Self([
            id as u16,
            (id >> 16) as u16,
            payload as u16,
            (payload >> 16) as u16,
        ]))
    }
    pub fn unpack(self) -> (u32, bool, u32, u32) {
        let [a, b, c, d] = self.0.map(u32::from);
        let id = a | b << 16;
        (id >> 23, id & (1 << 22) != 0, id & 0x3fffff, c | d << 16)
    }
}

/// Bundle-wide mesh asset index, stored in an integer lane (06 §§7, 18).
#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MeshAsset(pub u32);

/// Authored columns of the Mesh kind, before the compiler's derived bounds.
/// Like other xf rows, pose holds three packed basis columns then translation.
/// Each column is three scalar words in RowTables, with no vec3 padding.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MeshRow {
    pub pose: [[f32; 3]; 4],
    pub asset: MeshAsset,
    pub lod: u32,
}
