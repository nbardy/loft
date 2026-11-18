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
//! loft.encode_into(&device, &queue, &mut encoder, &View { canvas, color: &color, depth: &depth, size: [w, h],
//!                                                         owner: Owner::Caller, layers: Layers::Subject })?;
//! queue.submit([encoder.finish()]);
//! ```
//!
//! The binding contract the emitter writes against (loft-core emit, lane 2.2; draws, lane 3.1):
//! - `@group(0) @binding(0)`: `struct Frame { time: f32, params: array<vec4<f32>, P>, inputs: array<vec4<f32>, I> }`.
//!   Param k is component k of `params`, input j is `inputs[j]`, in bundle order (P, I at least 1).
//! - `@group(1) @binding(k)`: the module's `reads[k]`. Rows are `array<vec4<f32>>` in read-only storage; textures are
//!   `texture_2d<f32>` (`texture_3d` when deep, `texture_depth_2d` for depth), read with `textureLoad`.
//! - `@group(2) @binding(k)`: a kernel's `writes[k]`: rows in read-write storage, textures as write-only storage.
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
mod validate;

#[cfg(not(target_arch = "wasm32"))]
pub use cache::PipelineCache;
pub use validate::validate;

use loft_ir::Error;
use loft_ir::bundle::{
    Blend, Body, Bundle, Clear, Cover, Id, Init, Keep, Kernel, Layer, Module, Origin, Proxy, Raster, RasterPrim, Shape,
    Target, TexFormat, When,
};
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
    pub color: &'a TextureView,
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
    g1: Vec<BindGroup>,
    g2: Vec<BindGroup>,
    _own: Vec<Res>,
}
impl Instance {
    pub fn set_param(&mut self, loft: &Loft, name: &str, value: f32) -> Result<(), Error> {
        self.params[loft.param_slot(name)?] = value;
        Ok(())
    }
    pub fn set_input(&mut self, loft: &Loft, name: &str, value: &[f32]) -> Result<(), Error> {
        self.inputs[loft.input_slot(name, value.len())?].copy_from_slice(value);
        Ok(())
    }
}

/// A resource on the device. A texture a pass draws into (`Target::Texture`) also owns that pass's depth; no other
/// texture gets one, since a 3D table cannot have a depth texture and a large one would double its memory.
enum Res {
    Rows(Buffer),
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

/// A stepped kernel's state buffers: step i reads `x[(i - 1) % 2]` and writes `x[i % 2]`. `x[0]` is the resource itself
/// under `Latest`, a scratch under `Every`. `groups[p]` are (group 1, group 2) for steps of parity p.
struct Stepper {
    kernel: usize,
    res: usize,
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
/// The bind groups one copy of the subject dispatches with: (group 0, group 1 per module, group 2 per kernel).
type Groups<'a> = (&'a BindGroup, &'a [BindGroup], &'a [BindGroup]);

pub struct Loft {
    b: Bundle,
    build: [u8; 32],
    precision: Precision,
    canvases: Vec<Canvas>,
    res: Vec<Res>,
    frame: Buffer,
    steps: Buffer,
    bgl0: BindGroupLayout,
    frame_size: u64,
    bgl1: Vec<BindGroupLayout>,
    bgl2: Vec<BindGroupLayout>,
    g0: [BindGroup; 2],
    g1: Vec<BindGroup>,
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

fn binding(r: &Res) -> wgpu::BindingResource<'_> {
    match r {
        Res::Rows(buf) => buf.as_entire_binding(),
        Res::Tex(t) | Res::Target { tex: t, .. } => wgpu::BindingResource::TextureView(&t.view),
        Res::Frame => unreachable!("validated: group 1 and 2 never bind the frame"),
    }
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

fn frame_bytes(time: f32, params: &[f32], inputs: &[f32]) -> Vec<u8> {
    floats(&[&[time, 0.0, 0.0, 0.0][..], params, inputs].concat())
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

fn layout_entry(binding: u32, shape: &Shape, write: bool) -> wgpu::BindGroupLayoutEntry {
    let (format, view_dimension) = match *shape {
        Shape::Texture { format, extent } if extent[2] > 1 => (format, wgpu::TextureViewDimension::D3),
        Shape::Texture { format, .. } | Shape::ViewSized { format, .. } => (format, wgpu::TextureViewDimension::D2),
        // validated: group 1 and 2 never hold the frame
        Shape::Rows { .. } | Shape::Frame => {
            let ty = wgpu::BufferBindingType::Storage { read_only: !write };
            let ty = wgpu::BindingType::Buffer { ty, has_dynamic_offset: false, min_binding_size: None };
            let visibility =
                if write { ShaderStages::COMPUTE } else { ShaderStages::VERTEX_FRAGMENT | ShaderStages::COMPUTE };
            return wgpu::BindGroupLayoutEntry { binding, visibility, ty, count: None };
        }
    };
    let sample_type = match format {
        TexFormat::Depth32Float => wgpu::TextureSampleType::Depth,
        TexFormat::Rgba8Unorm | TexFormat::Rgba16Float | TexFormat::R32Float => {
            wgpu::TextureSampleType::Float { filterable: false }
        }
    };
    let ty = match write {
        true => wgpu::BindingType::StorageTexture {
            access: wgpu::StorageTextureAccess::WriteOnly,
            format: texel(format).0,
            view_dimension,
        },
        false => wgpu::BindingType::Texture { sample_type, view_dimension, multisampled: false },
    };
    let visibility = if write { ShaderStages::COMPUTE } else { ShaderStages::VERTEX_FRAGMENT | ShaderStages::COMPUTE };
    wgpu::BindGroupLayoutEntry { binding, visibility, ty, count: None }
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
        let frame_bytes = 16 + 16 * (p + i) as u64;
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
            size: (entries * stride).max(STEP_STRIDE),
            usage,
            mapped_at_creation: false,
        });
        let bgl0 = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("loft frame"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: ShaderStages::VERTEX_FRAGMENT | ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: true,
                    min_binding_size: None,
                },
                count: None,
            }],
        });
        let g0 = [&frame, &steps].map(|buffer| {
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("loft frame"),
                layout: &bgl0,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                        buffer,
                        offset: 0,
                        size: wgpu::BufferSize::new(frame_bytes),
                    }),
                }],
            })
        });
        let shape = |r: Id| &b.resources[r as usize].shape;
        let bgl = |list: &[Id], write: bool| {
            let entries: Vec<_> =
                list.iter().enumerate().map(|(k, &r)| layout_entry(k as u32, shape(r), write)).collect();
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor { label: None, entries: &entries })
        };
        let bgl1: Vec<_> = b.modules.iter().map(|m| bgl(&m.reads, false)).collect();
        let bgl2: Vec<_> = b.kernels.iter().map(|k| bgl(&k.writes, true)).collect();
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
                    bind_group_layouts: &[Some(&bgl0), Some(&bgl1[module as usize]), Some(&bgl2[k])],
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
        let over = Raster { cull: false, depth_write: false, blend: Blend::Alpha };
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
                            bind_group_layouts: &[Some(&bgl0), Some(&bgl1[item.module as usize])],
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
                                cull_mode: item.raster.cull.then_some(wgpu::Face::Back),
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
        let mut loft = Loft {
            res: Vec::new(),
            build,
            precision,
            canvases: canvases.to_vec(),
            frame,
            steps,
            bgl0,
            frame_size: frame_bytes,
            bgl1,
            bgl2,
            g0,
            g1: Vec::new(),
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
            b,
        };
        loft.res = (0..loft.b.resources.len()).map(|r| loft.resource(device, r)).collect();
        let mut base = 0;
        for (kernel, s) in steppings {
            let res = loft.b.kernels[kernel].writes[0] as usize;
            let state = state_bytes(&loft.b.resources[res].shape);
            let scratch = || {
                let usage = wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::COPY_SRC;
                device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("loft step state"),
                    size: state,
                    usage,
                    mapped_at_creation: false,
                })
            };
            let x = match (s.keep, &loft.res[res]) {
                (Keep::Latest, Res::Rows(r)) => [r.clone(), scratch()],
                (Keep::Every(_), Res::Rows(_)) => [scratch(), scratch()],
                (_, Res::Tex(_) | Res::Target { .. } | Res::Frame) => {
                    unreachable!("validated: a stepped kernel writes rows")
                }
            };
            loft.steppers.push(Stepper { kernel, res, s, x, base, done: 0, groups: Vec::new() });
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
        // Every(k) states are final after load; Latest starts at state 0.
        queue.submit([enc.finish()]);
        loft
    }

    /// A resource at the current size (view-sized textures scale it).
    fn resource(&self, device: &wgpu::Device, r: usize) -> Res {
        let shape = &self.b.resources[r].shape;
        let (format, extent) = match *shape {
            Shape::Rows { .. } => {
                let kept = self
                    .b
                    .kernels
                    .iter()
                    .find(|k| k.writes.contains(&(r as Id)))
                    .and_then(Stepping::of)
                    .map_or(1, |s| s.kept);
                let usage = wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::COPY_SRC;
                let size = state_bytes(shape) * u64::from(kept);
                return Res::Rows(device.create_buffer(&wgpu::BufferDescriptor {
                    label: None,
                    size,
                    usage,
                    mapped_at_creation: false,
                }));
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

    /// Every group 1 and group 2 bind group, and the steppers' (after a resize remade view-sized textures).
    fn bind(&mut self, device: &wgpu::Device) {
        let this = &*self;
        let own = |r: usize| binding(&this.res[r]);
        let g1 = this.b.modules.iter().zip(&this.bgl1).map(|(m, l)| group(device, l, &m.reads, &own)).collect();
        let g2 = this.b.kernels.iter().zip(&this.bgl2).map(|(k, l)| group(device, l, &k.writes, &own)).collect();
        let steppers: Vec<Vec<_>> = this
            .steppers
            .iter()
            .map(|st| {
                let k = &this.b.kernels[st.kernel];
                let m = entry_of(k).0 as usize;
                // Parity p = i % 2 of step i: it reads x[1 - p] and writes x[p] in place of its resource.
                (0..2)
                    .map(|p| {
                        let reads = |r: usize| if r == st.res { st.x[1 - p].as_entire_binding() } else { own(r) };
                        let writes = |r: usize| if r == st.res { st.x[p].as_entire_binding() } else { own(r) };
                        (
                            group(device, &this.bgl1[m], &this.b.modules[m].reads, &reads),
                            group(device, &this.bgl2[st.kernel], &k.writes, &writes),
                        )
                    })
                    .collect()
            })
            .collect();
        (self.g1, self.g2) = (g1, g2);
        for (st, g) in self.steppers.iter_mut().zip(steppers) {
            st.groups = g;
        }
    }

    fn init(&self, queue: &wgpu::Queue, enc: &mut wgpu::CommandEncoder, r: usize, init: &Init) {
        match (init, &self.res[r]) {
            (Init::Zero, Res::Rows(buf)) => enc.clear_buffer(buf, 0, None),
            (Init::Bytes(s), Res::Rows(buf)) => queue.write_buffer(buf, 0, &self.b.sections[*s as usize]),
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

    /// The loft's own bind groups: frame at group 0 (offset 0), group 1 per module, group 2 per kernel.
    fn own(&self) -> Groups<'_> {
        (&self.g0[0], &self.g1, &self.g2)
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
                self.dispatch(&mut pass, k, (g0, 0), (&g1[entry_of(&self.b.kernels[k]).0 as usize], &g2[k]));
            }
        }
    }

    /// Step i's frame: the current params and inputs at time `from + i·dt`.
    fn step_frame(&self, queue: &wgpu::Queue, st: &Stepper, i: u32) {
        let at = (st.base + u64::from(i)) * self.stride();
        queue.write_buffer(&self.steps, at, &self.frame_bytes(st.s.from + i as f32 * st.s.dt));
    }

    fn stride(&self) -> u64 {
        (16 + 4 * (self.params.len() + self.inputs.len()) as u64).next_multiple_of(STEP_STRIDE)
    }

    fn frame_bytes(&self, time: f32) -> Vec<u8> {
        frame_bytes(time, &self.params, &self.inputs)
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
        let (Res::Rows(r), state) = (&self.res[st.res], state_bytes(&self.b.resources[st.res].shape)) else {
            unreachable!("validated: a stepped kernel writes rows")
        };
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

    pub fn precision(&self) -> Precision {
        self.precision
    }
    pub fn bundle(&self) -> &Bundle {
        &self.b
    }
    pub fn set_time(&mut self, seconds: f32) {
        self.time = seconds;
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
        self.inputs[at].copy_from_slice(value);
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
        queue.write_buffer(&self.frame, 0, &self.frame_bytes(self.time));
        self.dispatch_all(enc, &self.frame_kernels(), self.timestamps.as_ref(), &[self.own()]);
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
        for (p, pass) in self.b.passes.iter().enumerate() {
            let clear = |owned: bool| match (owned, &pass.clear) {
                (true, Clear::Color(c)) => {
                    let c = c.map(|x| f64::from(f32::from_bits(x.0)));
                    (wgpu::LoadOp::Clear(wgpu::Color { r: c[0], g: c[1], b: c[2], a: c[3] }), wgpu::LoadOp::Clear(1.0))
                }
                (true, Clear::Load) | (false, _) => (wgpu::LoadOp::Load, wgpu::LoadOp::Load),
            };
            let (color, depth, ops, key) = match pass.target {
                Target::View => (
                    Some(view.color),
                    view.depth,
                    clear(view.owner == Owner::Loft),
                    (Some(view.canvas.color), view.canvas.depth, view.canvas.samples),
                ),
                Target::Texture(r) => match &self.res[r as usize] {
                    Res::Target { tex, depth } => {
                        (Some(&tex.view), depth, clear(true), (Some(tex.tex.format()), DEPTH, 1))
                    }
                    Res::Tex(_) | Res::Rows(_) | Res::Frame => unreachable!("a pass's colour target is a Res::Target"),
                },
                Target::Shadow { map, .. } => match &self.res[map as usize] {
                    Res::Tex(t) => (None, &t.view, clear(true), (None, DEPTH, 1)),
                    Res::Target { .. } | Res::Rows(_) | Res::Frame => {
                        unreachable!("validated: a shadow map is a depth texture, never a colour target")
                    }
                },
            };
            let store = wgpu::StoreOp::Store;
            let colors: Vec<_> = color
                .map(|view| {
                    Some(wgpu::RenderPassColorAttachment {
                        view,
                        depth_slice: None,
                        resolve_target: None,
                        ops: wgpu::Operations { load: ops.0, store },
                    })
                })
                .into_iter()
                .collect();
            let timestamp_writes = self.timestamps.as_ref().map(|query_set| wgpu::RenderPassTimestampWrites {
                query_set,
                beginning_of_pass_write_index: Some(2 + 2 * p as u32),
                end_of_pass_write_index: Some(3 + 2 * p as u32),
            });
            let mut rp = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("loft pass"),
                color_attachments: &colors,
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: depth,
                    depth_ops: Some(wgpu::Operations { load: ops.1, store }),
                    stencil_ops: None,
                }),
                timestamp_writes,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            rp.set_bind_group(0, &self.g0[0], &[0]);
            for &it in &self.pass_items[p] {
                let item = &self.items[it];
                let drawn = match (&pass.target, view.layers, item.layer) {
                    (Target::View, Layers::Subject, Layer::Surroundings) => false,
                    (Target::View, Layers::Subject, Layer::Subject)
                    | (Target::View, Layers::All, _)
                    | (Target::Texture(_) | Target::Shadow { .. }, _, _) => true,
                };
                if drawn {
                    rp.set_pipeline(&self.render[&(it, key)]);
                    rp.set_bind_group(1, &self.g1[item.module as usize], &[]);
                    rp.draw(0..item.count[0], 0..item.count[1]);
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
        let written = |r: usize| frame.iter().any(|&k| self.b.kernels[k].writes.contains(&(r as Id)));
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
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                    buffer: &frame_buf,
                    offset: 0,
                    size: wgpu::BufferSize::new(self.frame_size),
                }),
            }],
        });
        let own: Vec<(usize, Res)> =
            (0..self.res.len()).filter(|&r| written(r)).map(|r| (r, self.resource(device, r))).collect();
        // A copy starts as the loft's resource is now (its init, or the last frame), not as zeros.
        let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("loft instance") });
        for (r, copy) in &own {
            match (&self.res[*r], copy) {
                (Res::Rows(a), Res::Rows(b)) => enc.copy_buffer_to_buffer(a, 0, b, 0, None),
                (Res::Tex(a) | Res::Target { tex: a, .. }, Res::Tex(b) | Res::Target { tex: b, .. }) => {
                    enc.copy_texture_to_texture(a.tex.as_image_copy(), b.tex.as_image_copy(), a.tex.size())
                }
                (Res::Rows(_) | Res::Tex(_) | Res::Target { .. } | Res::Frame, _) => {
                    unreachable!("a copy has its resource's shape, and kernels never write the frame")
                }
            }
        }
        queue.submit([enc.finish()]);
        let at = |r: usize| binding(own.iter().find(|(o, _)| *o == r).map_or(&self.res[r], |(_, res)| res));
        let g1 = self.b.modules.iter().zip(&self.bgl1).map(|(m, l)| group(device, l, &m.reads, &at)).collect();
        let g2 = self.b.kernels.iter().zip(&self.bgl2).map(|(k, l)| group(device, l, &k.writes, &at)).collect();
        Ok(Instance {
            time: self.time,
            params: self.params.clone(),
            inputs: self.inputs.clone(),
            frame: frame_buf,
            g0,
            g1,
            g2,
            _own: own.into_iter().map(|(_, res)| res).collect(),
        })
    }

    /// Writes each instance's frame and runs the `Frame` kernels for all of them in one compute pass (timestamps 0 and 1
    /// when set). Call once per frame before `draw_subject`.
    pub fn prepare(&self, queue: &wgpu::Queue, enc: &mut wgpu::CommandEncoder, instances: &[Instance]) {
        for i in instances {
            queue.write_buffer(&i.frame, 0, &frame_bytes(i.time, &i.params, &i.inputs));
        }
        let sets: Vec<Groups> = instances.iter().map(|i| (&i.g0, &i.g1[..], &i.g2[..])).collect();
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
                rp.set_bind_group(1, &i.g1[item.module as usize], &[]);
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
