//! Shared std430 wire records (06 §8 and 02 §3). Integers occupy integer lanes.
mod advance;
mod tier0;
pub use advance::{AdvanceArgs, advance};
pub use tier0::Tier0Opcode;

pub mod det;
pub mod rand;
pub const DET_WGSL: &str = include_str!("det.wgsl");

#[repr(C, align(16))]
#[derive(Clone, Copy, Debug, Default)]
pub struct BodyRow {
    pub com_local: [f32; 3],
    pub mass: f32,
    pub inv_inertia: [[f32; 4]; 3],
    pub inv_mass: f32,
    pub rest_h: f32,
    pub tc: f32,
    pub mu: f32,
    pub strength: f32,
    pub chain_start: u32,
    pub chain_count: u32,
    pub seg_start: u32,
    pub seg_count: u32,
    pub eye_chain: u32,
    pub flags: u32,
    pub pad: u32,
}

#[repr(C, align(16))]
#[derive(Clone, Copy, Debug, Default)]
pub struct ChainRow {
    pub socket_pos: [f32; 3],
    pub role: u32,
    pub socket_rot: [f32; 4],
    pub rest_end: [f32; 3],
    pub ik_kind: u32,
    pub lens: [f32; 4],
    pub pitch_lo: f32,
    pub pitch_hi: f32,
    pub yaw_lo: f32,
    pub yaw_hi: f32,
    pub twist_lo: f32,
    pub twist_hi: f32,
    pub mu_s: f32,
    pub mu_k: f32,
    pub max_load: f32,
    pub seg_count: u32,
    pub pose_offset: u32,
    pub neighbours: u32,
}

#[repr(C, align(16))]
#[derive(Clone, Copy, Debug, Default)]
pub struct SegmentRow {
    pub rest_pos: [f32; 3],
    pub mass: f32,
    pub rest_rot: [f32; 4],
    pub half_extents: [f32; 3],
    pub parent_seg: i32,
}

#[repr(C, align(16))]
#[derive(Clone, Copy, Debug, Default)]
pub struct CreatureHeader {
    pub pos: [f32; 3],
    pub composure: f32,
    pub vel: [f32; 3],
    pub squash_q: f32,
    pub acc: [f32; 3],
    pub squash_v: f32,
    pub rot: [f32; 4],
    pub omega: [f32; 3],
    pub tick: u32,
    pub prev_rot: [f32; 4],
    pub head_world: [f32; 3],
    pub yaw_unwrapped: f32,
    pub skull_q: [f32; 4],
    pub bob_phase: f32,
    pub handoff: u32,
    pub glance_err: f32,
    pub pad: u32,
    pub body_id: u32,
    pub feet_start: u32,
    pub feet_count: u32,
    pub manip_start: u32,
    pub manip_count: u32,
    pub trail_start: u32,
    pub hit_window: f32,
    pub chained_hits: u32,
}

#[repr(C, align(16))]
#[derive(Clone, Copy, Debug, Default)]
pub struct FootPin {
    pub local_p: [f32; 3],
    pub lift: f32,
    pub local_n: [f32; 3],
    pub f_n: f32,
    pub from_p: [f32; 3],
    pub f_t: f32,
    pub aim_p: [f32; 3],
    pub phase: f32,
    pub surface_id: u32,
    pub mode: u32,
    pub dur: f32,
    pub down_t: f32,
}

#[repr(C, align(16))]
#[derive(Clone, Copy, Debug, Default)]
pub struct ManipState {
    pub target: [f32; 3],
    pub authority: f32,
    pub vel: [f32; 3],
    pub active: u32,
}

#[repr(C, align(16))]
#[derive(Clone, Copy, Debug, Default)]
pub struct TrailNode {
    pub pos: [f32; 3],
    pub inv_m: f32,
    pub prev: [f32; 3],
    pub pad: f32,
}

#[repr(C, align(16))]
#[derive(Clone, Copy, Debug, Default)]
pub struct PoseJoint {
    pub pos: [f32; 3],
    pub scale: f32,
    pub rot: [f32; 4],
}

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
