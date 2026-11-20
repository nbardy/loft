//! Native execution of the Phase 1.5 picture and motion fixtures.
mod motion;
mod edit;
pub use edit::TraceSession;
mod bakes;
use loft_runner::mips;
mod paths;
mod shadow;
use loft_runner::command::{self, Dispatch, Stamps, Work};
use paths::PathGpu;
use bakes::TextureBake;
use loft_passes::emit::Aov;
use std::sync::{Mutex, atomic::{AtomicU32, Ordering}};
pub mod ray_query;
pub use motion::{sample_motion, sample_motion_timed};

use loft_passes::program::{abi, Draw, Pass, Program};
use std::borrow::Cow;
use std::time::Instant;

pub struct Gpu {
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub adapter: String,
    pub device_signature: String,
    pub timestamps: bool,
    /// The disk pipeline cache's file name, when the backend has one (Vulkan); `None` on Metal.
    pub cache_key: Option<String>,
    bake_formats: Vec<wgpu::TextureFormat>,
}

/// A high-performance device; timestamps when the adapter grants them.
pub fn gpu() -> Result<Gpu, String> {
    request_gpu(false)
}

pub fn request_gpu(ray_query: bool) -> Result<Gpu, String> {
    ray_query::request_gpu(ray_query)
}

pub const COLOR: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb;
const VIS: wgpu::TextureFormat = wgpu::TextureFormat::Rg32Uint;
const DEPTH: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;
const HDR: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;

/// What loading built and how long each step took (milliseconds).
#[derive(Clone, Debug, Default)]
pub struct LoadStats {
    pub modules: usize,
    pub render_pipelines: usize,
    pub compute_pipelines: usize,
    pub modules_ms: f64,
    pub pipelines_ms: f64,
    pub resources_ms: f64,
    pub load_bake_commands: u32,
    pub load_bake_gpu_ms: Option<f64>,
    pub load_bake_pass_ms: Vec<(String, f64)>,
    pub load_bake_ticks: Vec<u64>,
    /// Bytes read from the disk pipeline cache (0: none on this backend, or a cold cache).
    pub cache_bytes: usize,
    /// Each pipeline's own creation time on its thread (ms), to see which one sets the cold load.
    pub pipeline_ms: Vec<(String, f64)>,
}

/// Runs `f` and returns its result with its wall time in ms.
fn timed<T>(f: impl FnOnce() -> T) -> (T, f64) {
    let t = Instant::now();
    let x = f();
    (x, t.elapsed().as_secs_f64() * 1e3)
}

struct KindGpu {
    render: wgpu::RenderPipeline,
    shade: wgpu::ComputePipeline,
    module: wgpu::ShaderModule,
    /// Owned here so the tables live as long as the kind.
    rows: wgpu::Buffer,
    _params: wgpu::Buffer,
    g1: wgpu::BindGroup,
    g2: wgpu::BindGroup,
    grid: Option<(wgpu::Buffer, wgpu::BindGroup)>,
}

pub struct Loaded {
    size: (u32, u32),
    tiles: (u32, u32),
    kinds: Vec<KindGpu>,
    passes: Vec<Pass>,
    commands: Vec<command::Pass>,
    camera: loft_passes::program::Orbit,
    sun: [f32; 3],
    n_inst: u32,
    frame_buf: wgpu::Buffer,
    g0: wgpu::BindGroup,
    bgl0: wgpu::BindGroupLayout,
    bgl_kind: wgpu::BindGroupLayout,
    bgl_shade: wgpu::BindGroupLayout,
    instance_bounds: wgpu::Buffer,
    draw_args: wgpu::Buffer,
    draw_reset: Vec<u8>,
    shade_args: wgpu::Buffer,
    shade_reset: Vec<u8>,
    cull: (wgpu::ComputePipeline, wgpu::BindGroup),
    classify: (wgpu::ComputePipeline, wgpu::BindGroup),
    present: command::Raster,
    bundle: command::Raster,
    /// The sun's view: its uniform, bundle, targets, and the texel size in metres; `None` without a shadow pass.
    shadow: Option<Shadow>,
    vis: wgpu::TextureView,
    depth: wgpu::TextureView,
    color: wgpu::Texture,
    color_view: wgpu::TextureView,
    clock: Option<Clock>,
    bake_textures: Mutex<Vec<TextureBake>>,
    params: wgpu::Buffer,
    param_values: Mutex<(Vec<f32>, bool)>,
    overdraw: Option<(command::Raster, wgpu::TextureView)>,
    path: Option<PathGpu>,
    pub stats: LoadStats,
}

struct Shadow {
    groups: (wgpu::BindGroup, Vec<wgpu::BindGroup>),
    args: wgpu::Buffer,
    frame_buf: wgpu::Buffer,
    bundle: command::Raster,
    vis: wgpu::TextureView,
    depth: wgpu::TextureView,
    view_proj: [[f32; 4]; 4],
    texel: f32,
    dir: [f32; 3],
    pass: usize,
    reuse: Mutex<shadow::Reuse>,
}

/// Timestamps at the start and end of every pass, resolved after the frame.
struct Clock {
    set: wgpu::QuerySet,
    resolve: wgpu::Buffer,
    read: wgpu::Buffer,
    n: u32,
    period_ns: f64,
    skipped_pass: AtomicU32,
}

/// One submitted frame's GPU timestamps in pass order.
#[derive(serde::Serialize)]
pub struct FrameTiming {
    pub serial: usize,
    pub commands: u32,
    pub gpu_ms: f64,
    pub pass_ms: Vec<f64>,
    /// Source-pass order. A reused Shadow pass has the pair `[0, 0]`.
    pub ticks: Vec<u64>,
}

struct ClockRead {
    submission: wgpu::SubmissionIndex,
    mapped: std::sync::mpsc::Receiver<Result<(), wgpu::BufferAsyncError>>,
    skipped_pass: u32,
    bytes: u64,
}

fn bytes_of(words: &[u32]) -> Vec<u8> {
    words.iter().flat_map(|w| w.to_le_bytes()).collect()
}

fn f32_bytes(xs: &[f32]) -> Vec<u8> {
    xs.iter().flat_map(|w| w.to_le_bytes()).collect()
}

fn layout(device: &wgpu::Device, label: &str, entries: &[(wgpu::ShaderStages, wgpu::BindingType)]) -> wgpu::BindGroupLayout {
    let entries: Vec<_> = entries
        .iter()
        .enumerate()
        .map(|(i, (visibility, ty))| wgpu::BindGroupLayoutEntry { binding: i as u32, visibility: *visibility, ty: *ty, count: None })
        .collect();
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor { label: Some(label), entries: &entries })
}

fn bind(device: &wgpu::Device, layout: &wgpu::BindGroupLayout, resources: &[wgpu::BindingResource<'_>]) -> wgpu::BindGroup {
    let entries: Vec<_> =
        resources.iter().enumerate().map(|(i, r)| wgpu::BindGroupEntry { binding: i as u32, resource: r.clone() }).collect();
    device.create_bind_group(&wgpu::BindGroupDescriptor { label: None, layout, entries: &entries })
}

fn storage(read_only: bool) -> wgpu::BindingType {
    wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Storage { read_only }, has_dynamic_offset: false, min_binding_size: None }
}
const UNIFORM: wgpu::BindingType =
    wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Uniform, has_dynamic_offset: false, min_binding_size: None };
fn texture(sample_type: wgpu::TextureSampleType) -> wgpu::BindingType {
    wgpu::BindingType::Texture { sample_type, view_dimension: wgpu::TextureViewDimension::D2, multisampled: false }
}
const ALL: wgpu::ShaderStages = wgpu::ShaderStages::VERTEX_FRAGMENT.union(wgpu::ShaderStages::COMPUTE);
const CS: wgpu::ShaderStages = wgpu::ShaderStages::COMPUTE;
const FS: wgpu::ShaderStages = wgpu::ShaderStages::FRAGMENT;

fn buffer(device: &wgpu::Device, label: &str, size: u64, usage: wgpu::BufferUsages) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor { label: Some(label), size: size.max(16), usage, mapped_at_creation: false })
}

fn tex(device: &wgpu::Device, label: &str, size: (u32, u32), format: wgpu::TextureFormat, usage: wgpu::TextureUsages) -> wgpu::Texture {
    device.create_texture(&wgpu::TextureDescriptor {
        label: Some(label),
        size: wgpu::Extent3d { width: size.0, height: size.1, depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage,
        view_formats: &[],
    })
}

/// The module with its cold-load nonce: a distinct value changes the code the driver compiles, never what it does
/// (the uniform it is compared with is always 0).
fn shader(device: &wgpu::Device, m: &loft_passes::program::ShaderModule, nonce: u32) -> wgpu::ShaderModule {
    let mut module = m.naga.clone();
    let c = module.constants.iter().find(|(_, c)| c.name.as_deref() == Some("LOFT_NONCE")).map(|(_, c)| c.init);
    if let Some(init) = c {
        module.global_expressions[init] = naga::Expression::Literal(naga::Literal::U32(nonce));
    }
    device.create_shader_module(wgpu::ShaderModuleDescriptor { label: Some(&m.name), source: wgpu::ShaderSource::Naga(Cow::Owned(module)) })
}

fn compute(device: &wgpu::Device, label: &str, layouts: &[&wgpu::BindGroupLayout], module: &wgpu::ShaderModule, entry: &str, cache: Option<&wgpu::PipelineCache>) -> wgpu::ComputePipeline {
    compute_constants(device, label, layouts, module, entry, cache, &[])
}

fn compute_constants(device: &wgpu::Device, label: &str, layouts: &[&wgpu::BindGroupLayout], module: &wgpu::ShaderModule, entry: &str, cache: Option<&wgpu::PipelineCache>, constants: &[(&str, f64)]) -> wgpu::ComputePipeline {
    let layouts: Vec<Option<&wgpu::BindGroupLayout>> = layouts.iter().map(|l| Some(*l)).collect();
    let pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor { label: None, bind_group_layouts: &layouts, immediate_size: 0 });
    device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: Some(label),
        layout: Some(&pl),
        module,
        entry_point: Some(entry),
        compilation_options: wgpu::PipelineCompilationOptions { constants, ..Default::default() },
        cache,
    })
}

#[allow(clippy::too_many_arguments)]
fn render(
    device: &wgpu::Device,
    label: &str,
    layouts: &[&wgpu::BindGroupLayout],
    module: &wgpu::ShaderModule,
    entries: (&str, &str),
    target: wgpu::TextureFormat,
    depth: bool,
    cull: Option<wgpu::Face>,
    cache: Option<&wgpu::PipelineCache>,
) -> wgpu::RenderPipeline {
    render_blended(device, label, layouts, module, entries, target, depth, cull, cache, None)
}

#[allow(clippy::too_many_arguments)]
fn render_blended(
    device: &wgpu::Device,
    label: &str,
    layouts: &[&wgpu::BindGroupLayout],
    module: &wgpu::ShaderModule,
    entries: (&str, &str),
    target: wgpu::TextureFormat,
    depth: bool,
    cull: Option<wgpu::Face>,
    cache: Option<&wgpu::PipelineCache>,
    blend: Option<wgpu::BlendState>,
) -> wgpu::RenderPipeline {
    let layouts: Vec<Option<&wgpu::BindGroupLayout>> = layouts.iter().map(|l| Some(*l)).collect();
    let pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor { label: None, bind_group_layouts: &layouts, immediate_size: 0 });
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some(label),
        layout: Some(&pl),
        vertex: wgpu::VertexState { module, entry_point: Some(entries.0), compilation_options: Default::default(), buffers: &[] },
        fragment: Some(wgpu::FragmentState {
            module,
            entry_point: Some(entries.1),
            compilation_options: Default::default(),
            targets: &[Some(wgpu::ColorTargetState { format: target, blend, write_mask: wgpu::ColorWrites::ALL })],
        }),
        primitive: wgpu::PrimitiveState { cull_mode: cull, front_face: wgpu::FrontFace::Ccw, ..Default::default() },
        depth_stencil: depth.then(|| wgpu::DepthStencilState {
            format: DEPTH,
            depth_write_enabled: Some(true),
            depth_compare: Some(wgpu::CompareFunction::Less),
            stencil: Default::default(),
            bias: Default::default(),
        }),
        multisample: Default::default(),
        multiview_mask: None,
        cache,
    })
}

impl Clock {
    fn stamps(&self, pass: u32) -> Stamps<'_> {
        Stamps { set: &self.set, start: Some(pass * 2), end: Some(pass * 2 + 1) }
    }
    fn new(g: &Gpu, n: u32) -> Self {
        use wgpu::BufferUsages as U;
        Self {
            set: g.device.create_query_set(&wgpu::QuerySetDescriptor {
                label: Some("GPU timer"), ty: wgpu::QueryType::Timestamp, count: n,
            }),
            resolve: buffer(&g.device, "ts resolve", u64::from(n) * 8, U::QUERY_RESOLVE | U::COPY_SRC),
            read: buffer(&g.device, "ts read", u64::from(n) * 8, U::COPY_DST | U::MAP_READ),
            n, period_ns: f64::from(g.queue.get_timestamp_period()),
            skipped_pass: AtomicU32::new(u32::MAX),
        }
    }

    // Metal exposes stale end samples if the resolve is encoded before the render
    // submission completes. Each slot keeps its own query set until after unmap.
    fn resolve_completed(&self, g: &Gpu) -> ClockRead {
        let skipped_pass = self.skipped_pass.load(Ordering::Relaxed);
        let n = self.n - 2 * u32::from(skipped_pass != u32::MAX);
        let bytes = u64::from(n) * 8;
        let mut enc = g.device.create_command_encoder(&Default::default());
        enc.resolve_query_set(&self.set, 0..n, &self.resolve, 0);
        enc.copy_buffer_to_buffer(&self.resolve, 0, &self.read, 0, Some(bytes));
        let submission = g.queue.submit([enc.finish()]);
        let (tx, mapped) = std::sync::mpsc::channel();
        self.read.map_async(wgpu::MapMode::Read, ..bytes, move |result| { let _ = tx.send(result); });
        ClockRead { submission, mapped, skipped_pass, bytes }
    }

    fn collect(&self, g: &Gpu, read: ClockRead) -> Result<Vec<u64>, String> {
        wait_for(g, Some(read.submission))?;
        read.mapped.try_recv().map_err(|e| format!("timestamp callback: {e}"))?
            .map_err(|e| format!("timestamp map: {e}"))?;
        let mut ticks: Vec<u64> = {
            let view = self.read.get_mapped_range(..read.bytes).map_err(|e| e.to_string())?;
            view.as_chunks::<8>().0.iter().map(|b| u64::from_le_bytes(*b)).collect()
        };
        self.read.unmap();
        if ticks.contains(&0) {
            return Err(format!("unusable timestamps {ticks:?}"));
        }
        if read.skipped_pass != u32::MAX {
            // Restore reporting in source pass order after resolving only the executed passes.
            let at = 2 * read.skipped_pass as usize;
            ticks.splice(at..at, [0, 0]);
        }
        Ok(ticks)
    }

    fn timing(&self, serial: usize, commands: u32, ticks: Vec<u64>) -> Result<FrameTiming, String> {
        let (first, last) = (ticks[0], ticks[self.n as usize - 1]);
        if first == 0 || last <= first {
            return Err(format!("frame {serial}: unusable timestamps {ticks:?}"));
        }
        let ms = |a: u64, b: u64| b.saturating_sub(a) as f64 * self.period_ns / 1e6;
        // Pass starts can overlap on Metal. Use consecutive ends to attribute
        // costs, and preserve the original pairs so every span can be inspected.
        let pass_ms = ticks.chunks_exact(2).scan(first, |end, pair| {
            Some(if pair[1] == 0 { 0.0 } else { ms(std::mem::replace(end, pair[1]), pair[1]) })
        }).collect();
        Ok(FrameTiming { serial, commands, gpu_ms: ms(first, last), pass_ms, ticks })
    }
}

fn wait_for(g: &Gpu, submission_index: Option<wgpu::SubmissionIndex>) -> Result<(), String> {
    g.device.poll(wgpu::PollType::Wait { submission_index, timeout: None })
        .map(|_| ()).map_err(|e| e.to_string())
}

fn read_clock(g: &Gpu, c: &Clock) -> Result<Vec<u64>, String> {
    wait_for(g, None)?;
    c.collect(g, c.resolve_completed(g))
}

impl Gpu {
    pub fn triangle_control_ms(&self, size: (u32, u32)) -> Result<f64, String> {
        if !self.timestamps { return Err("the speed control needs GPU timestamps".into()); }
        let d = &self.device;
        let format = wgpu::TextureFormat::Rgba8Unorm;
        let image = tex(d, "speed control", size, format, wgpu::TextureUsages::RENDER_ATTACHMENT);
        let view = image.create_view(&Default::default());
        let module = d.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("speed control"),
            source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(
                "@vertex fn vs(@builtin(vertex_index) i: u32) -> @builtin(position) vec4<f32> {
                   let p = array(vec2(-1., -1.), vec2(3., -1.), vec2(-1., 3.));
                   return vec4(p[i], 0., 1.);
                 }
                 @fragment fn fs() -> @location(0) vec4<f32> { return vec4(0.25, 0.5, 0.75, 1.); }",
            )),
        });
        let pipe = render(d, "speed control", &[], &module, ("vs", "fs"), format, false, None, None);
        let c = Clock::new(self, 2);
        let mut times = [0.; 11];
        for ms in &mut times {
            let mut encoder = d.create_command_encoder(&Default::default());
            {
                let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &view, depth_slice: None, resolve_target: None,
                        ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color::BLACK), store: wgpu::StoreOp::Store },
                    })],
                    timestamp_writes: Some(wgpu::RenderPassTimestampWrites {
                        query_set: &c.set, beginning_of_pass_write_index: Some(0), end_of_pass_write_index: Some(1),
                    }),
                    ..Default::default()
                });
                pass.set_pipeline(&pipe);
                pass.draw(0..3, 0..1);
            }
            self.queue.submit([encoder.finish()]);
            let ticks = read_clock(self, &c)?;
            let duration = ticks[1].checked_sub(ticks[0]).filter(|&x| x > 0 && ticks[0] != 0)
                .ok_or("speed control has unusable timestamps")?;
            *ms = duration as f64 * c.period_ns / 1e6;
        }
        times[2..].sort_by(f64::total_cmp);
        Ok(times[6])
    }
}

fn draw_bundle(
    d: &wgpu::Device, main: &wgpu::BindGroup, kinds: &[KindGpu], pass: &[u32],
    args: &wgpu::Buffer, overrides: Option<&[wgpu::BindGroup]>,
) -> command::Raster {
    let arg_stride = if kinds.iter().any(|kind| kind.grid.is_some()) { 20 } else { 16 };
    let draws = pass.iter().map(|&k| {
        let kg = &kinds[k as usize];
        let group = overrides.map_or(&kg.g1, |groups| &groups[k as usize]);
        let offset = u64::from(k) * arg_stride;
        let (groups, source) = match &kg.grid {
            Some((indices, grid)) => (vec![group, grid], command::DrawSource::Indexed {
                args, offset, indices, format: wgpu::IndexFormat::Uint32,
            }),
            None => (vec![group], command::DrawSource::Indirect { args, offset }),
        };
        command::Draw { pipeline: &kg.render, groups, source }
    });
    command::Raster::prepare(d, &wgpu::RenderBundleEncoderDescriptor {
        label: Some("visibility"),
        color_formats: &[Some(VIS)],
        depth_stencil: Some(wgpu::RenderBundleDepthStencil {
            format: DEPTH, depth_read_only: false, stencil_read_only: true,
        }),
        sample_count: 1, multiview: None,
    }, &[main], draws)
}

fn grid_layout(device: &wgpu::Device, write: bool) -> wgpu::BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("parametric grid"),
        entries: &[wgpu::BindGroupLayoutEntry {
            binding: if write { 6 } else { 5 },
            visibility: if write { CS } else { wgpu::ShaderStages::VERTEX },
            ty: storage(!write),
            count: None,
        }],
    })
}

fn grid_group(device: &wgpu::Device, bind_layout: &wgpu::BindGroupLayout, binding: u32, grid: &wgpu::Buffer) -> wgpu::BindGroup {
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("parametric grid"),
        layout: bind_layout,
        entries: &[wgpu::BindGroupEntry { binding, resource: grid.as_entire_binding() }],
    })
}

fn instance_bytes(p: &Program) -> Vec<u8> {
    let mut bytes = Vec::new();
    for i in &p.instances {
        bytes.extend(f32_bytes(&i.lo));
        bytes.extend(i.kind.to_le_bytes());
        bytes.extend(f32_bytes(&i.hi));
        bytes.extend(i.row.to_le_bytes());
    }
    bytes
}

fn caster_bounds(p: &Program, casters: &[u32]) -> ([f32; 3], [f32; 3]) {
    let (mut lo, mut hi) = ([f32::INFINITY; 3], [f32::NEG_INFINITY; 3]);
    for i in p.instances.iter().filter(|i| casters.contains(&i.kind)) {
        for a in 0..3 {
            lo[a] = lo[a].min(i.lo[a]);
            hi[a] = hi[a].max(i.hi[a]);
        }
    }
    (lo, hi)
}

impl Loaded {
    /// Builds every GPU object for `p` at `size`. `nonce` (any value but `u32::MAX`) forces a cold driver compile.
    pub fn new(g: &Gpu, p: &Program, size: (u32, u32), nonce: Option<u32>) -> Result<Loaded, String> {
        Self::load(g, p, size, nonce, None, Aov::Beauty)
    }

    pub fn with_aov(g: &Gpu, p: &Program, size: (u32, u32), nonce: Option<u32>, aov: Aov) -> Result<Loaded, String> {
        Self::load(g, p, size, nonce, None, aov)
    }

    fn load(
        g: &Gpu, p: &Program, size: (u32, u32), nonce: Option<u32>,
        edit: Option<(usize, &loft_passes::tier0::TraceEdit)>, aov: Aov,
    ) -> Result<Loaded, String> {
        let d = &g.device;
        let color_format = if aov == Aov::Beauty { COLOR } else { wgpu::TextureFormat::Rgba8Unorm };
        let limits = d.limits();
        if size.0 == 0 || size.1 == 0 || size.0.max(size.1) > limits.max_texture_dimension_2d {
            return Err("frame dimensions exceed device limits".into());
        }
        let nonce = nonce.unwrap_or(u32::MAX);
        let k_n = p.kinds.len();
        let tiles = (size.0.div_ceil(abi::TILE), size.1.div_ceil(abi::TILE));
        let mut stats = LoadStats { modules: p.modules.len(), ..Default::default() };

        let t = Instant::now();
        let mut modules: Vec<wgpu::ShaderModule> = p.modules.iter().map(|m| shader(d, m, nonce)).collect();
        if let Some((i, edit)) = edit {
            let index = p.kinds[i].module;
            let source = edit.shader(&p.modules[index].wgsl)?;
            let ir = naga::front::wgsl::parse_str(&source).map_err(|e| e.emit_to_string(&source))?;
            naga::valid::Validator::new(naga::valid::ValidationFlags::all(), naga::valid::Capabilities::default())
                .validate(&ir).map_err(|e| e.emit_to_string(&source))?;
            modules[index] = d.create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("editable trace"),
                source: wgpu::ShaderSource::Naga(Cow::Owned(ir)),
            });
        }
        stats.modules_ms = t.elapsed().as_secs_f64() * 1e3;

        let t = Instant::now();
        let bgl0 = layout(d, "frame", &[(ALL, UNIFORM),(ALL,storage(true)),(ALL,storage(true))]);
        let bgl_kind = layout(d, "kind", &[(ALL, storage(true)), (wgpu::ShaderStages::VERTEX, storage(true)), (ALL, UNIFORM), (ALL, storage(true))]);
        let bgl_shade = layout(
            d,
            "shade",
            &[
                (CS, texture(wgpu::TextureSampleType::Uint)),
                (CS, wgpu::BindingType::StorageTexture { access: wgpu::StorageTextureAccess::WriteOnly, format: HDR, view_dimension: wgpu::TextureViewDimension::D2 }),
                (CS, storage(true)),
                (CS, texture(wgpu::TextureSampleType::Depth)),
                (CS, wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Comparison)),
            ],
        );
        let bgl_cull = layout(d, "cull", &[(CS, storage(true)), (CS, storage(false)), (CS, storage(false)), (CS, storage(true))]);
        let bgl_classify = layout(d, "classify", &[(CS, texture(wgpu::TextureSampleType::Uint)), (CS, storage(false)), (CS, storage(false))]);
        let bgl_present =
            layout(d, "present", &[(FS, texture(wgpu::TextureSampleType::Float { filterable: false })), (FS, texture(wgpu::TextureSampleType::Uint))]);
        let path_layout = p.path.as_ref().map(|path| PathGpu::layout(d, path, &p.modules[p.present].naga)).transpose()?;
        let present_layouts = [&bgl0, &bgl_present].into_iter().chain(path_layout.iter()).collect::<Vec<_>>();
        let indexed = |kt: &loft_passes::program::KindTable| {
            aov != Aov::Overdraw && p.modules[kt.module].naga.entry_points.iter().any(|e| e.name == "vs_indexed")
        };
        let arg_words = if p.kinds.iter().any(indexed) { 5u32 } else { 4 };
        let bgl_grid = (arg_words == 5).then(|| grid_layout(d, false));
        // The disk pipeline cache, where the backend has one (see `gpu`).
        let cache_path = g.cache_key.as_ref().map(|k| std::path::Path::new("out/meta/pipeline_cache").join(k).with_extension(format!("{nonce}.bin")));
        let cache_data = cache_path.as_ref().and_then(|p| std::fs::read(p).ok());
        stats.cache_bytes = cache_data.as_ref().map_or(0, Vec::len);
        // Unsafe by wgpu's contract: the bytes must come from `get_data` on a compatible adapter. They do (the file name
        // is the adapter's cache key), and `fallback: true` makes wgpu discard a mismatched header instead of failing.
        let cache = cache_path.as_ref().map(|_| {
            #[allow(unsafe_code)]
            unsafe {
                d.create_pipeline_cache(&wgpu::PipelineCacheDescriptor { label: Some("loft"), data: cache_data.as_deref(), fallback: true })
            }
        });
        let cache = cache.as_ref();
        // Every pipeline at once: the driver compiles each on the calling thread, and wgpu's device is Sync, so one
        // thread per pipeline turns the cold compile from the sum of the pipelines into roughly the slowest one.
        let (kinds_pl, cull_pl, classify_pl, present_pl) = std::thread::scope(|sc| {
            let kinds: Vec<_> = p
                .kinds
                .iter()
                .map(|kt| {
                    let (m, bgl0, bgl_kind, bgl_shade) = (&modules[kt.module], &bgl0, &bgl_kind, &bgl_shade);
                    // A proxy draws its box's back faces (the camera may be inside); a surface draws both sides.
                    let face = match kt.draw {
                        Draw::Analytic | Draw::Traced => Some(wgpu::Face::Front),
                        Draw::Surface { .. } | Draw::Mesh { .. } => None,
                    };
                    let grid = bgl_grid.as_ref().filter(|_| indexed(kt));
                    let r = sc.spawn(move || timed(|| {
                        let layouts = [bgl0, bgl_kind].into_iter().chain(grid).collect::<Vec<_>>();
                        let entry = if grid.is_some() { "vs_indexed" } else { "vs" };
                        render(d, &kt.name, &layouts, m, (entry, "fs"), VIS, true, face, cache)
                    }));
                    let s = sc.spawn(move || timed(|| compute(d, &kt.name, &[bgl0, bgl_kind, bgl_shade], m, "shade", cache)));
                    (r, s)
                })
                .collect();
            let cull = sc.spawn(|| timed(|| {
                let words = [("loft_draw_words", f64::from(arg_words))];
                let constants = if arg_words == 5 { &words[..] } else { &[] };
                compute_constants(d, "cull", &[&bgl0, &bgl_cull], &modules[p.cull], "cull", cache, constants)
            }));
            let classify = sc.spawn(|| timed(|| compute(d, "classify", &[&bgl0, &bgl_classify], &modules[p.classify], "classify", cache)));
            let present = sc.spawn(|| timed(|| render(d, "present", &present_layouts, &modules[p.present], ("vs_present", "fs_present"), color_format, false, None, cache)));
            fn join<T>(h: std::thread::ScopedJoinHandle<'_, T>) -> T {
                h.join().expect("pipeline thread")
            }
            let kinds: Vec<_> = kinds.into_iter().enumerate().map(|(k, (r, s))| (k, join(r), join(s))).collect();
            (kinds, join(cull), join(classify), join(present))
        });
        for (k, r, s) in &kinds_pl {
            stats.pipeline_ms.push((format!("{} vis", p.kinds[*k].name), r.1));
            stats.pipeline_ms.push((format!("{} shade", p.kinds[*k].name), s.1));
        }
        stats.pipeline_ms.extend([("cull".into(), cull_pl.1), ("classify".into(), classify_pl.1), ("present".into(), present_pl.1)]);
        let kinds_pl: Vec<_> = kinds_pl.into_iter().map(|(k, r, s)| (k, r.0, s.0)).collect();
        let (cull_pl, classify_pl, present_pl) = (cull_pl.0, classify_pl.0, present_pl.0);
        // Metal compiles at creation; wait so the time is the compile, not a deferred one inside the first frame.
        d.poll(wgpu::PollType::wait_indefinitely()).map_err(|e| e.to_string())?;
        stats.pipelines_ms = t.elapsed().as_secs_f64() * 1e3;
        if let (Some(c), Some(path)) = (cache, &cache_path)
            && let Some(data) = c.get_data()
        {
            let _ = std::fs::create_dir_all("out/meta/pipeline_cache");
            std::fs::write(path, data).map_err(|e| format!("{}: {e}", path.display()))?;
        }
        stats.render_pipelines = k_n + 1;
        stats.compute_pipelines = k_n + 2;

        let t = Instant::now();
        use wgpu::BufferUsages as U;
        let frame_buf = buffer(d, "frame", abi::FRAME_BYTES, U::UNIFORM | U::COPY_DST);
        let inputs = buffer(d, "empty input table", 16, U::STORAGE);
        let params = buffer(d, "parameters", (p.params.len() as u64 * 4).max(16), U::STORAGE | U::COPY_DST);
        if !p.params.is_empty() { g.queue.write_buffer(&params, 0, &f32_bytes(&p.params)); }
        let g0 = bind(d, &bgl0, &[frame_buf.as_entire_binding(), inputs.as_entire_binding(), params.as_entire_binding()]);
        let n_inst = p.instances.len() as u32;
        let mut bases = vec![];
        let mut base = 0u32;
        for kt in &p.kinds {
            bases.push(base);
            base += kt.n_rows;
        }
        let init = |label: &str, bytes: &[u8], usage: U| {
            let b = buffer(d, label, bytes.len() as u64, usage | U::COPY_DST);
            g.queue.write_buffer(&b, 0, bytes);
            b
        };
        let mut inst = instance_bytes(p);
        // Even a zero-workgroup cull validates the array binding against one 32-byte element.
        if inst.is_empty() { inst.resize(32, 0); }
        let insts = init("instances", &inst, U::STORAGE);
        let bases_buf = init("bases", &bytes_of(&bases), U::STORAGE);
        let visible = buffer(d, "visible", u64::from(n_inst.max(1)) * 4, U::STORAGE);
        let args = |all: bool| bytes_of(&p.kinds.iter().flat_map(|k| {
            [k.draw.vertices(), if all { k.n_rows } else { 0 }, 0, 0, 0].into_iter().take(arg_words as usize)
        }).collect::<Vec<_>>());
        let draw_reset = args(false);
        let luts = init("luts", &f32_bytes(if p.luts.is_empty() { &[0.0] } else { &p.luts }), U::STORAGE);
        let shade_reset = bytes_of(&(0..k_n).flat_map(|_| [0u32, 1, 1]).collect::<Vec<_>>());
        let draw_args = init("draw args", &draw_reset, U::STORAGE | U::INDIRECT);
        let shade_args = init("shade args", &shade_reset, U::STORAGE | U::INDIRECT);
        let tile_cap = tiles.0 * tiles.1;
        let tile_lists = buffer(d, "tiles", u64::from(tile_cap) * 4 * k_n as u64, U::STORAGE);
        use wgpu::TextureUsages as T;
        let vis_tex = tex(d, "vis64", size, VIS, T::RENDER_ATTACHMENT | T::TEXTURE_BINDING);
        let depth_tex = tex(d, "depth", size, DEPTH, T::RENDER_ATTACHMENT);
        let hdr_tex = tex(d, "hdr", size, HDR, T::STORAGE_BINDING | T::TEXTURE_BINDING | T::RENDER_ATTACHMENT);
        let color = tex(d, "color", size, color_format, T::RENDER_ATTACHMENT | T::COPY_SRC);
        let view = |t: &wgpu::Texture| t.create_view(&Default::default());
        let (vis, depth, hdr, color_view) = (view(&vis_tex), view(&depth_tex), view(&hdr_tex), view(&color));
        // The sun's view: an orthographic box around every caster row, deep enough to reach the receivers below.
        let mut sun_passes = p.passes.iter().enumerate().filter_map(|(i, pass)| {
            if let Pass::Shadow(casters) = pass { Some((i, casters.as_slice())) } else { None }
        });
        let (sun_pass, casters) = sun_passes.next().unwrap_or_default();
        let side = p.shadow_size.max(1);
        let shadow_depth = tex(d, "shadow depth", (side, side), DEPTH, T::RENDER_ATTACHMENT | T::TEXTURE_BINDING);
        let shadow_view = view(&shadow_depth);
        let smp = d.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("shadow"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            compare: Some(wgpu::CompareFunction::LessEqual),
            ..Default::default()
        });
        let g2 = bind(
            d,
            &bgl_shade,
            &[
                wgpu::BindingResource::TextureView(&vis),
                wgpu::BindingResource::TextureView(&hdr),
                tile_lists.as_entire_binding(),
                wgpu::BindingResource::TextureView(&shadow_view),
                wgpu::BindingResource::Sampler(&smp),
            ],
        );
        // Every row of every kind, for the sun's bundle (identity row lists; the camera's cull does not apply).
        let all: Vec<u32> = p.kinds.iter().flat_map(|k| 0..k.n_rows).collect();
        let all_rows = init("all rows", &bytes_of(&all), U::STORAGE);
        let shadow_args = init("shadow args", &args(true), U::INDIRECT);
        let mut kinds = vec![];
        let mut g1_sun = vec![];
        let mut grid_encoder = bgl_grid.as_ref().map(|_| d.create_command_encoder(&Default::default()));
        let bgl_grid_out = bgl_grid.as_ref().map(|_| grid_layout(d, true));
        for (k, render, shade) in kinds_pl {
            let kt = &p.kinds[k];
            let bytes = f32_bytes(&kt.rows);
            let rows = if let Some((_, tape)) = edit.filter(|(i, _)| *i == k) {
                let offset = bytes.len() as u64;
                let capacity = offset + loft_passes::tier0::TAPE_BYTES;
                if capacity > d.limits().max_storage_buffer_binding_size {
                    return Err("editable rows exceed GPU storage binding capacity".into());
                }
                let rows = buffer(d, &kt.name, capacity, U::STORAGE | U::COPY_DST);
                g.queue.write_buffer(&rows, 0, &bytes);
                g.queue.write_buffer(&rows, offset, &tape.bytes());
                rows
            } else {
                init(&kt.name, &bytes, U::STORAGE)
            };
            let params = init("kind params", &bytes_of(&[k as u32, bases[k], kt.n_rows, 0]), U::UNIFORM);
            let g1 = bind(d, &bgl_kind, &[rows.as_entire_binding(), visible.as_entire_binding(), params.as_entire_binding(), luts.as_entire_binding()]);
            g1_sun.push(bind(d, &bgl_kind, &[rows.as_entire_binding(), all_rows.as_entire_binding(), params.as_entire_binding(), luts.as_entire_binding()]));
            let grid = match kt.draw {
                Draw::Surface { nu, nv } if indexed(kt) => {
                    let grid_size = u64::from(nu + 1) * u64::from(nv + 1);
                    if grid_size * 16 > d.limits().max_storage_buffer_binding_size {
                        return Err(format!("{}: surface grid exceeds the device storage limit", kt.name));
                    }
                    let points = buffer(d, "surface grid", grid_size * 16, U::STORAGE);
                    let write = bgl_grid_out.as_ref().expect("indexed surface layout");
                    let out = grid_group(d, write, 6, &points);
                    let (pipeline, ms) = timed(|| compute(
                        d, "bake surface", &[&bgl0, &bgl_kind, write], &modules[kt.module], "bake_surface", cache,
                    ));
                    stats.compute_pipelines += 1;
                    stats.pipeline_ms.push((format!("{} bake surface", kt.name), ms));
                    command::Pass::Compute(vec![Dispatch { pipeline, groups: vec![g0.clone(), g1.clone(), out],
                        work: Work::Direct([(grid_size as u32).div_ceil(64),1,1]) }])
                        .record(grid_encoder.as_mut().expect("indexed surface encoder"),None);
                    // Keep the original vertex order and both sides of the UV seam.
                    let indices = (0..nu * nv).flat_map(|cell| (0..6).map(move |corner| {
                        let u = cell % nu + ((0x16u32 >> corner) & 1);
                        let v = cell / nu + ((0x34u32 >> corner) & 1);
                        v * (nu + 1) + u
                    })).collect::<Vec<_>>();
                    let index = init("surface indices", &bytes_of(&indices), U::INDEX);
                    let group = grid_group(d, bgl_grid.as_ref().expect("indexed surface layout"), 5, &points);
                    Some((index, group))
                }
                _ => None,
            };
            kinds.push(KindGpu { render, shade, module: modules[kt.module].clone(), rows, _params: params, g1, g2: g2.clone(), grid });
        }
        if let Some(encoder) = grid_encoder {
            wait_for(g, Some(g.queue.submit([encoder.finish()])))?;
        }
        let cull_bg = bind(d, &bgl_cull, &[insts.as_entire_binding(), draw_args.as_entire_binding(), visible.as_entire_binding(), bases_buf.as_entire_binding()]);
        let classify_bg =
            bind(d, &bgl_classify, &[wgpu::BindingResource::TextureView(&vis), shade_args.as_entire_binding(), tile_lists.as_entire_binding()]);
        let present_bg = bind(d, &bgl_present, &[wgpu::BindingResource::TextureView(&hdr), wgpu::BindingResource::TextureView(&vis)]);

        // The bundle: recorded once, replayed every frame. Per kind: set pipeline, bind, one drawIndirect.
        let vis_kinds: Vec<u32> = p.passes.iter().find_map(|x| if let Pass::Vis(k) = x { Some(k.clone()) } else { None }).unwrap_or_default();
        let bundle = draw_bundle(d, &g0, &kinds, &vis_kinds, &draw_args, None);
        let overdraw = (aov == Aov::Overdraw).then(|| {
            let component = wgpu::BlendComponent { src_factor: wgpu::BlendFactor::One, dst_factor: wgpu::BlendFactor::One, operation: wgpu::BlendOperation::Add };
            let pipelines: Vec<_> = vis_kinds.iter().map(|&k| {
                let kt = &p.kinds[k as usize];
                let face = match kt.draw { Draw::Analytic | Draw::Traced => Some(wgpu::Face::Front), Draw::Surface { .. } | Draw::Mesh { .. } => None };
                render_blended(d, &kt.name, &[&bgl0, &bgl_kind], &modules[kt.module], ("vs", "fs_overdraw"),
                    HDR, false, face, cache, Some(wgpu::BlendState { color: component, alpha: component }))
            }).collect();
            let draws = vis_kinds.iter().zip(&pipelines).map(|(&kind, pipeline)| command::Draw {
                pipeline, groups: vec![&kinds[kind as usize].g1],
                source: command::DrawSource::Indirect { args: &draw_args, offset: u64::from(kind) * 16 },
            });
            let raster = command::Raster::prepare(d, &wgpu::RenderBundleEncoderDescriptor {
                label: Some("overdraw"), color_formats: &[Some(HDR)], depth_stencil: None,
                sample_count: 1, multiview: None,
            }, &[&g0], draws);
            (raster, hdr)
        });
        let shadow = (p.shadow_size > 0).then(|| {
            let frame_buf = buffer(d, "sun frame", abi::FRAME_BYTES, U::UNIFORM | U::COPY_DST);
            let g0s = bind(d, &bgl0, &[frame_buf.as_entire_binding(),inputs.as_entire_binding(),params.as_entire_binding()]);
            let bundle = draw_bundle(d, &g0s, &kinds, casters, &shadow_args, Some(&g1_sun));
            let (lo, hi) = caster_bounds(p, casters);
            let (view_proj, texel, dir) = sun_camera(p.sun, lo, hi, side);
            let vis_t = tex(d, "sun vis", (side, side), VIS, T::RENDER_ATTACHMENT);
            let reuse = if sun_pass > 0 && sun_pass + 1 < p.passes.len() && sun_passes.next().is_none()
                && !p.passes[..sun_pass].iter().any(|pass| matches!(pass, Pass::Shade(_)))
            {
                shadow::Reuse::prepare(p, casters, indexed, edit.map(|(kind, _)| kind))
            } else {
                shadow::Reuse::EveryFrame
            };
            Shadow { frame_buf, bundle, groups: (g0s, g1_sun), args: shadow_args, vis: view(&vis_t),
                depth: shadow_view.clone(), view_proj, texel, dir, pass: sun_pass, reuse: Mutex::new(reuse) }
        });

        let mut bake_cache = mips::Pipelines::default();
        let mut bake_textures = p.bakes.iter().map(|b| TextureBake::prepare(g, b, &modules, &bgl0, &mut bake_cache, &g0))
            .collect::<Result<Vec<_>, _>>()?;
        let load_stamps = bake_textures.iter().filter(|b| b.clock.due(true, &p.params))
            .map(TextureBake::timestamp_count).sum();
        if load_stamps != 0 {
            let clock = g.timestamps.then(|| Clock::new(g, load_stamps));
            let mut enc = d.create_command_encoder(&Default::default());
            let mut next_stamp = 0;
            let mut names = Vec::new();
            for bake in bake_textures.iter_mut().filter(|b| b.clock.due(true, &p.params)) {
                let stamp = clock.as_ref().map(|c| (&c.set, next_stamp));
                stats.load_bake_commands += bake.encode(&mut enc, stamp);
                next_stamp += bake.timestamp_count();
                bake.clock.complete(&p.params);
                names.push(format!("{} base", bake.name));
                if bake.mips.is_some() { names.push(format!("{} mips", bake.name)); }
            }
            g.queue.submit([enc.finish()]);
            if let Some(c) = clock {
                let times = read_clock(g, &c)?;
                let ticks = times[times.len()-1].checked_sub(times[0]).ok_or("unordered load-bake timestamps")?;
                stats.load_bake_gpu_ms = Some(ticks as f64 * c.period_ns / 1e6);
                stats.load_bake_pass_ms = names.into_iter().zip(times.chunks_exact(2)).map(|(name, pair)| {
                    let span = pair[1].checked_sub(pair[0]).ok_or("unordered bake pass timestamps")?;
                    Ok::<_, &str>((name, span as f64 * c.period_ns / 1e6))
                }).collect::<Result<_, _>>()?;
                stats.load_bake_ticks = times;
            }
        }
        let path = path_layout.as_ref().map(|layout| PathGpu::prepare(g, p, &bgl0, &bgl_present, layout, &modules[p.present], &g0, &present_bg)).transpose()?;
        let present_groups = [&g0, &present_bg].into_iter().chain(path.iter().map(|p| &p.group)).collect::<Vec<_>>();
        let present_bundle = command::Raster::prepare(d, &wgpu::RenderBundleEncoderDescriptor {
            color_formats: &[Some(color_format)], sample_count: 1, ..Default::default()
        }, &present_groups, [command::Draw {
            pipeline: &present_pl, groups: vec![],
            source: command::DrawSource::Direct { vertices: 0..3, instances: 0..1 },
        }]);
        let n = 2 * (p.passes.len() as u32 + u32::from(path.as_ref().is_some_and(PathGpu::has_photons)));
        let clock = g.timestamps.then(|| Clock::new(g, n));
        d.poll(wgpu::PollType::wait_indefinitely()).map_err(|e| e.to_string())?;
        stats.resources_ms = t.elapsed().as_secs_f64() * 1e3;
        let mut scene = Loaded {
            commands: Vec::new(),
            size,
            tiles,
            kinds,
            passes: p.passes.clone(),
            camera: p.camera,
            sun: p.sun,
            n_inst,
            frame_buf,
            g0,
            bgl0,
            bgl_kind,
            bgl_shade,
            instance_bounds: insts,
            draw_args,
            draw_reset,
            shade_args,
            shade_reset,
            cull: (cull_pl, cull_bg),
            classify: (classify_pl, classify_bg),
            present: present_bundle,
            bundle,
            shadow,
            vis,
            depth,
            color,
            color_view,
            clock,
            bake_textures: Mutex::new(bake_textures),
            params,
            param_values: Mutex::new((p.params.clone(), false)),
            overdraw,
            path,
            stats,
        };
        scene.prepare_passes();
        Ok(scene)
    }

    // The source schedule selects already prepared runner commands. The same
    // lowering reconnects the handles when an edit publishes new pipelines.
    fn prepare_passes(&mut self) {
        self.commands = self.passes.iter().map(|pass| match pass {
            Pass::Cull => self.direct(&self.cull, [self.n_inst.div_ceil(64), 1, 1]),
            Pass::Vis(_) => visibility(&self.bundle, &self.vis, &self.depth,
                wgpu::StoreOp::Store, wgpu::StoreOp::Discard),
            Pass::Shadow(_) => {
                let sun = self.shadow.as_ref().expect("validated caster pass");
                visibility(&sun.bundle, &sun.vis, &sun.depth, wgpu::StoreOp::Discard, wgpu::StoreOp::Store)
            }
            Pass::Classify => self.direct(&self.classify, [self.tiles.0, self.tiles.1, 1]),
            Pass::Shade(ids) => match &self.overdraw {
                Some((raster, hdr)) => color_pass(hdr, wgpu::Color::TRANSPARENT, raster),
                None => self.shading(ids),
            },
            Pass::Present => color_pass(&self.color_view, wgpu::Color::BLACK, &self.present),
        }).collect();
    }

    fn direct(&self, kernel: &(wgpu::ComputePipeline, wgpu::BindGroup), size: [u32; 3]) -> command::Pass {
        command::Pass::Compute(vec![Dispatch {
            pipeline: kernel.0.clone(), groups: vec![self.g0.clone(), kernel.1.clone()], work: Work::Direct(size),
        }])
    }

    fn shading(&self, ids: &[u32]) -> command::Pass {
        command::Pass::Compute(ids.iter().map(|&id| {
            let kind = &self.kinds[id as usize];
            Dispatch { pipeline: kind.shade.clone(), groups: vec![self.g0.clone(), kind.g1.clone(), kind.g2.clone()],
                work: Work::Indirect(self.shade_args.clone(), u64::from(id) * 12) }
        }).collect())
    }

    /// The camera's frame uniform at time t, or the sun's (`sun`: orthographic, `eye` holds the ray direction).
    fn frame_uniform(&self, t: f32, sun: bool, samples: u32) -> Vec<u8> {
        let mut bytes = frame_bytes(
            self.camera, self.sun, self.size, t, sun,
            self.shadow.as_ref().map(|x| (x.view_proj, x.texel, x.dir)),
        );
        // The public template uses zero samples. The native path fills this ABI word per frame.
        bytes[188..192].copy_from_slice(&samples.to_le_bytes());
        bytes
    }

    pub fn set_param_word(&self, word: usize, value: f32) -> Result<(), String> {
        let mut params = self.param_values.lock().unwrap();
        let slot = params.0.get_mut(word).ok_or("unknown parameter word")?;
        if slot.to_bits() != value.to_bits() {
            *slot = value;
            params.1 = true;
        }
        Ok(())
    }

    /// Texture contents are available after the queue finishes the load or first dirty bake.
    pub fn baked_texture(&self, name: &str) -> Option<wgpu::Texture> {
        self.bake_textures.lock().unwrap().iter().find(|b| b.name == name).map(|b| b.texture.clone())
    }

    /// Encodes and submits one frame at time `t`. Returns the GPU commands recorded (dispatches + draws).
    pub fn frame(&self, g: &Gpu, t: f32) -> u32 {
        self.frame_samples(g, t, 1)
    }

    pub fn frame_samples(&self, g: &Gpu, t: f32, samples: u32) -> u32 {
        self.submit_frame(g, t, samples, self.clock.as_ref()).0
    }

    fn submit_frame(&self, g: &Gpu, t: f32, samples: u32, clock: Option<&Clock>) -> (u32, wgpu::SubmissionIndex) {
        // Hold the submission lock across buffer writes, the reuse decision and queue submission.
        let mut params = self.param_values.lock().unwrap();
        g.queue.write_buffer(&self.frame_buf, 0, &self.frame_uniform(t, false, samples));
        let skip_shadow = self.shadow.as_ref().and_then(|sh| {
            let uniform = self.frame_uniform(t, true, samples);
            if sh.reuse.lock().unwrap().needs_draw(&uniform, params.1) {
                g.queue.write_buffer(&sh.frame_buf, 0, &uniform);
                None
            } else {
                Some(sh.pass)
            }
        });
        g.queue.write_buffer(&self.draw_args, 0, &self.draw_reset);
        g.queue.write_buffer(&self.shade_args, 0, &self.shade_reset);
        let mut enc = g.device.create_command_encoder(&Default::default());
        let mut commands = 0;
        if params.1 {
            g.queue.write_buffer(&self.params, 0, &f32_bytes(&params.0));
            params.1 = false;
        }
        for bake in self.bake_textures.lock().unwrap().iter_mut().filter(|b| b.clock.due(false, &params.0)) {
            commands += bake.encode(&mut enc, None);
            bake.clock.complete(&params.0);
        }
        let mut head = 0;
        if let Some((bins, photons)) = self.path.as_ref().and_then(|p| p.photons.as_ref()) {
            enc.clear_buffer(bins, 0, None);
            head = photons.record(&mut enc, clock.map(|c| c.stamps(0)));
            commands += head;
        }
        let skipped_pass = skip_shadow.map_or(u32::MAX, |i| head + i as u32);
        for (i, pass) in self.commands.iter().enumerate() {
            let stamp = head + i as u32;
            if stamp == skipped_pass { continue; }
            commands += pass.record(&mut enc, clock.map(|c| c.stamps(stamp - u32::from(stamp > skipped_pass))));
        }
        if let Some(clock) = clock {
            clock.skipped_pass.store(skipped_pass, Ordering::Relaxed);
        }
        (commands, g.queue.submit([enc.finish()]))
    }

    /// Profile a finite stream on this device's queue. Immediate reads use one
    /// slot. Deferred reads keep up to eight serials live with identical markers.
    pub fn profile_frames(
        &self,
        g: &Gpu,
        times: impl IntoIterator<Item = f32>,
        deferred: bool,
    ) -> Result<Vec<FrameTiming>, String> {
        use std::collections::VecDeque;
        use std::sync::{Arc, atomic::AtomicBool};

        let clock = self.clock.as_ref().ok_or("GPU timestamps are unavailable")?;
        let depth: usize = if deferred { 8 } else { 1 };
        let clocks = (0..depth).map(|_| Clock::new(g, clock.n)).collect::<Vec<_>>();
        type Submitted = (usize, u32, wgpu::SubmissionIndex, Arc<AtomicBool>);
        let mut submitted = VecDeque::<Submitted>::new();
        let mut resolving = VecDeque::<(usize, u32, ClockRead)>::new();
        let mut timings = Vec::new();
        let resolve_ready = |submitted: &mut VecDeque<Submitted>, resolving: &mut VecDeque<(usize, u32, ClockRead)>| {
            while submitted.front().is_some_and(|frame| frame.3.load(Ordering::Acquire)) {
                let (serial, commands, _, _) = submitted.pop_front().expect("completed frame");
                resolving.push_back((serial, commands, clocks[serial % depth].resolve_completed(g)));
            }
        };
        let collect_oldest = |resolving: &mut VecDeque<(usize, u32, ClockRead)>, timings: &mut Vec<_>| {
            let (serial, commands, read) = resolving.pop_front().ok_or("timestamp slot did not resolve")?;
            let clock = &clocks[serial % depth];
            let ticks = clock.collect(g, read)?;
            timings.push(clock.timing(serial, commands, ticks)?);
            Ok::<_, String>(())
        };
        for (serial, t) in times.into_iter().enumerate() {
            if submitted.len() + resolving.len() == depth {
                if resolving.is_empty() {
                    let (_, _, submission, _) = submitted.front().expect("full timestamp ring");
                    wait_for(g, Some(submission.clone()))?;
                    resolve_ready(&mut submitted, &mut resolving);
                }
                collect_oldest(&mut resolving, &mut timings)?;
            }
            let (commands, submission) = self.submit_frame(g, t, 1, Some(&clocks[serial % depth]));
            let complete = Arc::new(AtomicBool::new(false));
            let flag = Arc::clone(&complete);
            g.queue.on_submitted_work_done(move || flag.store(true, Ordering::Release));
            submitted.push_back((serial, commands, submission, complete));
            if deferred {
                g.device.poll(wgpu::PollType::Poll).map_err(|e| e.to_string())?;
                resolve_ready(&mut submitted, &mut resolving);
            }
        }
        if let Some((_, _, submission, _)) = submitted.back() {
            wait_for(g, Some(submission.clone()))?;
            resolve_ready(&mut submitted, &mut resolving);
        }
        if !submitted.is_empty() {
            return Err("frame completion callback did not run".into());
        }
        while !resolving.is_empty() {
            collect_oldest(&mut resolving, &mut timings)?;
        }
        Ok(timings)
    }

    /// After `frame`: waits, then the GPU time from the first pass's start to the last pass's end, in ms, and each
    /// pass's share of it in schedule order.
    /// The resolve goes in its own submit after the frame completes (19 Nov: resolving before Metal had written the
    /// last passes' end samples read the previous frame's values).
    pub fn gpu_ms(&self, g: &Gpu) -> Result<Option<(f64, Vec<f64>)>, String> {
        let Some(c) = &self.clock else { return Ok(None) };
        let timing = c.timing(0, 0, read_clock(g, c)?)?;
        Ok(Some((timing.gpu_ms, timing.pass_ms)))
    }

    /// The presented picture, tightly packed RGBA8 (sRGB), top row first.
    pub fn pixels(&self, g: &Gpu) -> Result<Vec<u8>, String> {
        let (w, h) = self.size;
        let row = (w * 4).next_multiple_of(256);
        let buf = buffer(&g.device, "readback", u64::from(row * h), wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST);
        let mut enc = g.device.create_command_encoder(&Default::default());
        enc.copy_texture_to_buffer(
            self.color.as_image_copy(),
            wgpu::TexelCopyBufferInfo { buffer: &buf, layout: wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(row), rows_per_image: Some(h) } },
            wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
        );
        g.queue.submit([enc.finish()]);
        buf.map_async(wgpu::MapMode::Read, .., |r| r.expect("readback map"));
        g.device.poll(wgpu::PollType::wait_indefinitely()).map_err(|e| e.to_string())?;
        let view = buf.get_mapped_range(..).map_err(|e| e.to_string())?;
        Ok(view.chunks_exact(row as usize).flat_map(|r| r[..(w * 4) as usize].to_vec()).collect())
    }

    /// The twin: runs kind `k`'s queries on the GPU at samples of 8 floats each (p.xyz or (u, v, 0), row as bits,
    /// dir.xyz, 0); returns 16 floats each (trace, visible, 0, 0 | material rgb, 0 | surface point, 0 | surface
    /// normal, 0). Built on demand; not part of a frame.
    pub fn twin(&self, g: &Gpu, k: usize, samples: &[f32]) -> Result<Vec<f32>, String> {
        let d = &g.device;
        let kg = &self.kinds[k];
        use wgpu::BufferUsages as U;
        let empty = layout(d, "empty", &[]);
        let bgl_twin = layout(d, "twin", &[(CS, storage(true)), (CS, storage(false))]);
        let pl = compute(d, "twin", &[&self.bgl0, &self.bgl_kind, &empty, &bgl_twin], &kg.module, "twin", None);
        let size = (samples.len() * 4) as u64;
        let input = buffer(d, "twin in", size, U::STORAGE | U::COPY_DST);
        g.queue.write_buffer(&input, 0, &f32_bytes(samples));
        let output = buffer(d, "twin out", 2 * size, U::STORAGE | U::COPY_SRC);
        let read = buffer(d, "twin read", 2 * size, U::MAP_READ | U::COPY_DST);
        let g3 = bind(d, &bgl_twin, &[input.as_entire_binding(), output.as_entire_binding()]);
        let g2 = bind(d, &empty, &[]);
        let mut enc = d.create_command_encoder(&Default::default());
        command::Pass::Compute(vec![Dispatch { pipeline: pl, groups: vec![self.g0.clone(),kg.g1.clone(),g2,g3],
            work: Work::Direct([((samples.len()/8) as u32).div_ceil(64),1,1]) }]).record(&mut enc,None);
        enc.copy_buffer_to_buffer(&output, 0, &read, 0, None);
        g.queue.submit([enc.finish()]);
        read.map_async(wgpu::MapMode::Read, .., |r| r.expect("twin map"));
        d.poll(wgpu::PollType::wait_indefinitely()).map_err(|e| e.to_string())?;
        let view = read.get_mapped_range(..).map_err(|e| e.to_string())?;
        let out = view.as_chunks::<4>().0.iter().map(|b| f32::from_le_bytes(*b)).collect();
        Ok(out)
    }
}

fn visibility(raster: &command::Raster, color: &wgpu::TextureView, depth: &wgpu::TextureView,
    store: wgpu::StoreOp, depth_store: wgpu::StoreOp) -> command::Pass {
    let clear = wgpu::Color { r: f64::from(abi::EMPTY), g: 0., b: 0., a: 0. };
    raster.attach(
        vec![command::Color { view: color.clone(), ops: wgpu::Operations { load: wgpu::LoadOp::Clear(clear), store } }],
        Some(command::Depth { view: depth.clone(), ops: Some(wgpu::Operations { load: wgpu::LoadOp::Clear(1.), store: depth_store }) }),
    )
}

fn color_pass(view: &wgpu::TextureView, clear: wgpu::Color, raster: &command::Raster) -> command::Pass {
    raster.attach(vec![command::Color { view: view.clone(),
        ops: wgpu::Operations { load: wgpu::LoadOp::Clear(clear), store: wgpu::StoreOp::Store } }], None)
}

pub fn frame_bytes(
    camera: loft_passes::program::Orbit, light: [f32; 3], size: (u32, u32),
    t: f32, sun: bool, light_view: Option<(M4, f32, [f32; 3])>,
) -> Vec<u8> {
    let eye = camera.eye(t);
    let aspect = size.0 as f32 / size.1 as f32;
    let vp = mul(perspective(camera.fov, aspect, 0.05, 400.0), look_at(eye, camera.at));
    let s = light;
    let l = (s[0] * s[0] + s[1] * s[1] + s[2] * s[2]).sqrt();
    let (light_vp, texel, dir) = light_view.unwrap_or(([[0.0; 4]; 4], 0.0, [0.0; 3]));
    let (vp, eye) = if sun { (light_vp, dir) } else { (vp, eye) };
    let inv = invert(vp);
    let mut w: Vec<f32> = vp.iter().flatten().copied().collect();
    w.extend(inv.iter().flatten());
    w.extend([eye[0], eye[1], eye[2], t, s[0] / l, s[1] / l, s[2] / l, texel]);
    let mut b = f32_bytes(&w);
    let cols = size.0.div_ceil(abi::TILE);
    let tiles = [size.0, size.1, cols, cols * size.1.div_ceil(abi::TILE)];
    b.extend(bytes_of(&[tiles[0], tiles[1], tiles[2], tiles[3], 0, u32::from(sun), u32::from(light_view.is_some()), 0]));
    b.extend(f32_bytes(&light_vp.iter().flatten().copied().collect::<Vec<_>>()));
    debug_assert_eq!(b.len() as u64, abi::FRAME_BYTES);
    b
}

type M4 = [[f32; 4]; 4];

/// The sun's orthographic camera around a box: (view-projection, texel size in metres, ray direction). Depth runs from
/// the box's sunward face to 100 m past it, so receivers below the casters fall inside it.
pub fn sun_camera(sun: [f32; 3], lo: [f32; 3], hi: [f32; 3], side: u32) -> (M4, f32, [f32; 3]) {
    let l = (sun[0] * sun[0] + sun[1] * sun[1] + sun[2] * sun[2]).sqrt();
    let s = sun.map(|x| x / l);
    let c = [0, 1, 2].map(|i| 0.5 * (lo[i] + hi[i]));
    let r = 0.5 * (0..3).map(|i| (hi[i] - lo[i]).powi(2)).sum::<f32>().sqrt() + 0.05;
    let view = look_at([0, 1, 2].map(|i| c[i] + s[i] * r), c);
    let proj = [[1.0 / r, 0.0, 0.0, 0.0], [0.0, 1.0 / r, 0.0, 0.0], [0.0, 0.0, -1.0 / (100.0 + r), 0.0], [0.0, 0.0, 0.0, 1.0]];
    (mul(proj, view), 2.0 * r / side as f32, s.map(|x| -x))
}

/// Column-major 4x4 product `a * b`.
fn mul(a: M4, b: M4) -> M4 {
    let mut r = [[0.0; 4]; 4];
    for (c, col) in r.iter_mut().enumerate() {
        for (row, v) in col.iter_mut().enumerate() {
            *v = (0..4).map(|k| a[k][row] * b[c][k]).sum();
        }
    }
    r
}

/// Right-handed perspective, depth 0..1.
fn perspective(fov: f32, aspect: f32, near: f32, far: f32) -> M4 {
    let h = 1.0 / (0.5 * fov).tan();
    [[h / aspect, 0.0, 0.0, 0.0], [0.0, h, 0.0, 0.0], [0.0, 0.0, far / (near - far), -1.0], [0.0, 0.0, near * far / (near - far), 0.0]]
}

fn look_at(eye: [f32; 3], at: [f32; 3]) -> M4 {
    let sub = |a: [f32; 3], b: [f32; 3]| [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
    let dot = |a: [f32; 3], b: [f32; 3]| a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
    let norm = |a: [f32; 3]| {
        let l = dot(a, a).sqrt();
        [a[0] / l, a[1] / l, a[2] / l]
    };
    let cross = |a: [f32; 3], b: [f32; 3]| [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]];
    let f = norm(sub(at, eye));
    let s = norm(cross(f, [0.0, 1.0, 0.0]));
    let u = cross(s, f);
    [[s[0], u[0], -f[0], 0.0], [s[1], u[1], -f[1], 0.0], [s[2], u[2], -f[2], 0.0], [-dot(s, eye), -dot(u, eye), dot(f, eye), 1.0]]
}

/// Gauss-Jordan in f64 with partial pivoting.
fn invert(m: M4) -> M4 {
    let mut a = [[0.0f64; 8]; 4];
    for r in 0..4 {
        for c in 0..4 {
            a[r][c] = f64::from(m[c][r]);
        }
        a[r][4 + r] = 1.0;
    }
    for c in 0..4 {
        let p = (c..4).max_by(|&x, &y| a[x][c].abs().total_cmp(&a[y][c].abs())).expect("rows");
        a.swap(c, p);
        let d = a[c][c];
        for v in &mut a[c] {
            *v /= d;
        }
        for r in 0..4 {
            if r != c {
                let f = a[r][c];
                let pivot = a[c];
                for (v, pv) in a[r].iter_mut().zip(pivot) {
                    *v -= f * pv;
                }
            }
        }
    }
    let mut out = [[0.0; 4]; 4];
    for (c, col) in out.iter_mut().enumerate() {
        for (r, v) in col.iter_mut().enumerate() {
            *v = a[r][4 + c] as f32;
        }
    }
    out
}
