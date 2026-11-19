//! The lowered program: what the runner needs and nothing about the language. Module count is O(kinds) by
//! construction: a kind's module text never depends on its row count, its kind id or its row base (those are data in
//! the kind's uniform), so 1 and 20 instances lower to byte-identical modules.

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

pub struct ShaderModule {
    pub name: String,
    pub wgsl: String,
    pub naga: naga::Module,
}

/// One kind's storage table and how it is drawn.
pub struct KindTable {
    pub name: String,
    pub module: usize,
    /// f32 words per row, hidden columns included.
    pub stride: u32,
    pub rows: Vec<f32>,
    pub n_rows: u32,
    /// The derived Lipschitz bound per row (also in the hidden column), for reports.
    pub lipschitz: Vec<f32>,
    pub draw: Draw,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Draw {
    /// `query visible`: closed-form ray hit in the proxy's fragment.
    Analytic,
    /// `query trace` only: sphere traced with the row's derived L.
    Traced,
    /// `query surface`: a `nu × nv` grid evaluated in the vertex stage; the record carries (u, v).
    Surface { nu: u32, nv: u32 },
}

impl Draw {
    /// Vertices of one row's draw: the proxy box, or two triangles per grid cell.
    pub fn vertices(self) -> u32 {
        match self {
            Draw::Analytic | Draw::Traced => 36,
            Draw::Surface { nu, nv } => 6 * nu * nv,
        }
    }
}

/// One row's derived world AABB, for the shared cull.
#[derive(Clone, Copy, Debug)]
pub struct Instance {
    pub lo: [f32; 3],
    pub hi: [f32; 3],
    pub kind: u32,
    pub row: u32,
}

/// Camera from the schedule's `orbit` pass, evaluated per frame at time t.
#[derive(Clone, Copy, Debug)]
pub struct Orbit {
    pub at: [f32; 3],
    pub radius: f32,
    pub height: f32,
    pub period: f32,
    pub fov: f32,
}

impl Orbit {
    pub fn eye(&self, t: f32) -> [f32; 3] {
        let a = std::f32::consts::TAU * t / self.period;
        [self.at[0] + self.radius * a.sin(), self.at[1] + self.height, self.at[2] + self.radius * a.cos()]
    }
}

/// The frame's passes in schedule order; one variant per pass kind, one handler each in the runner.
#[derive(Clone, Debug, PartialEq, Eq)]
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

pub struct Program {
    pub modules: Vec<ShaderModule>,
    pub kinds: Vec<KindTable>,
    pub instances: Vec<Instance>,
    pub camera: Orbit,
    pub sun: [f32; 3],
    /// Shadow-map side in texels (0 without a shadow pass).
    pub shadow_size: u32,
    /// Every scan's baked table, concatenated (12 words per frame).
    pub luts: Vec<f32>,
    pub passes: Vec<Pass>,
    /// Indices into `modules` of the three shared modules.
    pub cull: usize,
    pub classify: usize,
    pub present: usize,
}
