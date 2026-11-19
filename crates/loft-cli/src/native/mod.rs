//! Native execution of the Phase 1.5 picture and motion fixtures.
mod motion;
mod edit;
pub use edit::TraceSession;
mod bakes;
mod mips;
mod paths;
use paths::PathGpu;
use bakes::TextureBake;
use loft_passes::emit::Aov;
use std::sync::Mutex;
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
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::HighPerformance,
        ..Default::default()
    }))
    .map_err(|e| format!("no adapter: {e}"))?;
    // PIPELINE_CACHE exists on Vulkan only: wgpu 30's Metal backend stubs `create_pipeline_cache` (it never uses
    // MTLBinaryArchive), so on Metal the only cache across processes is the OS's own Metal shader cache.
    let mut requested = wgpu::Features::TIMESTAMP_QUERY | wgpu::Features::TIMESTAMP_QUERY_INSIDE_ENCODERS | wgpu::Features::PIPELINE_CACHE | wgpu::Features::TEXTURE_ADAPTER_SPECIFIC_FORMAT_FEATURES;
    if ray_query { requested |= wgpu::Features::EXPERIMENTAL_RAY_QUERY; }
    let features = adapter.features() & requested;
    // Opt in only for callers that request the unstable native traversal feature.
    let experimental_features = if features.contains(wgpu::Features::EXPERIMENTAL_RAY_QUERY) {
        #[allow(unsafe_code)]
        unsafe { wgpu::ExperimentalFeatures::enabled() }
    } else {
        Default::default()
    };
    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        label: Some("loft-runner"),
        required_features: features,
        experimental_features,
        // The cube kernel covers six 8x8 faces. wgpu otherwise grants only the default 256 lanes.
        required_limits: wgpu::Limits {
            max_compute_invocations_per_workgroup: adapter.limits().max_compute_invocations_per_workgroup.min(384),
            ..Default::default()
        },
        ..Default::default()
    }))
    .map_err(|e| format!("no device: {e}"))?;
    let info = adapter.get_info();
    use wgpu::TextureFormat as F;
    let bake_formats = [F::R8Unorm, F::R16Float, F::R32Float, F::Rgba8Unorm, F::Rgba16Float].into_iter()
        .filter(|f| {
            let ff = if features.contains(wgpu::Features::TEXTURE_ADAPTER_SPECIFIC_FORMAT_FEATURES) {
                adapter.get_texture_format_features(*f)
            } else {
                f.guaranteed_format_features(features)
            };
            ff.allowed_usages.contains(wgpu::TextureUsages::STORAGE_BINDING)
                && ff.flags.contains(wgpu::TextureFormatFeatureFlags::STORAGE_WRITE_ONLY)
        }).collect();
    Ok(Gpu {
        bake_formats,
        device,
        queue,
        device_signature: format!("{} ({:?}) {} {}", info.name, info.backend, info.driver, info.driver_info),
        adapter: format!("{} ({:?})", info.name, info.backend),
        timestamps: features.contains(wgpu::Features::TIMESTAMP_QUERY),
        cache_key: features.contains(wgpu::Features::PIPELINE_CACHE).then(|| wgpu::util::pipeline_cache_key(&info)).flatten(),
    })
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
}

pub struct Loaded {
    size: (u32, u32),
    tiles: (u32, u32),
    kinds: Vec<KindGpu>,
    passes: Vec<Pass>,
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
    present: (wgpu::RenderPipeline, wgpu::BindGroup),
    bundle: wgpu::RenderBundle,
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
    overdraw: Option<(wgpu::RenderBundle, wgpu::TextureView)>,
    path: Option<PathGpu>,
    pub stats: LoadStats,
}

struct Shadow {
    groups: (wgpu::BindGroup, Vec<wgpu::BindGroup>),
    args: wgpu::Buffer,
    frame_buf: wgpu::Buffer,
    bundle: wgpu::RenderBundle,
    vis: wgpu::TextureView,
    depth: wgpu::TextureView,
    view_proj: [[f32; 4]; 4],
    texel: f32,
    dir: [f32; 3],
}

/// Timestamps at the start and end of every pass, resolved after the frame.
struct Clock {
    set: wgpu::QuerySet,
    resolve: wgpu::Buffer,
    read: wgpu::Buffer,
    n: u32,
    period_ns: f64,
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
    let layouts: Vec<Option<&wgpu::BindGroupLayout>> = layouts.iter().map(|l| Some(*l)).collect();
    let pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor { label: None, bind_group_layouts: &layouts, immediate_size: 0 });
    device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: Some(label),
        layout: Some(&pl),
        module,
        entry_point: Some(entry),
        compilation_options: Default::default(),
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
    fn new(g: &Gpu, n: u32) -> Self {
        use wgpu::BufferUsages as U;
        Self {
            set: g.device.create_query_set(&wgpu::QuerySetDescriptor {
                label: Some("GPU timer"), ty: wgpu::QueryType::Timestamp, count: n,
            }),
            resolve: buffer(&g.device, "ts resolve", u64::from(n) * 8, U::QUERY_RESOLVE | U::COPY_SRC),
            read: buffer(&g.device, "ts read", u64::from(n) * 8, U::COPY_DST | U::MAP_READ),
            n, period_ns: f64::from(g.queue.get_timestamp_period()),
        }
    }
}

fn read_clock(g: &Gpu, c: &Clock) -> Result<Vec<u64>, String> {
    let poll = || g.device.poll(wgpu::PollType::wait_indefinitely()).map(|_| ()).map_err(|e| e.to_string());
    poll()?;
    let mut enc = g.device.create_command_encoder(&Default::default());
    enc.resolve_query_set(&c.set, 0..c.n, &c.resolve, 0);
    enc.copy_buffer_to_buffer(&c.resolve, 0, &c.read, 0, None);
    g.queue.submit([enc.finish()]);
    c.read.map_async(wgpu::MapMode::Read, .., |r| r.expect("timestamp map"));
    poll()?;
    let ticks: Vec<u64> = {
        let view = c.read.get_mapped_range(..).map_err(|e| e.to_string())?;
        view.as_chunks::<8>().0.iter().map(|b| u64::from_le_bytes(*b)).collect()
    };
    c.read.unmap();
    Ok(ticks)
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
) -> wgpu::RenderBundle {
    let mut be = d.create_render_bundle_encoder(&wgpu::RenderBundleEncoderDescriptor {
        label: Some("visibility"),
        color_formats: &[Some(VIS)],
        depth_stencil: Some(wgpu::RenderBundleDepthStencil {
            format: DEPTH, depth_read_only: false, stencil_read_only: true,
        }),
        sample_count: 1, multiview: None,
    });
    be.set_bind_group(0, main, &[]);
    for &k in pass {
        let kg = &kinds[k as usize];
        be.set_pipeline(&kg.render);
        be.set_bind_group(1, overrides.map_or(&kg.g1, |groups| &groups[k as usize]), &[]);
        be.draw_indirect(args, u64::from(k) * 16);
    }
    be.finish(&wgpu::RenderBundleDescriptor { label: Some("visibility") })
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
        let bgl_kind = layout(d, "kind", &[(ALL, storage(true)), (ALL, storage(true)), (ALL, UNIFORM), (ALL, storage(true))]);
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
        let path_layout = p.path.as_ref().map(|path| PathGpu::layout(d, path));
        let present_layouts = [&bgl0, &bgl_present].into_iter().chain(path_layout.iter()).collect::<Vec<_>>();
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
                    let r = sc.spawn(move || timed(|| render(d, &kt.name, &[bgl0, bgl_kind], m, ("vs", "fs"), VIS, true, face, cache)));
                    let s = sc.spawn(move || timed(|| compute(d, &kt.name, &[bgl0, bgl_kind, bgl_shade], m, "shade", cache)));
                    (r, s)
                })
                .collect();
            let cull = sc.spawn(|| timed(|| compute(d, "cull", &[&bgl0, &bgl_cull], &modules[p.cull], "cull", cache)));
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
        let draw_reset = bytes_of(&p.kinds.iter().flat_map(|k| [k.draw.vertices(), 0, 0, 0]).collect::<Vec<_>>());
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
        let casters: Vec<u32> = p.passes.iter().find_map(|x| if let Pass::Shadow(k) = x { Some(k.clone()) } else { None }).unwrap_or_default();
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
        let shadow_args = init("shadow args", &bytes_of(&p.kinds.iter().flat_map(|k| [k.draw.vertices(), k.n_rows, 0, 0]).collect::<Vec<_>>()), U::INDIRECT);
        let mut kinds = vec![];
        let mut g1_sun = vec![];
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
            kinds.push(KindGpu { render, shade, module: modules[kt.module].clone(), rows, _params: params, g1, g2: g2.clone() });
        }
        let cull_bg = bind(d, &bgl_cull, &[insts.as_entire_binding(), draw_args.as_entire_binding(), visible.as_entire_binding(), bases_buf.as_entire_binding()]);
        let classify_bg =
            bind(d, &bgl_classify, &[wgpu::BindingResource::TextureView(&vis), shade_args.as_entire_binding(), tile_lists.as_entire_binding()]);
        let present_bg = bind(d, &bgl_present, &[wgpu::BindingResource::TextureView(&hdr), wgpu::BindingResource::TextureView(&vis)]);

        // The bundle: recorded once, replayed every frame. Per kind: set pipeline, bind, one drawIndirect.
        let vis_kinds: Vec<u32> = p.passes.iter().find_map(|x| if let Pass::Vis(k) = x { Some(k.clone()) } else { None }).unwrap_or_default();
        let bundle = draw_bundle(d, &g0, &kinds, &vis_kinds, &draw_args, None);
        let overdraw = (aov == Aov::Overdraw).then(|| {
            let mut be = d.create_render_bundle_encoder(&wgpu::RenderBundleEncoderDescriptor {
                label: Some("overdraw"), color_formats: &[Some(HDR)], depth_stencil: None,
                sample_count: 1, multiview: None,
            });
            let component = wgpu::BlendComponent { src_factor: wgpu::BlendFactor::One, dst_factor: wgpu::BlendFactor::One, operation: wgpu::BlendOperation::Add };
            let pipelines: Vec<_> = vis_kinds.iter().map(|&k| {
                let kt = &p.kinds[k as usize];
                let face = match kt.draw { Draw::Analytic | Draw::Traced => Some(wgpu::Face::Front), Draw::Surface { .. } => None };
                render_blended(d, &kt.name, &[&bgl0, &bgl_kind], &modules[kt.module], ("vs", "fs_overdraw"),
                    HDR, false, face, cache, Some(wgpu::BlendState { color: component, alpha: component }))
            }).collect();
            be.set_bind_group(0, &g0, &[]);
            for (&kind, pipeline) in vis_kinds.iter().zip(&pipelines) {
                be.set_pipeline(pipeline);
                be.set_bind_group(1, &kinds[kind as usize].g1, &[]);
                be.draw_indirect(&draw_args, u64::from(kind) * 16);
            }
            (be.finish(&wgpu::RenderBundleDescriptor { label: Some("overdraw") }), hdr)
        });
        let shadow = (p.shadow_size > 0).then(|| {
            let frame_buf = buffer(d, "sun frame", abi::FRAME_BYTES, U::UNIFORM | U::COPY_DST);
            let g0s = bind(d, &bgl0, &[frame_buf.as_entire_binding(),inputs.as_entire_binding(),params.as_entire_binding()]);
            let bundle = draw_bundle(d, &g0s, &kinds, &casters, &shadow_args, Some(&g1_sun));
            let (lo, hi) = caster_bounds(p, &casters);
            let (view_proj, texel, dir) = sun_camera(p.sun, lo, hi, side);
            let vis_t = tex(d, "sun vis", (side, side), VIS, T::RENDER_ATTACHMENT);
            Shadow { frame_buf, bundle, groups: (g0s, g1_sun), args: shadow_args, vis: view(&vis_t), depth: shadow_view.clone(), view_proj, texel, dir }
        });

        let mut bake_cache = mips::Pipelines::default();
        let mut bake_textures = p.bakes.iter().map(|b| TextureBake::prepare(g, b, &modules, &bgl0, &mut bake_cache))
            .collect::<Result<Vec<_>, _>>()?;
        let n_load = bake_textures.iter().filter(|b| b.clock.due(true, &p.params)).count() as u32;
        if n_load != 0 {
            let clock = g.timestamps.then(|| Clock::new(g, 2 * n_load));
            let mut enc = d.create_command_encoder(&Default::default());
            for (i, bake) in bake_textures.iter_mut().filter(|b| b.clock.due(true, &p.params)).enumerate() {
                let stamp = clock.as_ref().map(|c| (&c.set, i as u32 * 2));
                stats.load_bake_commands += bake.encode(&g0, &mut enc, stamp);
                bake.clock.complete(&p.params);
            }
            g.queue.submit([enc.finish()]);
            if let Some(c) = clock {
                let times = read_clock(g, &c)?;
                let ticks = times[times.len()-1].checked_sub(times[0]).ok_or("unordered load-bake timestamps")?;
                stats.load_bake_gpu_ms = Some(ticks as f64 * c.period_ns / 1e6);
            }
        }
        let path = path_layout.as_ref().map(|layout| PathGpu::prepare(g, p, &bgl0, &bgl_present, layout, &modules[p.present])).transpose()?;
        let n = 2 * (p.passes.len() as u32 + u32::from(path.as_ref().is_some_and(PathGpu::has_photons)));
        let clock = g.timestamps.then(|| Clock::new(g, n));
        d.poll(wgpu::PollType::wait_indefinitely()).map_err(|e| e.to_string())?;
        stats.resources_ms = t.elapsed().as_secs_f64() * 1e3;
        Ok(Loaded {
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
            present: (present_pl, present_bg),
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
        })
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
        g.queue.write_buffer(&self.frame_buf, 0, &self.frame_uniform(t, false, samples));
        if let Some(sh) = &self.shadow {
            g.queue.write_buffer(&sh.frame_buf, 0, &self.frame_uniform(t, true, samples));
        }
        g.queue.write_buffer(&self.draw_args, 0, &self.draw_reset);
        g.queue.write_buffer(&self.shade_args, 0, &self.shade_reset);
        let mut enc = g.device.create_command_encoder(&Default::default());
        let mut commands = 0;
        let mut params = self.param_values.lock().unwrap();
        if params.1 {
            g.queue.write_buffer(&self.params, 0, &f32_bytes(&params.0));
            params.1 = false;
        }
        for bake in self.bake_textures.lock().unwrap().iter_mut().filter(|b| b.clock.due(false, &params.0)) {
            commands += bake.encode(&self.g0, &mut enc, None);
            bake.clock.complete(&params.0);
        }
        let mut head = 0;
        if let Some(path) = &self.path {
            let timestamp_writes = self.clock.as_ref().map(|c| wgpu::ComputePassTimestampWrites {
                query_set: &c.set, beginning_of_pass_write_index: Some(0), end_of_pass_write_index: Some(1),
            });
            head = path.encode_photons(&mut enc, &self.g0, &self.present.1, timestamp_writes);
            commands += head;
        }
        for (i, pass) in self.passes.iter().enumerate() {
            let ts = |i: usize| self.clock.as_ref().map(|c| (&c.set, 2 * (head + i as u32)));
            let cts = ts(i).map(|(query_set, b)| wgpu::ComputePassTimestampWrites {
                query_set,
                beginning_of_pass_write_index: Some(b),
                end_of_pass_write_index: Some(b + 1),
            });
            let rts = ts(i).map(|(query_set, b)| wgpu::RenderPassTimestampWrites {
                query_set,
                beginning_of_pass_write_index: Some(b),
                end_of_pass_write_index: Some(b + 1),
            });
            match pass {
                Pass::Cull => {
                    let mut cp = enc.begin_compute_pass(&wgpu::ComputePassDescriptor { label: Some("cull"), timestamp_writes: cts });
                    cp.set_pipeline(&self.cull.0);
                    cp.set_bind_group(0, &self.g0, &[]);
                    cp.set_bind_group(1, &self.cull.1, &[]);
                    cp.dispatch_workgroups(self.n_inst.div_ceil(64), 1, 1);
                    commands += 1;
                }
                Pass::Vis(ks) => {
                    let mut rp = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                        label: Some("vis64"),
                        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                            view: &self.vis,
                            depth_slice: None,
                            resolve_target: None,
                            ops: wgpu::Operations {
                                load: wgpu::LoadOp::Clear(wgpu::Color { r: f64::from(abi::EMPTY), g: 0.0, b: 0.0, a: 0.0 }),
                                store: wgpu::StoreOp::Store,
                            },
                        })],
                        depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                            view: &self.depth,
                            depth_ops: Some(wgpu::Operations { load: wgpu::LoadOp::Clear(1.0), store: wgpu::StoreOp::Discard }),
                            stencil_ops: None,
                        }),
                        timestamp_writes: rts,
                        occlusion_query_set: None,
                        multiview_mask: None,
                    });
                    rp.execute_bundles([&self.bundle]);
                    commands += ks.len() as u32;
                }
                Pass::Shadow(ks) => {
                    let sh = self.shadow.as_ref().expect("a shadow pass has its targets");
                    let mut rp = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                        label: Some("sun"),
                        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                            view: &sh.vis,
                            depth_slice: None,
                            resolve_target: None,
                            ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color { r: f64::from(abi::EMPTY), g: 0.0, b: 0.0, a: 0.0 }), store: wgpu::StoreOp::Discard },
                        })],
                        depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                            view: &sh.depth,
                            depth_ops: Some(wgpu::Operations { load: wgpu::LoadOp::Clear(1.0), store: wgpu::StoreOp::Store }),
                            stencil_ops: None,
                        }),
                        timestamp_writes: rts,
                        occlusion_query_set: None,
                        multiview_mask: None,
                    });
                    rp.execute_bundles([&sh.bundle]);
                    commands += ks.len() as u32;
                }
                Pass::Classify => {
                    let mut cp = enc.begin_compute_pass(&wgpu::ComputePassDescriptor { label: Some("classify"), timestamp_writes: cts });
                    cp.set_pipeline(&self.classify.0);
                    cp.set_bind_group(0, &self.g0, &[]);
                    cp.set_bind_group(1, &self.classify.1, &[]);
                    cp.dispatch_workgroups(self.tiles.0, self.tiles.1, 1);
                    commands += 1;
                }
                Pass::Shade(ks) => {
                    if let Some((bundle, hdr)) = &self.overdraw {
                        let mut rp = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                            label: Some("overdraw"),
                            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                                view: hdr, depth_slice: None, resolve_target: None,
                                ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT), store: wgpu::StoreOp::Store },
                            })],
                            depth_stencil_attachment: None, timestamp_writes: rts, occlusion_query_set: None, multiview_mask: None,
                        });
                        rp.execute_bundles([bundle]);
                        commands += ks.len() as u32;
                        continue;
                    }
                    let mut cp = enc.begin_compute_pass(&wgpu::ComputePassDescriptor { label: Some("shade"), timestamp_writes: cts });
                    cp.set_bind_group(0, &self.g0, &[]);
                    for &k in ks {
                        let kg = &self.kinds[k as usize];
                        cp.set_pipeline(&kg.shade);
                        cp.set_bind_group(1, &kg.g1, &[]);
                        cp.set_bind_group(2, &kg.g2, &[]);
                        cp.dispatch_workgroups_indirect(&self.shade_args, u64::from(k) * 12);
                        commands += 1;
                    }
                }
                Pass::Present => {
                    let mut rp = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                        label: Some("present"),
                        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                            view: &self.color_view,
                            depth_slice: None,
                            resolve_target: None,
                            ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color::BLACK), store: wgpu::StoreOp::Store },
                        })],
                        depth_stencil_attachment: None,
                        timestamp_writes: rts,
                        occlusion_query_set: None,
                        multiview_mask: None,
                    });
                    rp.set_pipeline(&self.present.0);
                    rp.set_bind_group(0, &self.g0, &[]);
                    rp.set_bind_group(1, &self.present.1, &[]);
                    if let Some(path) = &self.path { rp.set_bind_group(2, &path.group, &[]); }
                    rp.draw(0..3, 0..1);
                    commands += 1;
                }
            }
        }
        g.queue.submit([enc.finish()]);
        commands
    }

    /// After `frame`: waits, then the GPU time from the first pass's start to the last pass's end, in ms, and each
    /// pass's share of it in schedule order.
    /// The resolve goes in its own submit after the frame completes (19 Nov: resolving before Metal had written the
    /// last passes' end samples read the previous frame's values).
    pub fn gpu_ms(&self, g: &Gpu) -> Result<Option<(f64, Vec<f64>)>, String> {
        let Some(c) = &self.clock else { return Ok(None) };
        let ticks = read_clock(g, c)?;
        let (first, last) = (ticks[0], ticks[c.n as usize - 1]);
        if first == 0 || last <= first {
            return Err(format!("timestamps unusable: first {first}, last {last}"));
        }
        let ms = |a: u64, b: u64| b.saturating_sub(a) as f64 * c.period_ns / 1e6;
        // A pass's cost is its end minus the previous pass's end: passes overlap on the GPU, so begin stamps of
        // later passes come early (a present pass "took" 4 ms of a 5 ms frame), while ends follow the dependencies.
        let ends: Vec<u64> = std::iter::once(first).chain(ticks.chunks(2).map(|p| p[1])).collect();
        Ok(Some((ms(first, last), ends.windows(2).map(|w| ms(w[0], w[1])).collect())))
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
        {
            let mut cp = enc.begin_compute_pass(&wgpu::ComputePassDescriptor { label: Some("twin"), timestamp_writes: None });
            cp.set_pipeline(&pl);
            cp.set_bind_group(0, &self.g0, &[]);
            cp.set_bind_group(1, &kg.g1, &[]);
            cp.set_bind_group(2, &g2, &[]);
            cp.set_bind_group(3, &g3, &[]);
            cp.dispatch_workgroups(((samples.len() / 8) as u32).div_ceil(64), 1, 1);
        }
        enc.copy_buffer_to_buffer(&output, 0, &read, 0, None);
        g.queue.submit([enc.finish()]);
        read.map_async(wgpu::MapMode::Read, .., |r| r.expect("twin map"));
        d.poll(wgpu::PollType::wait_indefinitely()).map_err(|e| e.to_string())?;
        let view = read.get_mapped_range(..).map_err(|e| e.to_string())?;
        let out = view.as_chunks::<4>().0.iter().map(|b| f32::from_le_bytes(*b)).collect();
        Ok(out)
    }
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
