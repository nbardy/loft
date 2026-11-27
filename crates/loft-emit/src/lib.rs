//! Emit WGSL from checked Loft programs.
pub mod bake_tex;
pub mod emit;
pub mod events;
#[path = "world/mesh.rs"]
mod mesh;
pub mod provenance;
pub mod quant;

/// Traversal shared by nearest-hit and segment any-hit. Supply the two arrays and
/// `loft_bvh_intersect(leaf: LoftLeafRef, ray: LoftRay) -> f32` in the consuming module.
pub const BVH_WGSL: &str = concat!(include_str!("slab.wgsl"), include_str!("bvh.wgsl"));
