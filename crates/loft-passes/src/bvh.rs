//! World BVH and the compiled-program CPU twin.
mod twin;
pub use twin::{CpuWorld, Hit};
pub use loft_emit::BVH_WGSL as WGSL;
pub use loft_eval::bvh::{Bvh2, BvhError, Ray};
