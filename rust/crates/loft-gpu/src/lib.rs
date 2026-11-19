//! loft-gpu: the host (plan 2026-11-19 §1.10, §4, §5). It loads a `scene.loft` bundle on a device the caller owns and
//! draws its frames: write the frame uniform, run `Frame` kernels, step `Steps` kernels to the time, run the passes in
//! order. It never links the compiler. Inputs in, pixels out, nothing back (Keel's rule): hit shapes stay in the game's
//! sim, and a Loft attack is built from the sim's timing, never the reverse.
//!
//! ```ignore
//! let canvas = Canvas { color: format, depth: wgpu::TextureFormat::Depth32Float, samples: 1 };
//! let mut loft = pollster::block_on(Loft::load(&device, &queue, &bytes, Precision::Mixed, &[canvas], None))?;
//! loft.set_time(t);
//! loft.set_input("strike.at", &[x, z])?;
//! loft.encode_into(&device, &queue, &mut encoder, &View { canvas, color: &color, resolve: None, depth: &depth, size: [w, h],
//!                                                         owner: Owner::Caller, layers: Layers::Subject })?;
//! queue.submit([encoder.finish()]);
//! ```
//!
//! The binding contract the emitter writes against (loft-core emit, lane 2.2; draws, lane 3.1):
//! - `@group(0)`: emit's prelude (loft-core `emit::prelude`, bytes in `loft_ir::frame`): the frame uniform (camera,
//!   viewport, time, sun, then params and inputs), `loft_detail`, and the opaque scene's depth, clearance and colour.
//! - `@group(1) @binding(p)`: pack p (`Bundle::packs`), `array<vec4<f32>>`, the same layout for every module of a stage:
//!   read-only storage in render pipelines; in compute pipelines `Access::Write` packs are read-write. A shader reads
//!   resource r at its pack offset, a constant in its text, so a scene binds at most a handful of buffers however
//!   many tables it has. Textures are not bound at group 1: no module reads one yet (pass targets and shadow maps only).
//! - `@group(2) @binding(k)`: a kernel's `writes[k]`: rows as the whole buffer holding them, read-write (a fill writes
//!   its pack in place through group 1; a step writes its next state here), textures as write-only storage.
//! - A `Map` entry is `@workgroup_size(64)` over `rows` invocations; a `Scan` entry runs as one invocation; `Raw` uses
//!   its own dispatch.
//! - A draw's module has exactly one `@vertex` entry; `shade` names the fragment entry, writing location 0. Vertices ×
//!   instances: Grid 6uv × count; Mesh rows(res) × count; Quads 6 × count; Ray 36 (a box) × 1, or × rows(res) for
//!   `Instances`; Screen 3 × 1. Compose entries are Screen draws: ground before the pass's draws, outline after.
//! - Stepped state: a `Steps` kernel writes one rows resource. `Keep::Every(k)` runs every step at load and keeps state
//!   `first + j·k` in slot j of the resource (`first` = round(-from/dt), loft-core `kernel::kept`); readers pick and
//!   replay. `Keep::Latest` steps live to n = floor((t - from)/dt) per frame and rewinds from the init. A step binds the
//!   previous state in place of its resource and writes the next; step i sees a frame whose time is `from + i·dt`.
//! - `Origin::Library` is the bundle writer's promise that the module holds no author text, that every index it makes is
//!   in range (a `Fetch` clamps to `arrayLength`; constant tables are indexed by the host's own vertex counts) and that
//!   every loop has a static bound. Such modules compile without Naga's runtime bounds and loop checks (§4.3). A module
//!   with any author WGSL is `Origin::Author` and keeps every check.
//!
//! Many copies of one subject (Keel step 8) share one `Loft`: `instance()` per copy, `prepare` once per frame, and
//! `draw_subject` into the game's own render pass, so there is no pass per copy:
//!
//! ```ignore
//! let mut bosses: Vec<Instance> = (0..5).map(|_| loft.instance(&device, &queue)).collect::<Result<_, _>>()?;
//! bosses[1].set_input(&loft, "at", &[4.0, 0.0, 0.0])?;
//! loft.prepare(&queue, &mut encoder, &bosses);
//! let mut pass = encoder.begin_render_pass(&game_pass);
//! loft.draw_subject(&mut pass, canvas, &bosses)?;
//! ```
#[cfg(not(target_arch = "wasm32"))]
mod cache;
mod ticks;
mod validate;
mod view;

#[cfg(not(target_arch = "wasm32"))]
pub use cache::PipelineCache;
pub use validate::validate;
pub use view::Aim;

use loft_ir::Error;
use loft_ir::bundle::{
    Access, Blend, Body, Bundle, Clear, Cover, Cull, Id, Init, Keep, Kernel, Layer, Module, Origin, Pack, Proxy,
    Raster, RasterPrim, Shape, Target, TexFormat, When,
};
use loft_ir::frame::{self, Camera};
use loft_ir::identity::{FrameKey, Precision};
use std::collections::HashMap;
use wgpu::{BindGroup, BindGroupLayout, Buffer, ShaderStages, TextureFormat, TextureView};

/// Loft's own depth: offscreen passes and shadow maps.
pub const DEPTH: TextureFormat = TextureFormat::Depth32Float;
const STEP_STRIDE: u64 = 256;

/// Who owns a `View`'s attachments: Loft clears them as each pass says; a caller's are loaded and never cleared.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Owner {
    Loft,
    Caller,
}
/// Which draws reach the view: everything, or the subject alone over a game's own surroundings (Keel).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Layers {
    All,
    Subject,
}
/// The attachment formats a view draws with; pipelines are built per canvas at load.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Canvas {
    pub color: TextureFormat,
    pub depth: TextureFormat,
    pub samples: u32,
}
pub struct View<'a> {
    pub canvas: Canvas,
    /// The colour attachment: multisampled when `canvas.samples` > 1, then resolved into `resolve` (TS src/host/lower.ts
    /// drew R0 4x multisampled).
    pub color: &'a TextureView,
    /// Where a multisampled `color` resolves: the single-sampled picture. None when `canvas.samples` is 1.
    pub resolve: Option<&'a TextureView>,
    pub depth: &'a TextureView,
    pub size: [u32; 2],
    pub owner: Owner,
    pub layers: Layers,
}

/// One copy of a loaded subject (Keel step 8): its own time, params and inputs, and its own copy of every resource a
/// `Frame` kernel writes. `Load` state, `Every` history and pipelines stay shared with the `Loft` it came from.
pub struct Instance {
    pub time: f32,
    params: Vec<f32>,
    inputs: Vec<f32>,
    frame: Buffer,
    g0: BindGroup,
    g1: Packs<BindGroup>,
    g2: Vec<BindGroup>,
    timelines: std::cell::RefCell<Vec<ticks::Timeline>>,
    _own: (Vec<Buffer>, Vec<Res>),
}
impl Instance {
    pub fn set_param(&mut self, loft: &Loft, name: &str, value: f32) -> Result<(), Error> {
        self.params[loft.param_slot(name)?] = value;
        Ok(())
    }
    pub fn set_input(&mut self, loft: &Loft, name: &str, value: &[f32]) -> Result<(), Error> {
        let slot = loft.input_slot(name, value.len())?;
        for (s, p) in self
            .timelines
            .get_mut()
            .iter_mut()
            .zip(&loft.programs)
            .filter(|(_, p)| p.spec.input as usize == slot.start / 4)
        {
            for row in 0..p.count {
                s.write(p, row, ((self.time * p.spec.hz as f32) as u32).max(1), value, None)?;
            }
        }
        self.inputs[slot].copy_from_slice(value);
        Ok(())
    }
}

/// A resource on the device. Rows live in a pack (`Loft::packs`) at a vec4 offset. A texture a pass draws into
/// (`Target::Texture`) also owns that pass's depth; no other texture gets one, since a 3D table cannot have a depth
/// texture and a large one would double its memory.
enum Res {
    Rows { pack: usize, offset: u64 },
    Tex(Box<Tex>),
    Target { tex: Box<Tex>, depth: TextureView },
    Frame,
}

struct Tex {
    tex: wgpu::Texture,
    view: TextureView,
}

/// What one `Steps` kernel needs from its `When`.
#[derive(Clone, Copy)]
pub(crate) struct Stepping {
    from: f32,
    dt: f32,
    count: u32,
    keep: Keep,
    first: u32,
    pub(crate) kept: u32,
}
impl Stepping {
    pub(crate) fn of(k: &Kernel) -> Option<Stepping> {
        match k.when {
            When::Steps { dt, from, count, keep } => {
                let (from, dt) = (f32::from_bits(from.0), f32::from_bits(dt.0));
                let first = (-from / dt).round().clamp(0.0, count as f32) as u32;
                let kept = match keep {
                    Keep::Latest => 1,
                    Keep::Every(k) => 1 + (count - first) / k.max(1),
                };
                Some(Stepping { from, dt, count, keep, first, kept })
            }
            When::Load | When::Frame => None,
        }
    }
    fn at(&self, t: f32) -> u32 {
        ((t - self.from) / self.dt).floor().clamp(0.0, self.count as f32) as u32
    }
}

/// A stepped kernel's state buffers: step i reads `x[(i - 1) % 2]` and writes `x[i % 2]`. `x[0]` is the resource's
/// pack itself under `Latest` (validated: a stepped resource is alone in its pack, at offset 0), a scratch under
/// `Every`. `groups[p]` are (group 1, group 2) for steps of parity p: the pack bound to the previous state, the next.
struct Stepper {
    kernel: usize,
    res: usize,
    pack: usize,
    s: Stepping,
    x: [Buffer; 2],
    base: u64,
    done: u32,
    groups: Vec<(BindGroup, BindGroup)>,
}

/// A draw or a compose entry, as the passes run it.
struct Item {
    module: Id,
    entry: Id,
    layer: Layer,
    raster: Raster,
    screen: bool,
    count: [u32; 2],
}

type Key = (Option<TextureFormat>, TextureFormat, u32);
/// The bind groups one copy of the subject dispatches with: (group 0, compute group 1, group 2 per kernel).
type Groups<'a> = (&'a BindGroup, &'a BindGroup, &'a [BindGroup]);

/// Group 1 as each stage binds it: one layout and one bind group per stage, not per module.
struct Packs<T> {
    render: T,
    compute: T,
}

pub struct Loft {
    b: Bundle,
    build: [u8; 32],
    precision: Precision,
    canvases: Vec<Canvas>,
    res: Vec<Res>,
    /// `Bundle::packs` on the device.
    packs: Vec<Buffer>,
    frame: Buffer,
    steps: Buffer,
    /// `loft_detail` (emit's prelude, `@group(0) @binding(1)`): zero until the level-of-detail plan is ported.
    detail: Buffer,
    /// `loft_scene_depth`, `loft_scene_clearance` and `loft_post_colour` (emit's prelude, bindings 2 to 4): 1 × 1, far
    /// and clear, until a pass resolves the opaque draws into them (view.rs).
    scene: view::Scene,
    resolve: view::Resolve,
    /// Per pass: the item before which it resolves the scene (view.rs's rule), if any.
    resolve_at: Vec<Option<usize>>,
    cameras: view::Cameras,
    aim: Aim,
    bgl0: BindGroupLayout,
    frame_size: u64,
    bgl1: Packs<BindGroupLayout>,
    bgl2: Vec<BindGroupLayout>,
    g0: [BindGroup; 2],
    g1: Packs<BindGroup>,
    g2: Vec<BindGroup>,
    compute: Vec<wgpu::ComputePipeline>,
    render: HashMap<(usize, Key), wgpu::RenderPipeline>,
    items: Vec<Item>,
    pass_items: Vec<Vec<usize>>,
    steppers: Vec<Stepper>,
    size: [u32; 2],
    time: f32,
    params: Vec<f32>,
    inputs: Vec<f32>,
    timestamps: Option<wgpu::QuerySet>,
    frame_dispatches: usize,
    programs: Vec<ticks::Program>,
    timelines: Vec<ticks::Timeline>,
}

pub(crate) fn prelude(m: &Module, p: Precision) -> &'static str {
    match (m.precision_alias, p) {
        (false, _) => "",
        (true, Precision::F32) => "alias lf = f32;\n",
        (true, Precision::Mixed) => "enable f16;\nalias lf = f16;\n",
    }
}

fn texel(f: TexFormat) -> (TextureFormat, u64) {
    match f {
        TexFormat::Rgba8Unorm => (TextureFormat::Rgba8Unorm, 4),
        TexFormat::Rgba16Float => (TextureFormat::Rgba16Float, 8),
        TexFormat::R32Float => (TextureFormat::R32Float, 4),
        TexFormat::Depth32Float => (TextureFormat::Depth32Float, 4),
    }
}

/// Bytes of one state: rows are vec4 lanes; a texture is tightly packed texels.
pub(crate) fn state_bytes(s: &Shape) -> u64 {
    match *s {
        Shape::Rows { rows, lanes } => u64::from(rows) * u64::from(lanes) * 16,
        Shape::Texture { extent, format } => extent.iter().map(|&e| u64::from(e)).product::<u64>() * texel(format).1,
        Shape::ViewSized { .. } | Shape::Frame => 0,
    }
}

fn rows_of(b: &Bundle, r: Id) -> u32 {
    match b.resources[r as usize].shape {
        Shape::Rows { rows, .. } => rows,
        Shape::Texture { .. } | Shape::ViewSized { .. } | Shape::Frame => 0,
    }
}

fn floats(f: &[f32]) -> Vec<u8> {
    f.iter().flat_map(|x| x.to_le_bytes()).collect()
}

/// A kernel write's binding: rows as the whole buffer of their pack (`packs`, the loft's or an instance's copies).
fn binding<'a>(r: &'a Res, packs: &dyn Fn(usize) -> &'a Buffer) -> wgpu::BindingResource<'a> {
    match r {
        Res::Rows { pack, .. } => packs(*pack).as_entire_binding(),
        Res::Tex(t) | Res::Target { tex: t, .. } => wgpu::BindingResource::TextureView(&t.view),
        Res::Frame => unreachable!("validated: group 2 never binds the frame"),
    }
}

fn storage(binding: u32, read_only: bool) -> wgpu::BindGroupLayoutEntry {
    let ty = wgpu::BindingType::Buffer {
        ty: wgpu::BufferBindingType::Storage { read_only },
        has_dynamic_offset: false,
        min_binding_size: None,
    };
    let visibility =
        if read_only { ShaderStages::VERTEX_FRAGMENT | ShaderStages::COMPUTE } else { ShaderStages::COMPUTE };
    wgpu::BindGroupLayoutEntry { binding, visibility, ty, count: None }
}

/// Group 1's two layouts: every pack read-only for render pipelines; written packs read-write for compute ones.
fn pack_layouts(device: &wgpu::Device, packs: &[Pack]) -> Packs<BindGroupLayout> {
    let make = |write: bool| {
        let entries: Vec<_> = packs
            .iter()
            .enumerate()
            .map(|(p, pack)| storage(p as u32, !(write && pack.access == Access::Write)))
            .collect();
        device
            .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor { label: Some("loft packs"), entries: &entries })
    };
    Packs { render: make(false), compute: make(true) }
}

/// A group 1 bind group over `buffers`, binding p = `buffers(p)`.
fn pack_group<'a>(
    device: &wgpu::Device,
    layout: &BindGroupLayout,
    n: usize,
    buffers: &dyn Fn(usize) -> &'a Buffer,
) -> BindGroup {
    let entries: Vec<_> =
        (0..n).map(|p| wgpu::BindGroupEntry { binding: p as u32, resource: buffers(p).as_entire_binding() }).collect();
    device.create_bind_group(&wgpu::BindGroupDescriptor { label: Some("loft packs"), layout, entries: &entries })
}

/// A bind group of `list`, binding k = `list[k]` as `at` resolves it (the loft's own, an instance's or a step state).
fn group<'a>(
    device: &wgpu::Device,
    layout: &BindGroupLayout,
    list: &[Id],
    at: &dyn Fn(usize) -> wgpu::BindingResource<'a>,
) -> BindGroup {
    let entries: Vec<_> = list
        .iter()
        .enumerate()
        .map(|(k, &r)| wgpu::BindGroupEntry { binding: k as u32, resource: at(r as usize) })
        .collect();
    device.create_bind_group(&wgpu::BindGroupDescriptor { label: None, layout, entries: &entries })
}

/// The frame uniform (loft_ir::frame): the camera header, then params and inputs.
fn frame_bytes(camera: &Camera, size: [u32; 2], time: f32, params: &[f32], inputs: &[f32]) -> Vec<u8> {
    let (w, h) = (size[0] as f32, size[1] as f32);
    let mut out = frame::header(camera, [w, h, 1.0 / w, 1.0 / h], time).to_vec();
    out.extend(floats(&[params, inputs].concat()));
    out
}

/// The camera before the caller sets one: identity matrices, the eye at the origin looking down -z, the sun overhead.
const NO_CAMERA: Camera = Camera {
    view_proj: [1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0],
    inv_view_proj: [1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0],
    eye: [0.0, 0.0, 0.0, 1.0],
    forward: [0.0, 0.0, -1.0, 0.0],
    sun: [0.0, 1.0, 0.0, 0.0],
};

/// Group 0's entries (emit's prelude, `loft_ir::frame`): the frame (dynamic offset, so step frames share one buffer),
/// `loft_detail`, and the opaque scene's depth, clearance and colour textures.
fn group0_layout() -> [wgpu::BindGroupLayoutEntry; 6] {
    let buffer = |dynamic: bool| wgpu::BindingType::Buffer {
        ty: wgpu::BufferBindingType::Uniform,
        has_dynamic_offset: dynamic,
        min_binding_size: None,
    };
    let visibility = ShaderStages::VERTEX_FRAGMENT | ShaderStages::COMPUTE;
    let texture = wgpu::BindingType::Texture {
        sample_type: wgpu::TextureSampleType::Float { filterable: false },
        view_dimension: wgpu::TextureViewDimension::D2,
        multisampled: false,
    };
    [
        wgpu::BindGroupLayoutEntry { binding: 0, visibility, ty: buffer(true), count: None },
        wgpu::BindGroupLayoutEntry { binding: 1, visibility, ty: buffer(false), count: None },
        wgpu::BindGroupLayoutEntry { binding: 2, visibility, ty: texture, count: None },
        wgpu::BindGroupLayoutEntry { binding: 3, visibility, ty: texture, count: None },
        wgpu::BindGroupLayoutEntry {
            binding: 4,
            visibility,
            ty: wgpu::BindingType::Texture {
                sample_type: wgpu::TextureSampleType::Float { filterable: true },
                view_dimension: wgpu::TextureViewDimension::D2,
                multisampled: false,
            },
            count: None,
        },
        wgpu::BindGroupLayoutEntry {
            binding: 5,
            visibility,
            ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
            count: None,
        },
    ]
}

fn group0<'a>(
    frame: &'a Buffer,
    frame_size: u64,
    detail: &'a Buffer,
    scene: &'a view::Scene,
) -> [wgpu::BindGroupEntry<'a>; 6] {
    let sampler = &scene.sampler;
    let scene = &scene.views;
    [
        wgpu::BindGroupEntry {
            binding: 0,
            resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                buffer: frame,
                offset: 0,
                size: wgpu::BufferSize::new(frame_size),
            }),
        },
        wgpu::BindGroupEntry { binding: 1, resource: detail.as_entire_binding() },
        wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::TextureView(&scene[0]) },
        wgpu::BindGroupEntry { binding: 3, resource: wgpu::BindingResource::TextureView(&scene[1]) },
        wgpu::BindGroupEntry { binding: 4, resource: wgpu::BindingResource::TextureView(&scene[2]) },
        wgpu::BindGroupEntry { binding: 5, resource: wgpu::BindingResource::Sampler(sampler) },
    ]
}

/// Library modules without Naga's runtime bounds and loop checks (§4.3); author modules with every check.
#[allow(unsafe_code)]
fn shader_module(device: &wgpu::Device, origin: Origin, wgsl: String) -> wgpu::ShaderModule {
    let desc = wgpu::ShaderModuleDescriptor { label: None, source: wgpu::ShaderSource::Wgsl(wgsl.into()) };
    match origin {
        Origin::Author => device.create_shader_module(desc),
        Origin::Library => {
            let checks =
                wgpu::ShaderRuntimeChecks { bounds_checks: false, force_loop_bounding: false, ..Default::default() };
            // SAFETY: `Origin::Library` is the writer's promise (crate docs, the binding contract) that no index leaves
            // its array and every loop is statically bounded: emit clamps each `Fetch` to `arrayLength`, loop counts
            // are static, and constant tables are indexed by the vertex counts this host draws. Naga's integer-division
            // guards are not a switch and stay; ray-query tracking keeps its default.
            unsafe { device.create_shader_module_trusted(desc, checks) }
        }
    }
}

/// Group 2's entry for one kernel write: rows read-write (their whole pack), a texture as write-only storage.
fn write_entry(binding: u32, shape: &Shape) -> wgpu::BindGroupLayoutEntry {
    let (format, view_dimension) = match *shape {
        Shape::Texture { format, extent } if extent[2] > 1 => (format, wgpu::TextureViewDimension::D3),
        Shape::Texture { format, .. } | Shape::ViewSized { format, .. } => (format, wgpu::TextureViewDimension::D2),
        // validated: group 2 never holds the frame
        Shape::Rows { .. } | Shape::Frame => return storage(binding, false),
    };
    let ty = wgpu::BindingType::StorageTexture {
        access: wgpu::StorageTextureAccess::WriteOnly,
        format: texel(format).0,
        view_dimension,
    };
    wgpu::BindGroupLayoutEntry { binding, visibility: ShaderStages::COMPUTE, ty, count: None }
}

impl Loft {
    /// Reads, validates and loads a bundle on the caller's device: every pipeline for every `canvas` is compiled here.
    /// `Mixed` runs where the device has `SHADER_F16` and `F32` elsewhere; `precision()` and `frame_key` say which.
    pub async fn load(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        bytes: &[u8],
        ask: Precision,
        canvases: &[Canvas],
        cache: Option<&wgpu::PipelineCache>,
    ) -> Result<Loft, Error> {
        let b = Bundle::read(bytes)?;
        let precision = match ask {
            Precision::Mixed if device.features().contains(wgpu::Features::SHADER_F16) => Precision::Mixed,
            Precision::Mixed | Precision::F32 => Precision::F32,
        };
        validate(&b, precision, &device.limits())?;
        let scope = device.push_error_scope(wgpu::ErrorFilter::Validation);
        let loft = Loft::create(device, queue, b, loft_ir::identity::build(bytes), precision, canvases, cache);
        match scope.pop().await {
            None => Ok(loft),
            Some(e) => Err(Error::Environment(format!("the device refused a validated bundle: {e}").into())),
        }
    }

    fn create(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        b: Bundle,
        build: [u8; 32],
        precision: Precision,
        canvases: &[Canvas],
        cache: Option<&wgpu::PipelineCache>,
    ) -> Loft {
        let p = b.params.len().div_ceil(4).max(1);
        let i = b.inputs.len().max(1);
        let frame_bytes = (frame::HEADER + 16 * (p + i)) as u64;
        let mut params = vec![0.0; p * 4];
        let mut inputs = vec![0.0; i * 4];
        for (k, d) in b.params.iter().enumerate() {
            params[k] = f32::from_bits(d.def.0);
        }
        for (k, d) in b.inputs.iter().enumerate() {
            for (j, x) in d.def.iter().enumerate() {
                inputs[k * 4 + j] = f32::from_bits(x.0);
            }
        }
        let usage = wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST;
        let frame = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("loft frame"),
            size: frame_bytes.max(STEP_STRIDE),
            usage,
            mapped_at_creation: false,
        });
        let stride = frame_bytes.next_multiple_of(STEP_STRIDE);
        let steppings: Vec<(usize, Stepping)> =
            b.kernels.iter().enumerate().filter_map(|(k, kern)| Stepping::of(kern).map(|s| (k, s))).collect();
        let entries: u64 = steppings.iter().map(|(_, s)| u64::from(s.count) + 1).sum();
        let steps = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("loft step frames"),
            // At least one stride: group 0 binds a whole frame from it even when nothing steps.
            size: (entries * stride).max(stride),
            usage,
            mapped_at_creation: false,
        });
        let detail = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("loft_detail"),
            size: 16 * frame::DETAIL_SLOTS as u64,
            usage,
            mapped_at_creation: false,
        });
        let scene = view::Scene::new(device, queue, [1, 1], TextureFormat::Rgba8Unorm);
        let bgl0 = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("loft frame"),
            entries: &group0_layout(),
        });
        let g0 = [&frame, &steps].map(|buffer| {
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("loft frame"),
                layout: &bgl0,
                entries: &group0(buffer, frame_bytes, &detail, &scene),
            })
        });
        let shape = |r: Id| &b.resources[r as usize].shape;
        let bgl1 = pack_layouts(device, &b.packs);
        let bgl2: Vec<_> = b
            .kernels
            .iter()
            .map(|k| {
                let entries: Vec<_> =
                    k.writes.iter().enumerate().map(|(i, &r)| write_entry(i as u32, shape(r))).collect();
                device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor { label: None, entries: &entries })
            })
            .collect();
        let packs: Vec<Buffer> = b
            .packs
            .iter()
            .map(|p| {
                let usage = wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::COPY_SRC;
                device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("loft pack"),
                    size: p.vec4s * 16,
                    usage,
                    mapped_at_creation: false,
                })
            })
            .collect();
        let g1 = Packs {
            render: pack_group(device, &bgl1.render, packs.len(), &|p| &packs[p]),
            compute: pack_group(device, &bgl1.compute, packs.len(), &|p| &packs[p]),
        };
        let modules: Vec<_> = b
            .modules
            .iter()
            .map(|m| {
                let text = std::str::from_utf8(&b.sections[m.wgsl as usize]).expect("validated UTF-8");
                shader_module(device, m.origin, format!("{}{text}", prelude(m, precision)))
            })
            .collect();
        let name = |n: Id| &*b.names[n as usize];
        let compute = b
            .kernels
            .iter()
            .enumerate()
            .map(|(k, kern)| {
                let (module, entry) = entry_of(kern);
                let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: None,
                    bind_group_layouts: &[Some(&bgl0), Some(&bgl1.compute), Some(&bgl2[k])],
                    immediate_size: 0,
                });
                device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                    label: Some(name(entry)),
                    layout: Some(&layout),
                    module: &modules[module as usize],
                    entry_point: Some(name(entry)),
                    compilation_options: Default::default(),
                    cache,
                })
            })
            .collect();
        // Items: the draws, then each pass's compose entries; each pass lists its own in order.
        let mut items: Vec<Item> = b
            .draws
            .iter()
            .map(|d| {
                let count = match &d.cover {
                    Cover::Raster { prim: RasterPrim::Grid { u, v }, count } => [6 * u * v, *count],
                    Cover::Raster { prim: RasterPrim::Mesh { res }, count } => [rows_of(&b, *res), *count],
                    Cover::Raster { prim: RasterPrim::Quads, count } => [6, *count],
                    Cover::Ray { proxy: Proxy::Box { .. }, .. } => [36, 1],
                    Cover::Ray { proxy: Proxy::Instances { res, .. }, .. } => [36, rows_of(&b, *res)],
                    Cover::Screen => [3, 1],
                };
                let screen = matches!(d.cover, Cover::Screen);
                Item { module: d.shade.0, entry: d.shade.1, layer: d.layer, raster: d.raster, screen, count }
            })
            .collect();
        let over = Raster { cull: Cull::None, depth_write: false, blend: Blend::Alpha };
        let mut pass_items = Vec::new();
        for p in b.passes.iter() {
            let mut compose = |e: &Option<(Id, Id)>| {
                e.map(|(module, entry)| {
                    items.push(Item {
                        module,
                        entry,
                        layer: Layer::Surroundings,
                        raster: over,
                        screen: true,
                        count: [3, 1],
                    });
                    items.len() - 1
                })
            };
            let ground = compose(&p.compose.ground);
            let outline = compose(&p.compose.outline);
            pass_items.push(
                ground.into_iter().chain(p.draws[0] as usize..p.draws[1] as usize).chain(outline).collect::<Vec<_>>(),
            );
        }
        let mut render = HashMap::new();
        for (p, pass) in b.passes.iter().enumerate() {
            let keys: Vec<Key> = match pass.target {
                Target::View => canvases.iter().map(|c| (Some(c.color), c.depth, c.samples)).collect(),
                Target::Texture(r) => match *shape(r) {
                    Shape::Texture { format, .. } | Shape::ViewSized { format, .. } => {
                        vec![(Some(texel(format).0), DEPTH, 1)]
                    }
                    Shape::Rows { .. } | Shape::Frame => unreachable!("validated: a pass target is a texture"),
                },
                Target::Shadow { .. } => vec![(None, DEPTH, 1)],
            };
            for key in keys {
                for &it in &pass_items[p] {
                    render.entry((it, key)).or_insert_with(|| {
                        let item = &items[it];
                        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                            label: None,
                            bind_group_layouts: &[Some(&bgl0), Some(&bgl1.render)],
                            immediate_size: 0,
                        });
                        let blend = match item.raster.blend {
                            Blend::Opaque => None,
                            Blend::Alpha => Some(wgpu::BlendState::ALPHA_BLENDING),
                            Blend::Add => {
                                let add = wgpu::BlendComponent {
                                    src_factor: wgpu::BlendFactor::One,
                                    dst_factor: wgpu::BlendFactor::One,
                                    operation: wgpu::BlendOperation::Add,
                                };
                                Some(wgpu::BlendState { color: add, alpha: add })
                            }
                        };
                        let targets: Vec<_> = key
                            .0
                            .map(|format| wgpu::ColorTargetState { format, blend, write_mask: wgpu::ColorWrites::ALL })
                            .into_iter()
                            .map(Some)
                            .collect();
                        let module = &modules[item.module as usize];
                        device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                            label: Some(name(item.entry)),
                            layout: Some(&layout),
                            vertex: wgpu::VertexState {
                                module,
                                entry_point: None,
                                compilation_options: Default::default(),
                                buffers: &[],
                            },
                            fragment: Some(wgpu::FragmentState {
                                module,
                                entry_point: Some(name(item.entry)),
                                compilation_options: Default::default(),
                                targets: &targets,
                            }),
                            primitive: wgpu::PrimitiveState {
                                cull_mode: match item.raster.cull {
                                    Cull::None => None,
                                    Cull::Back => Some(wgpu::Face::Back),
                                    Cull::Front => Some(wgpu::Face::Front),
                                },
                                ..Default::default()
                            },
                            depth_stencil: Some(wgpu::DepthStencilState {
                                format: key.1,
                                depth_write_enabled: Some(item.raster.depth_write),
                                depth_compare: Some(if item.screen {
                                    wgpu::CompareFunction::Always
                                } else {
                                    wgpu::CompareFunction::Less
                                }),
                                stencil: Default::default(),
                                bias: Default::default(),
                            }),
                            multisample: wgpu::MultisampleState { count: key.2, ..Default::default() },
                            multiview_mask: None,
                            cache,
                        })
                    });
                }
            }
        }
        let lenses: Vec<_> = b.cameras.iter().map(|c| (&modules[c.lens.0 as usize], name(c.lens.1))).collect();
        let cameras = view::Cameras::new(device, &bgl0, &bgl1.render, &lenses, cache);
        // view.rs's rule: a colour pass resolves the scene before its first item that writes no depth after one that did;
        // a pass that loads the view and opens with a Screen cover (a style's post pass) resolves before it, so it
        // reads the picture so far.
        let resolve_at = b
            .passes
            .iter()
            .zip(&pass_items)
            .map(|(pass, its)| match pass.target {
                Target::View => {
                    if matches!(pass.clear, Clear::Load) && its.first().is_some_and(|&it| items[it].screen) {
                        return Some(0);
                    }
                    let first = its.iter().position(|&it| items[it].raster.depth_write)?;
                    its.iter().skip(first).position(|&it| !items[it].raster.depth_write).map(|k| first + k)
                }
                Target::Texture(_) | Target::Shadow { .. } => None,
            })
            .collect();
        let mut loft = Loft {
            res: Vec::new(),
            packs,
            build,
            precision,
            canvases: canvases.to_vec(),
            frame,
            steps,
            detail,
            scene,
            resolve: view::Resolve::new(device),
            resolve_at,
            cameras,
            aim: Aim::Matrices(NO_CAMERA),
            bgl0,
            frame_size: frame_bytes,
            bgl1,
            bgl2,
            g0,
            g1,
            g2: Vec::new(),
            compute,
            render,
            items,
            pass_items,
            steppers: Vec::new(),
            size: [1, 1],
            time: 0.0,
            params,
            inputs,
            timestamps: None,
            frame_dispatches: 0,
            programs: b
                .row_steps
                .iter()
                .map(|s| ticks::Program::new(device, &b, s, &modules[s.module as usize], cache))
                .collect(),
            timelines: vec![],
            b,
        };
        loft.res = (0..loft.b.resources.len()).map(|r| loft.resource(device, r)).collect();
        let mut base = 0;
        for (kernel, s) in steppings {
            let res = loft.b.kernels[kernel].writes[0] as usize;
            let state = state_bytes(&loft.b.resources[res].shape);
            let Res::Rows { pack, .. } = loft.res[res] else { unreachable!("validated: a stepped kernel writes rows") };
            let scratch = || {
                let usage = wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::COPY_SRC;
                device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("loft step state"),
                    size: state,
                    usage,
                    mapped_at_creation: false,
                })
            };
            let x = match s.keep {
                Keep::Latest => [loft.packs[pack].clone(), scratch()],
                Keep::Every(_) => [scratch(), scratch()],
            };
            loft.steppers.push(Stepper { kernel, res, pack, s, x, base, done: 0, groups: Vec::new() });
            base += u64::from(s.count) + 1;
        }
        loft.bind(device);
        let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("loft load") });
        for (r, res) in loft.b.resources.iter().enumerate() {
            loft.init(queue, &mut enc, r, &res.init);
        }
        // Load kernels read group 0 too: time 0 with the default params and inputs, not an unwritten (zero) frame.
        queue.write_buffer(&loft.frame, 0, &loft.frame_bytes(0.0));
        let load: Vec<usize> =
            (0..loft.b.kernels.len()).filter(|&k| matches!(loft.b.kernels[k].when, When::Load)).collect();
        loft.dispatch_all(&mut enc, &load, None, &[loft.own()]);
        for s in 0..loft.steppers.len() {
            loft.precompute(queue, &mut enc, s);
        }
        for p in &loft.programs {
            let Res::Rows { pack, .. } = loft.res[p.spec.output as usize] else { unreachable!() };
            let mut timeline = p.start(device, queue, &loft.b, &loft.packs[pack]);
            if matches!(loft.b.inputs[p.spec.input as usize].kind, loft_ir::bundle::InputKind::Script { .. }) {
                timeline.bake(p, &mut enc, None);
            }
            loft.timelines.push(timeline);
        }
        // Every(k) states are final after load; Latest starts at state 0.
        queue.submit([enc.finish()]);
        loft
    }

    /// A resource at the current size (view-sized textures scale it).
    fn resource(&self, device: &wgpu::Device, r: usize) -> Res {
        let shape = &self.b.resources[r].shape;
        let (format, extent) = match *shape {
            Shape::Rows { .. } => {
                let (pack, offset) = self
                    .b
                    .packs
                    .iter()
                    .enumerate()
                    .find_map(|(p, pk)| pk.holds.iter().find(|h| h.0 as usize == r).map(|h| (p, h.1)))
                    .expect("validated: every rows resource is in one pack");
                return Res::Rows { pack, offset };
            }
            Shape::Frame => return Res::Frame,
            Shape::Texture { format, extent } => (format, extent),
            Shape::ViewSized { format, scale } => {
                let s = f32::from_bits(scale.0);
                (
                    format,
                    [
                        ((self.size[0] as f32 * s).ceil() as u32).max(1),
                        ((self.size[1] as f32 * s).ceil() as u32).max(1),
                        1,
                    ],
                )
            }
        };
        let (format, _) = texel(format);
        let size = wgpu::Extent3d { width: extent[0], height: extent[1], depth_or_array_layers: extent[2] };
        let deep = extent[2] > 1;
        let usage = wgpu::TextureUsages::TEXTURE_BINDING
            | wgpu::TextureUsages::COPY_DST
            | wgpu::TextureUsages::COPY_SRC
            | if deep { wgpu::TextureUsages::empty() } else { wgpu::TextureUsages::RENDER_ATTACHMENT }
            | if format == DEPTH { wgpu::TextureUsages::empty() } else { wgpu::TextureUsages::STORAGE_BINDING };
        let dimension = if deep { wgpu::TextureDimension::D3 } else { wgpu::TextureDimension::D2 };
        let desc = |format, usage| wgpu::TextureDescriptor {
            label: None,
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension,
            format,
            usage,
            view_formats: &[],
        };
        let tex = device.create_texture(&desc(format, usage));
        let tex = Box::new(Tex { view: tex.create_view(&Default::default()), tex });
        match self.b.passes.iter().any(|p| matches!(p.target, Target::Texture(t) if t as usize == r)) {
            true => {
                let depth = device.create_texture(&desc(DEPTH, wgpu::TextureUsages::RENDER_ATTACHMENT));
                Res::Target { tex, depth: depth.create_view(&Default::default()) }
            }
            false => Res::Tex(tex),
        }
    }

    /// Every group 2 bind group, and the steppers' (after a resize remade view-sized textures). Group 1 binds packs
    /// only, which a resize never remakes.
    fn bind(&mut self, device: &wgpu::Device) {
        let this = &*self;
        let packs = |p: usize| &this.packs[p];
        let own = |r: usize| binding(&this.res[r], &packs);
        let g2 = this.b.kernels.iter().zip(&this.bgl2).map(|(k, l)| group(device, l, &k.writes, &own)).collect();
        let n = this.packs.len();
        let steppers: Vec<Vec<_>> = this
            .steppers
            .iter()
            .map(|st| {
                let k = &this.b.kernels[st.kernel];
                // Parity p = i % 2 of step i: its pack is bound to x[1 - p] (the previous state) and it writes x[p].
                (0..2)
                    .map(|p| {
                        let reads = |q: usize| if q == st.pack { &st.x[1 - p] } else { &this.packs[q] };
                        let writes = |r: usize| if r == st.res { st.x[p].as_entire_binding() } else { own(r) };
                        (
                            pack_group(device, &this.bgl1.compute, n, &reads),
                            group(device, &this.bgl2[st.kernel], &k.writes, &writes),
                        )
                    })
                    .collect()
            })
            .collect();
        self.g2 = g2;
        for (st, g) in self.steppers.iter_mut().zip(steppers) {
            st.groups = g;
        }
    }

    fn init(&self, queue: &wgpu::Queue, enc: &mut wgpu::CommandEncoder, r: usize, init: &Init) {
        match (init, &self.res[r]) {
            (Init::Zero, Res::Rows { pack, offset }) => {
                let kept = self.b.kernels.iter().find(|k| k.writes.contains(&(r as Id))).and_then(Stepping::of);
                let size = state_bytes(&self.b.resources[r].shape) * u64::from(kept.map_or(1, |s| s.kept));
                enc.clear_buffer(&self.packs[*pack], offset * 16, Some(size));
            }
            (Init::Bytes(s), Res::Rows { pack, offset }) => {
                queue.write_buffer(&self.packs[*pack], offset * 16, &self.b.sections[*s as usize])
            }
            (Init::Bytes(s), Res::Tex(t) | Res::Target { tex: t, .. }) => {
                let tex = &t.tex;
                let e = tex.size();
                let row = e.width * tex.format().block_copy_size(None).expect("an uncompressed format");
                let layout =
                    wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(row), rows_per_image: Some(e.height) };
                queue.write_texture(tex.as_image_copy(), &self.b.sections[*s as usize], layout, e);
            }
            // A new texture is zero; `By` runs with the load kernels, or `rewind` runs it.
            (Init::Zero, Res::Tex(_) | Res::Target { .. } | Res::Frame)
            | (Init::Bytes(_), Res::Frame)
            | (Init::By(_), _) => {}
        }
    }

    fn dispatch(
        &self,
        pass: &mut wgpu::ComputePass,
        k: usize,
        g0: (&BindGroup, u32),
        groups: (&BindGroup, &BindGroup),
    ) {
        let [x, y, z] = match self.b.kernels[k].body {
            Body::Map { rows, .. } => [rows.div_ceil(64), 1, 1],
            Body::Scan { .. } => [1, 1, 1],
            Body::Raw { dispatch, .. } => dispatch,
        };
        pass.set_pipeline(&self.compute[k]);
        pass.set_bind_group(0, g0.0, &[g0.1]);
        pass.set_bind_group(1, groups.0, &[]);
        pass.set_bind_group(2, groups.1, &[]);
        pass.dispatch_workgroups(x, y, z);
    }

    /// The loft's own bind groups: frame at group 0 (offset 0), the compute packs at group 1, group 2 per kernel.
    fn own(&self) -> Groups<'_> {
        (&self.g0[0], &self.g1.compute, &self.g2)
    }

    /// One compute pass: `kernels` in order for each set of bind groups (the loft's own, or one per instance).
    fn dispatch_all(
        &self,
        enc: &mut wgpu::CommandEncoder,
        kernels: &[usize],
        timestamps: Option<&wgpu::QuerySet>,
        sets: &[Groups],
    ) {
        if kernels.is_empty() || sets.is_empty() {
            return;
        }
        let timestamp_writes = timestamps.map(|query_set| wgpu::ComputePassTimestampWrites {
            query_set,
            beginning_of_pass_write_index: Some(0),
            end_of_pass_write_index: Some(1),
        });
        let mut pass =
            enc.begin_compute_pass(&wgpu::ComputePassDescriptor { label: Some("loft kernels"), timestamp_writes });
        for &(g0, g1, g2) in sets {
            for &k in kernels {
                self.dispatch(&mut pass, k, (g0, 0), (g1, &g2[k]));
            }
        }
    }

    /// Step i's frame: the current params and inputs at time `from + i·dt`.
    fn step_frame(&self, queue: &wgpu::Queue, st: &Stepper, i: u32) {
        let at = (st.base + u64::from(i)) * self.stride();
        queue.write_buffer(&self.steps, at, &self.frame_bytes(st.s.from + i as f32 * st.s.dt));
    }

    fn stride(&self) -> u64 {
        ((frame::HEADER + 4 * (self.params.len() + self.inputs.len())) as u64).next_multiple_of(STEP_STRIDE)
    }

    fn frame_bytes(&self, time: f32) -> Vec<u8> {
        frame_bytes(&self.host_camera(), self.size, time, &self.params, &self.inputs)
    }

    /// The camera the CPU writes with the frame: the host's matrices, or a stand-in the GPU camera then overwrites.
    fn host_camera(&self) -> Camera {
        match self.aim {
            Aim::Matrices(c) => c,
            Aim::Look(_) | Aim::Named(_) => NO_CAMERA,
        }
    }

    /// Group 0 for the loft's frame and its step frames, over the current scene textures.
    fn group0s(&mut self, device: &wgpu::Device) {
        self.g0 = [&self.frame, &self.steps].map(|buffer| {
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("loft frame"),
                layout: &self.bgl0,
                entries: &group0(buffer, self.frame_size, &self.detail, &self.scene),
            })
        });
    }

    /// Steps `s` from state `from` to `to` in one compute pass (state i ends in `x[i % 2]`).
    fn run_steps(&self, queue: &wgpu::Queue, enc: &mut wgpu::CommandEncoder, s: usize, from: u32, to: u32) {
        let st = &self.steppers[s];
        let mut pass =
            enc.begin_compute_pass(&wgpu::ComputePassDescriptor { label: Some("loft steps"), timestamp_writes: None });
        for i in from + 1..=to {
            self.step_frame(queue, st, i);
            let (g1, g2) = &st.groups[(i % 2) as usize];
            let offset = ((st.base + u64::from(i)) * self.stride()) as u32;
            self.dispatch(&mut pass, st.kernel, (&self.g0[1], offset), (g1, g2));
        }
    }

    /// `Every(k)`: every step at load, keeping state `first + j·k` in slot j of the resource.
    fn precompute(&self, queue: &wgpu::Queue, enc: &mut wgpu::CommandEncoder, s: usize) {
        let st = &self.steppers[s];
        let Keep::Every(every) = st.s.keep else { return };
        let (r, state) = (&self.packs[st.pack], state_bytes(&self.b.resources[st.res].shape));
        // The init landed in the resource; state 0 starts from it. One pass per step: kept states are copied between.
        enc.copy_buffer_to_buffer(r, 0, &st.x[0], 0, Some(state));
        for i in 0..=st.s.count {
            if i > 0 {
                self.run_steps(queue, enc, s, i - 1, i);
            }
            if i >= st.s.first && (i - st.s.first).is_multiple_of(every) {
                enc.copy_buffer_to_buffer(
                    &st.x[(i % 2) as usize],
                    0,
                    r,
                    u64::from((i - st.s.first) / every) * state,
                    Some(state),
                );
            }
        }
    }

    // ---- the view ----------------------------------------------------------------------------------------------

    /// What loading built: shader modules, render pipelines and compute pipelines (the camera builder's included). Load
    /// time grows with these (CORE.md: score them beside frame time).
    pub fn built(&self) -> [usize; 3] {
        [self.b.modules.len(), self.render.len(), self.compute.len() + self.b.cameras.len() + 2 * self.programs.len()]
    }

    /// Frame-rate kernels dispatched in the most recent encode, excluding lens and step passes.
    pub fn frame_dispatches(&self) -> usize {
        self.frame_dispatches
    }

    pub fn precision(&self) -> Precision {
        self.precision
    }
    pub fn bundle(&self) -> &Bundle {
        &self.b
    }
    pub fn set_time(&mut self, seconds: f32) {
        self.time = seconds;
    }
    /// The host's camera matrices for every pass (`loft_ir::frame`); the viewport comes from the view's size.
    pub fn set_camera(&mut self, camera: Camera) {
        self.aim = Aim::Matrices(camera);
    }
    /// A host look camera (perspective, `fovy` radians), built on the GPU as a bundle's look cameras are.
    pub fn set_look(&mut self, eye: [f32; 3], target: [f32; 3], up: [f32; 3], fovy: f32) {
        self.aim = Aim::Look(frame::look(eye, target, up, fovy));
    }
    /// The bundle camera named `name`, evaluated on the GPU every frame.
    pub fn use_camera(&mut self, name: &str) -> Result<(), Error> {
        let (k, _) = self.named(&self.b.cameras, |c| c.name, name, "camera")?;
        self.aim = Aim::Named(k);
        Ok(())
    }
    /// The bundle's camera names, in order.
    pub fn cameras(&self) -> impl Iterator<Item = &str> {
        self.b.cameras.iter().map(|c| &*self.b.names[c.name as usize])
    }
    pub fn aim(&self) -> Aim {
        self.aim
    }
    fn named<'a, T>(
        &self,
        list: &'a [T],
        name_of: impl Fn(&T) -> Id,
        name: &str,
        kind: &str,
    ) -> Result<(usize, &'a T), Error> {
        list.iter()
            .enumerate()
            .find(|(_, d)| &*self.b.names[name_of(d) as usize] == name)
            .ok_or_else(|| Error::Usage(format!("no {kind} named \"{name}\" in this bundle").into()))
    }
    /// A param by name; `min` and `max` are for editors and do not clamp.
    pub fn set_param(&mut self, name: &str, value: f32) -> Result<(), Error> {
        let k = self.param_slot(name)?;
        self.params[k] = value;
        Ok(())
    }
    /// An input by name, as many floats as its type (a host camera writes the view camera's inputs).
    pub fn set_input(&mut self, name: &str, value: &[f32]) -> Result<(), Error> {
        let at = self.input_slot(name, value.len())?;
        for (s, p) in
            self.timelines.iter_mut().zip(&self.programs).filter(|(_, p)| p.spec.input as usize == at.start / 4)
        {
            let tick = ((self.time * p.spec.hz as f32).floor() as u32).max(1);
            for r in 0..p.count {
                s.write(p, r, tick, value, None)?;
            }
        }
        self.inputs[at].copy_from_slice(value);
        Ok(())
    }
    /// Supply an input at its simulation tick, independently of the presentation clock.
    pub fn write_tick(
        &mut self,
        name: &str,
        row: u32,
        tick: u32,
        value: &[f32],
        sequence: Option<u64>,
    ) -> Result<(), Error> {
        let slot = self.input_slot(name, value.len())?.start / 4;
        let mut found = false;
        for (s, p) in self.timelines.iter_mut().zip(&self.programs).filter(|(_, p)| p.spec.input as usize == slot) {
            s.write(p, row, tick, value, sequence)?;
            found = true;
        }
        if !found {
            return Err(Error::Usage(format!("input {name} is frame-rate: use set_input").into()));
        }
        Ok(())
    }
    /// Copy a current or retained state into a caller-owned staging buffer without mapping in the frame loop.
    pub fn copy_state(
        &self,
        enc: &mut wgpu::CommandEncoder,
        index: usize,
        tick: Option<u32>,
        dst: &Buffer,
        off: u64,
    ) -> Result<(), Error> {
        let p = self.programs.get(index).ok_or_else(|| Error::Usage("choose a declared stepped table".into()))?;
        self.timelines[index].copy(p, enc, tick, dst, off)
    }
    /// Run just the stepped tables, e.g. for headless verification. Timestamp the single-table span at 0/1.
    pub fn advance_rows(&mut self, enc: &mut wgpu::CommandEncoder) -> usize {
        self.programs
            .iter()
            .zip(&mut self.timelines)
            .enumerate()
            .map(|(i, (p, s))| s.encode(p, enc, self.time, self.timestamps.as_ref().filter(|_| i == 0)))
            .sum()
    }
    /// Rebake an unedited script into its retained states, optionally measuring that same entry at 0/1.
    pub fn bake_rows(&mut self, enc: &mut wgpu::CommandEncoder, index: usize) -> Result<(), Error> {
        let p = self.programs.get(index).ok_or_else(|| Error::Usage("choose a declared script".into()))?;
        let timeline = &mut self.timelines[index];
        if !timeline.is_baked() {
            return Err(Error::Usage("live input is present: reload the script to bake it".into()));
        }
        timeline.bake(p, enc, self.timestamps.as_ref());
        Ok(())
    }
    /// GPU-to-GPU copy of a row resource, including its pack offset.
    pub fn copy_rows(&self, enc: &mut wgpu::CommandEncoder, r: Id, dst: &Buffer, off: u64) -> Result<(), Error> {
        let Some(Res::Rows { pack, offset }) = self.res.get(r as usize) else {
            return Err(Error::Usage("choose a rows resource to read".into()));
        };
        enc.copy_buffer_to_buffer(
            &self.packs[*pack],
            16 * offset,
            dst,
            off,
            Some(state_bytes(&self.b.resources[r as usize].shape)),
        );
        Ok(())
    }
    fn param_slot(&self, name: &str) -> Result<usize, Error> {
        self.named(&self.b.params, |p| p.name, name, "param").map(|(k, _)| k)
    }
    /// Where an input of `n` floats lives in the inputs array.
    fn input_slot(&self, name: &str, n: usize) -> Result<std::ops::Range<usize>, Error> {
        let (k, d) = self.named(&self.b.inputs, |i| i.name, name, "input")?;
        match n == d.def.len() {
            true => Ok(k * 4..k * 4 + n),
            false => Err(Error::Usage(format!("input \"{name}\" takes {} floats, given {n}", d.def.len()).into())),
        }
    }
    /// Measuring: a query set of `2 + 2·passes` timestamps: the kernel pass at 0 and 1, render pass k at `2 + 2k`,
    /// `3 + 2k`. Resolve them only after the frame completes (Metal: a resolve in the next submit read stale ends).
    pub fn set_timestamps(&mut self, set: Option<wgpu::QuerySet>) {
        self.timestamps = set;
    }
    /// What a frame of this view is, as drawn now (plan §1.9): with `Identity::build` the bundle's hash.
    pub fn frame_key(&self, camera: Id, size: [u32; 2]) -> FrameKey {
        let bits = |f: &[f32]| f.iter().map(|x| x.to_bits()).collect();
        let inputs: Vec<f32> = self
            .b
            .inputs
            .iter()
            .enumerate()
            .flat_map(|(k, d)| self.inputs[k * 4..k * 4 + d.def.len()].to_vec())
            .collect();
        FrameKey {
            steps: self.programs.iter().zip(&self.timelines).map(|(p, s)| s.key(p, self.time)).collect(),
            camera,
            time: self.time.to_bits(),
            size,
            params: bits(&self.params[..self.b.params.len()]),
            inputs: bits(&inputs),
            precision: self.precision,
        }
    }
    pub fn build(&self) -> [u8; 32] {
        self.build
    }

    // ---- a frame -----------------------------------------------------------------------------------------------

    /// Records a frame into the caller's encoder: frame kernels, live steps to the time, then the passes in order. A
    /// `View` pass draws into `view`'s attachments, and with `Owner::Caller` it loads them and clears nothing.
    pub fn encode_into(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        enc: &mut wgpu::CommandEncoder,
        view: &View,
    ) -> Result<(), Error> {
        if !self.canvases.contains(&view.canvas) {
            return Err(Error::Usage(format!("{:?} was not loaded: pass it to Loft::load", view.canvas).into()));
        }
        if view.size[0] == 0 || view.size[1] == 0 {
            return Err(Error::Usage(format!("view size {:?}: both sides must be at least 1", view.size).into()));
        }
        if view.size != self.size && self.b.resources.iter().any(|r| matches!(r.shape, Shape::ViewSized { .. })) {
            self.size = view.size;
            for r in 0..self.res.len() {
                if let Shape::ViewSized { .. } = self.b.resources[r].shape {
                    self.res[r] = self.resource(device, r);
                }
            }
            self.bind(device);
        }
        self.size = view.size;
        if self.resolve_at.iter().any(Option::is_some) && !self.scene.fits(view.size, view.canvas.color) {
            self.scene = view::Scene::new(device, queue, view.size, view.canvas.color);
            self.group0s(device);
        }
        queue.write_buffer(&self.frame, 0, &self.frame_bytes(self.time));

        for s in 0..self.steppers.len() {
            let st = &self.steppers[s];
            let Keep::Latest = st.s.keep else { continue };
            let to = st.s.at(self.time);
            if to < st.done {
                // Rewind: the resource back to its init, state 0.
                let init = self.b.resources[st.res].init.clone();
                self.init(queue, enc, st.res, &init);
                if let Init::By(k) = init {
                    self.dispatch_all(enc, &[k as usize], None, &[self.own()]);
                }
                self.steppers[s].done = 0;
            }
            let st = &self.steppers[s];
            if to > st.done {
                self.run_steps(queue, enc, s, st.done, to);
                // Readers bind x[0], the resource: an odd last state is copied home.
                if to % 2 == 1 {
                    enc.copy_buffer_to_buffer(&st.x[1], 0, &st.x[0], 0, None);
                }
                self.steppers[s].done = to;
            }
        }
        let kernels = self.frame_kernels();
        self.frame_dispatches = kernels.len();
        for (i, (p, s)) in self.programs.iter().zip(&mut self.timelines).enumerate() {
            self.frame_dispatches +=
                s.encode(p, enc, self.time, self.timestamps.as_ref().filter(|_| kernels.is_empty() && i == 0));
        }
        // A camera can read presented rows. Fill its matrices before frame kernels that use the view.
        self.cameras.encode(queue, enc, &self.aim, (&self.g0[0], &self.g1.render), &[&self.frame]);
        self.dispatch_all(enc, &kernels, self.timestamps.as_ref(), &[self.own()]);
        for (p, pass) in self.b.passes.iter().enumerate() {
            let clear = |owned: bool| match (owned, &pass.clear) {
                (true, Clear::Color(c)) => {
                    let c = c.map(|x| f64::from(f32::from_bits(x.0)));
                    (wgpu::LoadOp::Clear(wgpu::Color { r: c[0], g: c[1], b: c[2], a: c[3] }), wgpu::LoadOp::Clear(1.0))
                }
                (true, Clear::Load) | (false, _) => (wgpu::LoadOp::Load, wgpu::LoadOp::Load),
            };
            let (color, depth, ops, key, into) = match pass.target {
                Target::View => (
                    Some(view.color),
                    view.depth,
                    clear(view.owner == Owner::Loft),
                    (Some(view.canvas.color), view.canvas.depth, view.canvas.samples),
                    view.resolve,
                ),
                Target::Texture(r) => match &self.res[r as usize] {
                    Res::Target { tex, depth } => {
                        (Some(&tex.view), depth, clear(true), (Some(tex.tex.format()), DEPTH, 1), None)
                    }
                    Res::Tex(_) | Res::Rows { .. } | Res::Frame => {
                        unreachable!("a pass's colour target is a Res::Target")
                    }
                },
                Target::Shadow { map, .. } => match &self.res[map as usize] {
                    Res::Tex(t) => (None, &t.view, clear(true), (None, DEPTH, 1), None),
                    Res::Target { .. } | Res::Rows { .. } | Res::Frame => {
                        unreachable!("validated: a shadow map is a depth texture, never a colour target")
                    }
                },
            };
            let store = wgpu::StoreOp::Store;
            let items = &self.pass_items[p];
            // One render pass, or two around the scene resolve (view.rs): the second loads what the first stored.
            let split = self.resolve_at[p].unwrap_or(items.len());
            for (seg, run) in [&items[..split], &items[split..]].into_iter().enumerate() {
                if seg == 1 {
                    if run.is_empty() {
                        break;
                    }
                    // A multisampled view resolves its colour each segment: the scene colour is copied from there.
                    let picture = into.or(color).expect("a View pass has colour");
                    self.resolve.encode(device, enc, &self.scene, picture, depth)?;
                }
                let load = if seg == 0 { ops } else { (wgpu::LoadOp::Load, wgpu::LoadOp::Load) };
                let colors: Vec<_> = color
                    .map(|view| {
                        Some(wgpu::RenderPassColorAttachment {
                            view,
                            depth_slice: None,
                            resolve_target: into,
                            ops: wgpu::Operations { load: load.0, store },
                        })
                    })
                    .into_iter()
                    .collect();
                let last = split == items.len() || seg == 1;
                let timestamp_writes = self.timestamps.as_ref().map(|query_set| wgpu::RenderPassTimestampWrites {
                    query_set,
                    beginning_of_pass_write_index: (seg == 0).then_some(2 + 2 * p as u32),
                    end_of_pass_write_index: last.then_some(3 + 2 * p as u32),
                });
                let mut rp = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("loft pass"),
                    color_attachments: &colors,
                    depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                        view: depth,
                        depth_ops: Some(wgpu::Operations { load: load.1, store }),
                        stencil_ops: None,
                    }),
                    timestamp_writes,
                    occlusion_query_set: None,
                    multiview_mask: None,
                });
                rp.set_bind_group(0, &self.g0[0], &[0]);
                rp.set_bind_group(1, &self.g1.render, &[]);
                for &it in run {
                    let item = &self.items[it];
                    let drawn = match (&pass.target, view.layers, item.layer) {
                        (Target::View, Layers::Subject, Layer::Surroundings) => false,
                        (Target::View, Layers::Subject, Layer::Subject)
                        | (Target::View, Layers::All, _)
                        | (Target::Texture(_) | Target::Shadow { .. }, _, _) => true,
                    };
                    if drawn {
                        rp.set_pipeline(&self.render[&(it, key)]);
                        rp.draw(0..item.count[0], 0..item.count[1]);
                    }
                }
            }
        }
        Ok(())
    }
}

impl Loft {
    // ---- many instances (Keel step 8) ------------------------------------------------------------------------------

    /// A new copy at this loft's current time, params and inputs. Refused when a copy would share per-frame state it
    /// cannot share: a live (`Keep::Latest`) stepped kernel, an offscreen pass, or a view-sized `Frame` write.
    pub fn instance(&self, device: &wgpu::Device, queue: &wgpu::Queue) -> Result<Instance, Error> {
        let live = self
            .steppers
            .iter()
            .find(|st| st.s.keep == Keep::Latest)
            .map(|st| format!("kernel {} steps live", st.kernel));
        let offscreen = self
            .b
            .passes
            .iter()
            .position(|p| !matches!(p.target, Target::View))
            .map(|p| format!("pass {p} draws offscreen"));
        let frame: Vec<usize> = self.frame_kernels();
        let written = |r: usize| {
            self.programs.iter().any(|p| p.spec.output as usize == r)
                || frame.iter().any(|&k| self.b.kernels[k].writes.contains(&(r as Id)))
        };
        let sized = (0..self.res.len())
            .find(|&r| written(r) && matches!(self.b.resources[r].shape, Shape::ViewSized { .. }))
            .map(|r| format!("frame kernels write view-sized resource {r}"));
        if let Some(why) = live.or(offscreen).or(sized) {
            return Err(Error::Usage(
                format!("this bundle cannot be instanced: {why}, which every copy would share. Fix: draw it with encode_into").into(),
            ));
        }
        let frame_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("loft instance frame"),
            size: self.frame_size.max(STEP_STRIDE),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let g0 = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("loft instance frame"),
            layout: &self.bgl0,
            entries: &group0(&frame_buf, self.frame_size, &self.detail, &self.scene),
        });
        // A copy owns every pack a frame kernel writes (whole: its other tables are rewritten each frame too, or are
        // load state copied once here) and every texture a frame kernel writes.
        let mut packs: Vec<usize> = (0..self.res.len())
            .filter(|&r| written(r))
            .filter_map(|r| match self.res[r] {
                Res::Rows { pack, .. } => Some(pack),
                Res::Tex(_) | Res::Target { .. } | Res::Frame => None,
            })
            .collect();
        packs.sort_unstable();
        packs.dedup();
        let own_packs: Vec<(usize, Buffer)> = packs
            .iter()
            .map(|&p| {
                let usage = wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::COPY_SRC;
                let size = self.packs[p].size();
                (
                    p,
                    device.create_buffer(&wgpu::BufferDescriptor {
                        label: Some("loft instance pack"),
                        size,
                        usage,
                        mapped_at_creation: false,
                    }),
                )
            })
            .collect();
        let own_tex: Vec<(usize, Res)> = (0..self.res.len())
            .filter(|&r| written(r) && !matches!(self.res[r], Res::Rows { .. }))
            .map(|r| (r, self.resource(device, r)))
            .collect();
        // A copy starts as the loft's resource is now (its init, or the last frame), not as zeros.
        let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("loft instance") });
        for (p, copy) in &own_packs {
            enc.copy_buffer_to_buffer(&self.packs[*p], 0, copy, 0, None);
        }
        for (r, copy) in &own_tex {
            match (&self.res[*r], copy) {
                (Res::Tex(a) | Res::Target { tex: a, .. }, Res::Tex(b) | Res::Target { tex: b, .. }) => {
                    enc.copy_texture_to_texture(a.tex.as_image_copy(), b.tex.as_image_copy(), a.tex.size())
                }
                (Res::Rows { .. } | Res::Tex(_) | Res::Target { .. } | Res::Frame, _) => {
                    unreachable!("a copy has its resource's shape, and kernels never write the frame")
                }
            }
        }
        queue.submit([enc.finish()]);
        let pack = |p: usize| own_packs.iter().find(|(o, _)| *o == p).map_or(&self.packs[p], |(_, b)| b);
        let at = |r: usize| binding(own_tex.iter().find(|(o, _)| *o == r).map_or(&self.res[r], |(_, res)| res), &pack);
        let n = self.packs.len();
        let g1 = Packs {
            render: pack_group(device, &self.bgl1.render, n, &pack),
            compute: pack_group(device, &self.bgl1.compute, n, &pack),
        };
        let g2 = self.b.kernels.iter().zip(&self.bgl2).map(|(k, l)| group(device, l, &k.writes, &at)).collect();
        let mut start = device.create_command_encoder(&Default::default());
        let timelines = self
            .programs
            .iter()
            .map(|p| {
                let Res::Rows { pack: index, .. } = self.res[p.spec.output as usize] else { unreachable!() };
                let mut s = p.start(device, queue, &self.b, pack(index));
                if matches!(self.b.inputs[p.spec.input as usize].kind, loft_ir::bundle::InputKind::Script { .. }) {
                    s.bake(p, &mut start, None);
                }
                s
            })
            .collect();
        queue.submit([start.finish()]);
        Ok(Instance {
            timelines: std::cell::RefCell::new(timelines),
            time: self.time,
            params: self.params.clone(),
            inputs: self.inputs.clone(),
            frame: frame_buf,
            g0,
            g1,
            g2,
            _own: (own_packs.into_iter().map(|(_, b)| b).collect(), own_tex.into_iter().map(|(_, r)| r).collect()),
        })
    }

    /// Writes each instance's frame and runs the `Frame` kernels for all of them in one compute pass (timestamps 0 and 1
    /// when set). Call once per frame before `draw_subject`.
    pub fn prepare(&self, queue: &wgpu::Queue, enc: &mut wgpu::CommandEncoder, instances: &[Instance]) {
        let camera = self.host_camera();
        for i in instances {
            for (s, p) in i.timelines.borrow_mut().iter_mut().zip(&self.programs) {
                s.encode(p, enc, i.time, None);
            }
            queue.write_buffer(&i.frame, 0, &frame_bytes(&camera, self.size, i.time, &i.params, &i.inputs));
        }
        // A GPU camera is built once, on the loft's own frame, and copied into every copy's.
        queue.write_buffer(&self.frame, 0, &self.frame_bytes(self.time));
        let frames: Vec<&Buffer> = instances.iter().map(|i| &i.frame).collect();
        self.cameras.encode(queue, enc, &self.aim, (&self.g0[0], &self.g1.render), &frames);
        let sets: Vec<Groups> = instances.iter().map(|i| (&i.g0, &i.g1.compute, &i.g2[..])).collect();
        self.dispatch_all(enc, &self.frame_kernels(), self.timestamps.as_ref(), &sets);
    }

    /// Records every instance's subject draws of the view passes into the caller's render pass, whose attachments are
    /// `canvas` (loaded at `Loft::load`). Each draw's pipeline is set once for all instances.
    pub fn draw_subject(&self, rp: &mut wgpu::RenderPass, canvas: Canvas, instances: &[Instance]) -> Result<(), Error> {
        if !self.canvases.contains(&canvas) {
            return Err(Error::Usage(format!("{canvas:?} was not loaded: pass it to Loft::load").into()));
        }
        let key = (Some(canvas.color), canvas.depth, canvas.samples);
        // `instance()` refused offscreen passes, so every pass draws to the view.
        for &it in self.pass_items.iter().flatten() {
            let item = &self.items[it];
            match item.layer {
                Layer::Surroundings => continue,
                Layer::Subject => {}
            }
            rp.set_pipeline(&self.render[&(it, key)]);
            for i in instances {
                rp.set_bind_group(0, &i.g0, &[0]);
                rp.set_bind_group(1, &i.g1.render, &[]);
                rp.draw(0..item.count[0], 0..item.count[1]);
            }
        }
        Ok(())
    }

    fn frame_kernels(&self) -> Vec<usize> {
        (0..self.b.kernels.len()).filter(|&k| matches!(self.b.kernels[k].when, When::Frame)).collect()
    }
}

/// A kernel's (module, entry).
pub(crate) fn entry_of(k: &Kernel) -> (Id, Id) {
    match k.body {
        Body::Map { module, entry, .. } | Body::Scan { module, entry, .. } | Body::Raw { module, entry, .. } => {
            (module, entry)
        }
    }
}
