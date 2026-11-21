//! The lowered program: what the runner needs and nothing about the language. Module count is O(kinds) by
//! construction: a kind's module text never depends on its row count, its kind id or its row base (those are data in
//! the kind's uniform), so 1 and 20 instances lower to byte-identical modules.

pub use loft_check::program::{Dispatch, DispatchBinding, DispatchResource, DispatchRows, Draw, Instance, KindTable, abi};

#[derive(serde::Serialize, serde::Deserialize)]
pub struct ExpressionOrigin { pub function:usize,pub expression:usize,pub span:(String,usize,usize) }

#[derive(serde::Serialize,serde::Deserialize)]
pub struct ShaderModule {
    #[serde(default)]
    pub origins:Vec<ExpressionOrigin>,
    pub name: String,
    #[serde(skip)]
    pub wgsl: String,
    /// Author declaration (file, line, column); shared compiler modules have no source declaration.
    pub span: Option<(String, usize, usize)>,
    pub naga: naga::Module,
}

/// Perspective camera evaluated per frame at time t.
#[derive(Clone, Copy, Debug)]
#[derive(serde::Serialize, serde::Deserialize)]
pub struct Orbit {
    pub at: [f32; 3],
    pub radius: f32,
    pub height: f32,
    pub period: f32,
    pub fov: f32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fixed_eye: Option<[f32; 3]>,
}

impl Orbit {
    pub fn eye(&self, t: f32) -> [f32; 3] {
        if let Some(eye) = self.fixed_eye {
            return eye;
        }
        let a = std::f32::consts::TAU * t / self.period;
        [self.at[0] + self.radius * a.sin(), self.at[1] + self.height, self.at[2] + self.radius * a.cos()]
    }
}

/// The frame's passes in schedule order; one variant per pass kind, one handler each in the runner.
#[derive(Clone, Debug, PartialEq, Eq)]
#[derive(serde::Serialize, serde::Deserialize)]
pub enum Pass {
    /// One dispatch over every instance: frustum test, compact, one `DrawIndirectArgs` per kind.
    Cull,
    /// The RenderBundle: per kind, set pipeline and one `drawIndirect`, into Vis64 + depth.
    Vis(Vec<u32>),
    /// The sun's shadow map: a second bundle replaying these kinds' own vis pipelines, every row, from the sun.
    Shadow(Vec<u32>),
    /// One dispatch over 8×8 tiles: per kind, append the tile and bump its indirect dispatch.
    Classify,
    /// Per kind, one `dispatchWorkgroupsIndirect` over its tiles.
    Shade(Vec<u32>),
    /// Sky, ACES, sRGB to the target.
    Present,
}

#[derive(Debug)]
pub struct MotionSample {
    pub kind:String,
    pub fields:Vec<(String,Vec<f32>)>,
    pub checks:Vec<(String,Vec<[f32;2]>)>,
}

#[derive(serde::Serialize, serde::Deserialize)]
pub struct Program {
    #[serde(default)]
    pub bakes: Vec<crate::bake_tex::Bake>,
    #[serde(default)]
    pub path: Option<crate::emit::paths::PathTrace>,
    #[serde(default)]
    pub dispatches: Vec<Dispatch>,
    pub modules: Vec<ShaderModule>,
    pub kinds: Vec<KindTable>,
    pub instances: Vec<Instance>,
    pub camera: Orbit,
    pub sun: [f32; 3],
    /// Compiler-packed group 0 parameter storage.
    #[serde(default, with = "loft_check::program::words")]
    pub params: Vec<f32>,
    /// Authored scalar parameters occupy one vec4 slot each after three environment slots.
    #[serde(default)]
    pub param_ranges: Vec<(String, Option<[f32; 2]>)>,
    /// Shadow-map side in texels (0 without a shadow pass).
    pub shadow_size: u32,
    /// Baked scans and prior tables share the same word-addressed storage.
    pub luts: Vec<f32>,
    #[serde(default)]
    pub priors: std::collections::BTreeMap<String, u32>,
    pub passes: Vec<Pass>,
    /// Indices into `modules` of the three shared modules.
    pub cull: usize,
    pub classify: usize,
    pub present: usize,
}
