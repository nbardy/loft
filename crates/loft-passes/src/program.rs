//! The lowered program: what the runner needs and nothing about the language. Module count is O(kinds) by
//! construction: a kind's module text never depends on its row count, its kind id or its row base (those are data in
//! the kind's uniform), so 1 and 20 instances lower to byte-identical modules.

pub use loft_check::program::{Dispatch, DispatchBinding, DispatchRead, DispatchResource, DispatchRows, Draw, Instance, KindTable, abi};

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
#[derive(Clone, PartialEq, Eq)]
#[derive(serde::Serialize, serde::Deserialize)]
pub enum Pass {
    /// The RenderBundle: per kind, set pipeline and one `drawIndirect`, into Vis64 + depth.
    Vis(Vec<u32>),
    /// The sun's shadow map: a second bundle replaying these kinds' own vis pipelines, every row, from the sun.
    Shadow(Vec<u32>),
    /// Per kind, one `dispatchWorkgroupsIndirect` over its tiles.
    Shade(Vec<u32>),
    /// A frame pass the schedule wrote from a template (the stock cull, classify and present among them).
    Authored(AuthoredPass),
    /// A path-traced picture: this module's full-screen pass draws the whole frame.
    Path(usize),
}

impl Pass {
    /// Whether the pass draws to the swapchain.
    pub fn presents(&self) -> bool {
        match self {
            Self::Path(_) => true,
            Self::Authored(a) => matches!(a.run, AuthoredRun::FullScreen { .. }),
            Self::Vis(_) | Self::Shadow(_) | Self::Shade(_) => false,
        }
    }
}

// Reports and gate tables name passes by their Debug head; an authored pass is named by its schedule label.
impl std::fmt::Debug for Pass {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Vis(k) => f.debug_tuple("Vis").field(k).finish(),
            Self::Shadow(k) => f.debug_tuple("Shadow").field(k).finish(),
            Self::Shade(k) => f.debug_tuple("Shade").field(k).finish(),
            Self::Authored(a) => f.write_str(&a.label),
            Self::Path(m) => f.debug_tuple("Path").field(m).finish(),
        }
    }
}

/// A schedule-authored frame pass: its module, how it runs, and the frame resource behind each binding.
#[derive(Clone, Debug, PartialEq, Eq)]
#[derive(serde::Serialize, serde::Deserialize)]
pub struct AuthoredPass {
    pub label: String,
    pub module: usize,
    pub run: AuthoredRun,
    /// (group, binding) as the template declares it, and the frame resource bound there.
    pub bindings: Vec<(u32, u32, FrameView)>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[derive(serde::Serialize, serde::Deserialize)]
pub enum AuthoredRun {
    Compute { entry: String, extent: Extent },
    /// One full-screen triangle into the swapchain.
    FullScreen { vertex: String, fragment: String },
}

/// Workgroups of a compute frame pass.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[derive(serde::Serialize, serde::Deserialize)]
pub enum Extent {
    Groups([u32; 3]),
    /// One invocation per instance, `width` per workgroup (the entry's `@workgroup_size`).
    Instances { width: u32 },
    /// One invocation per pixel, `tile` per workgroup (the entry's `@workgroup_size`).
    Pixels { tile: [u32; 2] },
}

/// The frame ABI a template binds by WGSL global name.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[derive(serde::Serialize, serde::Deserialize)]
pub enum FrameView {
    /// `insts`: every drawn row's World bounds, kind and row.
    Instances,
    /// `bases`: each drawn kind's first slot in `visible`.
    KindBases,
    /// `draw_args`: one indirect draw per kind.
    DrawArgs,
    /// `visible`: the rows each kind draws.
    Visible,
    /// `vis_tex`: Vis64, sampled.
    Visibility,
    /// `hdr`: ColorHdr as a storage target.
    HdrStore,
    /// `hdr_tex`: ColorHdr, sampled.
    HdrSample,
    /// `shade_args`: one indirect dispatch per kind.
    ShadeArgs,
    /// `tiles`: each kind's 8x8 tile list.
    Tiles,
}

/// Where a frame view lives: the schedule store a pass must list to bind it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ViewStore {
    /// Any drawn kind's rows: the pass lists the drawn kinds.
    Kinds,
    Named(&'static str),
}

impl FrameView {
    /// The view a WGSL global of this name binds, its store, and whether binding it writes the store.
    pub fn named(name: &str) -> Option<(Self, ViewStore, bool)> {
        use ViewStore::{Kinds, Named};
        Some(match name {
            "insts" => (Self::Instances, Kinds, false),
            "bases" => (Self::KindBases, Kinds, false),
            "draw_args" => (Self::DrawArgs, Named("IndirectBuf"), true),
            "visible" => (Self::Visible, Named("IndirectBuf"), true),
            "vis_tex" => (Self::Visibility, Named("Vis64"), false),
            "hdr" => (Self::HdrStore, Named("ColorHdr"), true),
            "hdr_tex" => (Self::HdrSample, Named("ColorHdr"), false),
            "shade_args" => (Self::ShadeArgs, Named("TileLists"), true),
            "tiles" => (Self::Tiles, Named("TileLists"), true),
            _ => return None,
        })
    }
}

/// When the shadow step redraws its map.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[derive(serde::Serialize, serde::Deserialize)]
pub enum StepClock {
    /// When its casters or the light change. The host proves which casters are static; this is also the default.
    #[default]
    Dirty,
    /// Once: after the first frame the map persists, whatever moves.
    Load,
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
    /// Baked scans share the word-addressed storage.
    pub luts: Vec<f32>,
    pub passes: Vec<Pass>,
    #[serde(default)]
    pub shadow_clock: StepClock,
}

impl Program {
    /// The module a pass or kind of this name compiled to.
    pub fn module_named(&self, name: &str) -> Option<usize> {
        self.modules.iter().position(|m| m.name == name)
    }
}
