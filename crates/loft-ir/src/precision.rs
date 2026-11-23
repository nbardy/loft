//! Numeric width is decided by the compiler from each value's role. The author's only control is `Precision::F32`.
//!
//! A resource is created through its `Role`, never through a format, so a role that must stay f32 cannot be given a
//! half format: `Role::format` has no half arm for it. See agent_notes/2026-11-23_perf/perf-precision.md.
use crate::{GpuFeatures, ImageFormat};

/// The one default switch. `Decided` stores each role at the width it needs; `F32` is the opt-out.
pub const DEFAULT: Precision = Precision::Decided;

/// The author's whole control: leave it alone (`Decided`), or force f32 everywhere for exact reproduction or debugging
/// (`--precision f32`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Precision {
    Decided,
    F32,
}

impl Default for Precision {
    fn default() -> Self { DEFAULT }
}

/// What a build actually ran with. Every variant is reported next to the frame, so a missing device feature is never a
/// silent fallback.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Resolved {
    /// Half storage, and the device has `SHADER_F16`, so half arithmetic is available to roles that allow it.
    Half,
    /// Half storage only: the adapter lacks `SHADER_F16`, so every shader stays f32 arithmetic.
    HalfStorage,
    /// Everything f32: `Precision::F32` was asked for.
    F32,
}

impl Precision {
    pub fn resolve(self, features: GpuFeatures) -> Resolved {
        match self {
            Precision::F32 => Resolved::F32,
            Precision::Decided if features.contains(GpuFeatures::F16) => Resolved::Half,
            Precision::Decided => Resolved::HalfStorage,
        }
    }
}

impl Resolved {
    pub fn name(self) -> &'static str {
        match self {
            Resolved::Half => "half",
            Resolved::HalfStorage => "half-storage (adapter lacks SHADER_F16)",
            Resolved::F32 => "f32 (requested)",
        }
    }
}

/// What an intermediate texture holds. Only roles with a resource today; a new role needs a resource that uses it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
    /// Shaded radiance between Shade and Present: half unless the opt-out is on.
    Radiance,
    /// Visibility words (kind, row, primitive): integer data, never a float.
    Integer,
    /// Depth and shadow depth: world-space distance, always f32.
    Depth,
}

impl Role {
    pub fn format(self, precision: Resolved) -> ImageFormat {
        match (self, precision) {
            (Role::Radiance, Resolved::F32) => ImageFormat::Rgba32Float,
            (Role::Radiance, Resolved::Half | Resolved::HalfStorage) => ImageFormat::Rgba16Float,
            (Role::Integer, _) => ImageFormat::Rg32Uint,
            (Role::Depth, _) => ImageFormat::Depth32Float,
        }
    }
}
