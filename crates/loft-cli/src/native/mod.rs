//! Native execution of the Phase 1.5 picture and motion fixtures.
mod motion;
mod edit;
pub use edit::TraceSession;
use loft_runner::command::{self, Dispatch, Work};
use loft_passes::emit::Aov;
use std::sync::Mutex;
pub mod ray_query;
pub use motion::{sample_motion, sample_motion_timed};

use loft_passes::program::{abi, Pass, Program};
use loft_ir::{HostCaps, GpuFeatures, DepthConvention, InstanceId, ModuleId, TableId, KindId, ParamId, ParamMeta, IrTy};
use loft_runner::{Runner, frame::FrameOptions};
use std::sync::Arc;
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
}

/// A high-performance device; timestamps when the adapter grants them.
pub fn gpu() -> Result<Gpu, String> {
    request_gpu(false)
}

pub fn request_gpu(ray_query: bool) -> Result<Gpu, String> {
    ray_query::request_gpu(ray_query)
}

pub const COLOR: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb;
const DEPTH: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

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

pub struct Loaded {
    size: (u32, u32),
    camera: loft_passes::program::Orbit,
    sun: [f32; 3],
    light_view: Option<(M4, f32, [f32; 3])>,
    frame_table: TableId,
    runner: Mutex<Runner>,
    id: InstanceId,
    modules: Vec<ModuleId>,
    kind_tables: Vec<TableId>,
    twin_sources: Vec<naga::Module>,
    luts: Vec<f32>,
    kind_words: Vec<[u32; 4]>,
    params: Mutex<Vec<f32>>,
    instance_bounds: wgpu::Buffer,
    color: wgpu::Texture,
    color_view: wgpu::TextureView,
    clock: Option<Clock>,
    pub stats: LoadStats,
}

/// Timestamps at the start and end of every pass, resolved after the frame.
struct Clock {
    set: wgpu::QuerySet,
    resolve: wgpu::Buffer,
    read: wgpu::Buffer,
    n: u32,
    period_ns: f64,
}

/// One submitted frame's GPU timestamps in pass order.
#[derive(serde::Serialize)]
pub struct FrameTiming {
    pub serial: usize,
    pub commands: u32,
    pub gpu_ms: f64,
    pub pass_ms: Vec<f64>,
    pub ticks: Vec<u64>,
}

struct ClockRead {
    submission: wgpu::SubmissionIndex,
    mapped: std::sync::mpsc::Receiver<Result<(), wgpu::BufferAsyncError>>,
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
const ALL: wgpu::ShaderStages = wgpu::ShaderStages::VERTEX_FRAGMENT.union(wgpu::ShaderStages::COMPUTE);
const CS: wgpu::ShaderStages = wgpu::ShaderStages::COMPUTE;

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

    // Metal exposes stale end samples if the resolve is encoded before the render
    // submission completes. Each slot keeps its own query set until after unmap.
    fn resolve_completed(&self, g: &Gpu) -> ClockRead {
        let mut enc = g.device.create_command_encoder(&Default::default());
        enc.resolve_query_set(&self.set, 0..self.n, &self.resolve, 0);
        enc.copy_buffer_to_buffer(&self.resolve, 0, &self.read, 0, None);
        let submission = g.queue.submit([enc.finish()]);
        let (tx, mapped) = std::sync::mpsc::channel();
        self.read.map_async(wgpu::MapMode::Read, .., move |result| { let _ = tx.send(result); });
        ClockRead { submission, mapped }
    }

    fn collect(&self, g: &Gpu, read: ClockRead) -> Result<Vec<u64>, String> {
        wait_for(g, Some(read.submission))?;
        read.mapped.try_recv().map_err(|e| format!("timestamp callback: {e}"))?
            .map_err(|e| format!("timestamp map: {e}"))?;
        let ticks = {
            let view = self.read.get_mapped_range(..).map_err(|e| e.to_string())?;
            view.as_chunks::<8>().0.iter().map(|b| u64::from_le_bytes(*b)).collect()
        };
        self.read.unmap();
        Ok(ticks)
    }

    fn timing(&self, serial: usize, commands: u32, ticks: Vec<u64>) -> Result<FrameTiming, String> {
        let (first, last) = (ticks[0], ticks[self.n as usize - 1]);
        if first == 0 || last <= first || ticks.contains(&0) {
            return Err(format!("frame {serial}: unusable timestamps {ticks:?}"));
        }
        let ms = |a: u64, b: u64| b.saturating_sub(a) as f64 * self.period_ns / 1e6;
        // Pass starts can overlap on Metal. Use consecutive ends to attribute
        // costs, and preserve the original pairs so every span can be inspected.
        let ends = ticks.chunks_exact(2).map(|pair| pair[1]);
        let pass_ms = std::iter::once(first).chain(ends.clone()).zip(ends)
            .map(|(a, b)| ms(a, b)).collect();
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

fn runner_error(err: loft_runner::RunnerError) -> String { format!("{err:?}") }

impl Loaded {
    pub fn new(g: &Gpu, p: &Program, size: (u32, u32), nonce: Option<u32>) -> Result<Self, String> {
        Self::with_aov(g, p, size, nonce, Aov::Beauty)
    }

    pub fn with_aov(g: &Gpu, p: &Program, size: (u32, u32), nonce: Option<u32>, aov: Aov) -> Result<Self, String> {
        Self::load(g, p, size, nonce, None, aov)
    }

    fn load(g: &Gpu, p: &Program, size: (u32, u32), nonce: Option<u32>,
        edit: Option<(usize, &loft_passes::tier0::TraceEdit)>, aov: Aov) -> Result<Self, String> {
        let started = Instant::now();
        let d = &g.device;
        if size.0 == 0 || size.1 == 0 || size.0.max(size.1) > d.limits().max_texture_dimension_2d {
            return Err("frame dimensions exceed device limits".into());
        }
        let edit = edit.map(|(i, tape)| Ok::<_, String>((i, tape.shader(&p.modules[p.kinds[i].module].wgsl)?, tape.bytes())))
            .transpose()?;
        let params = p.params.iter().enumerate().map(|(word, &value)| ParamMeta {
            id: ParamId(word as u32), name: word.to_string(), ty: IrTy::F32, offset_bytes: word as u32 * 4,
            default: [value, 0., 0., 0.], range: None,
        }).collect();
        let frame = |sun, alternate: Option<([f32; 3], [f32; 3], u32)>| frame_bytes(
            p.camera, p.sun, size, 0., sun, alternate.map(|(lo, hi, side)| sun_camera(p.sun, lo, hi, side)));
        let plan = loft_passes::bundle::lower_program(p, params, size, loft_passes::bundle::Options {
            nonce, overdraw: aov == Aov::Overdraw, picture_only: true,
            edit: edit.as_ref().map(|(i, source, bytes)| (*i, source.as_str(), bytes.as_slice())),
        }, frame)?;
        let color_format = if aov == Aov::Beauty { COLOR } else { wgpu::TextureFormat::Rgba8Unorm };
        let color = tex(d, "color", size, color_format, wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC);
        let color_view = color.create_view(&Default::default());
        let caps = HostCaps { depth_convention: DepthConvention::StandardZeroToOne,
            msaa_samples: 1, features: GpuFeatures::empty() };
        let mut runner = Runner::attach(Arc::new(d.clone()), Arc::new(g.queue.clone()), caps);
        let layouts_start = Instant::now();
        let scope = d.push_error_scope(wgpu::ErrorFilter::Validation);
        let prepared = runner.load_bundle(&plan.bundle).and_then(|id| {
            runner.prepare_frame(id, &color_view)?;
            Ok(id)
        });
        if let Some(err) = pollster::block_on(scope.pop()) { return Err(err.to_string()); }
        let id = prepared.map_err(runner_error)?;
        d.poll(wgpu::PollType::wait_indefinitely()).map_err(|e| e.to_string())?;
        let mut stats = LoadStats { modules: plan.bundle.modules.len(),
            render_pipelines: p.kinds.len() + 1, compute_pipelines: p.kinds.len() + 2,
            modules_ms: (layouts_start - started).as_secs_f64() * 1e3,
            pipelines_ms: layouts_start.elapsed().as_secs_f64() * 1e3, ..Default::default() };
        stats.compute_pipelines += p.kinds.iter().filter(|kind| aov != Aov::Overdraw &&
            p.modules[kind.module].naga.entry_points.iter().any(|e| e.name == "bake_surface")).count();

        let stages = runner.load_stage_count(id).map_err(runner_error)?;
        if stages != 0 {
            let clock = g.timestamps.then(|| Clock::new(g, stages * 2));
            let mut enc = d.create_command_encoder(&Default::default());
            let report = runner.encode_load(id, &mut enc, clock.as_ref().map(|c| &c.set)).map_err(runner_error)?;
            g.queue.submit([enc.finish()]);
            if !p.bakes.is_empty() {
                stats.load_bake_commands = report.commands;
                if let Some(c) = clock {
                    let ticks = read_clock(g, &c)?;
                    let timing = c.timing(0, report.commands, ticks)?;
                    stats.load_bake_gpu_ms = Some(timing.gpu_ms);
                    stats.load_bake_pass_ms = report.stages.into_iter().zip(timing.pass_ms).collect();
                    stats.load_bake_ticks = timing.ticks;
                }
            }
        }
        let photons = p.path.as_ref().is_some_and(|p| matches!(p.integrator, loft_passes::emit::paths::Integrator::Spectral));
        let clock = g.timestamps.then(|| Clock::new(g, 2 * (p.passes.len() as u32 + u32::from(photons))));
        let kind_tables = (0..p.kinds.len()).map(|i| plan.bundle.rows[&KindId(i as u32)]).collect();
        let mut base = 0;
        let kind_words = p.kinds.iter().enumerate().map(|(i, k)| {
            let words = [i as u32, base, k.n_rows, 0];
            base += k.n_rows;
            words
        }).collect();
        let frame_table = plan.bundle.resources.frame.as_ref().ok_or("missing frame declaration")?.main;
        let instance_bounds = runner.export_buffer(id, plan.instances).ok_or("missing bounds table")?.as_ref().clone();
        Ok(Self { size, camera: p.camera, sun: p.sun, light_view: plan.shadow.map(|(lo, hi, side)| sun_camera(p.sun, lo, hi, side)),
            frame_table, runner: Mutex::new(runner), id, modules: plan.modules, kind_tables,
            twin_sources: p.kinds.iter().map(|k| p.modules[k.module].naga.clone()).collect(),
            luts: p.luts.clone(), kind_words, params: Mutex::new(p.params.clone()), instance_bounds,
            color, color_view, clock, stats })
    }

    pub fn set_param_word(&self, word: usize, value: f32) -> Result<(), String> {
        self.runner.lock().unwrap().set_param(self.id, ParamId(word.try_into().map_err(|_| "unknown parameter word")?), &[value])
            .map_err(runner_error)?;
        self.params.lock().unwrap()[word] = value;
        Ok(())
    }

    pub fn baked_texture(&self, name: &str) -> Option<wgpu::Texture> {
        self.runner.lock().unwrap().image(self.id, name).map(|t| t.as_ref().clone())
    }

    pub fn frame(&self, g: &Gpu, t: f32) -> u32 { self.frame_samples(g, t, 1) }

    pub fn frame_samples(&self, g: &Gpu, t: f32, samples: u32) -> u32 {
        self.submit_frame(g, t, samples, self.clock.as_ref()).0
    }

    fn submit_frame(&self, g: &Gpu, t: f32, samples: u32, clock: Option<&Clock>) -> (u32, wgpu::SubmissionIndex) {
        let uniform = |table| {
            let mut bytes = frame_bytes(self.camera, self.sun, self.size, t, table != self.frame_table, self.light_view);
            bytes[188..192].copy_from_slice(&samples.to_le_bytes());
            bytes
        };
        let mut runner = self.runner.lock().unwrap();
        let mut enc = g.device.create_command_encoder(&Default::default());
        let options = FrameOptions { uniform: Some(&uniform), timestamps: clock.map(|c| &c.set) };
        let report = runner.encode_frame(self.id, &mut enc, &self.color_view, t.into(), options).expect("prepared frame");
        (report.commands, g.queue.submit([enc.finish()]))
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
        use std::sync::{Arc, atomic::{AtomicBool, Ordering}};

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
        use wgpu::BufferUsages as U;
        let init = |bytes: &[u8], usage: U| {
            let b = buffer(d, "twin", bytes.len() as u64, usage | U::COPY_DST);
            if !bytes.is_empty() { g.queue.write_buffer(&b, 0, bytes); }
            b
        };
        let rows = self.rows(k)?;
        let luts = init(&f32_bytes(&self.luts), U::STORAGE);
        let count = init(&bytes_of(&self.kind_words[k]), U::UNIFORM);
        let frame = init(&frame_bytes(self.camera, self.sun, self.size, 0., false, self.light_view), U::UNIFORM);
        let params = init(&f32_bytes(&self.params.lock().unwrap()), U::STORAGE);
        let empty_buf = buffer(d, "twin empty", 16, U::STORAGE);
        let fixed = layout(d, "frame", &[(ALL, UNIFORM), (ALL, storage(true)), (ALL, storage(true))]);
        let kind = layout(d, "kind", &[(ALL, storage(true)), (wgpu::ShaderStages::VERTEX, storage(true)),
            (ALL, UNIFORM), (ALL, storage(true))]);
        let empty = layout(d, "empty", &[]);
        let twin = layout(d, "twin", &[(CS, storage(true)), (CS, storage(false))]);
        let shader = d.create_shader_module(wgpu::ShaderModuleDescriptor { label: Some("twin"),
            source: wgpu::ShaderSource::Naga(Cow::Owned(self.twin_sources[k].clone())) });
        let pipe = compute(d, "twin", &[&fixed, &kind, &empty, &twin], &shader, "twin", None);
        let size = (samples.len() * 4) as u64;
        let input = init(&f32_bytes(samples), U::STORAGE);
        let output = buffer(d, "twin out", 2 * size, U::STORAGE | U::COPY_SRC);
        let read = buffer(d, "twin read", 2 * size, U::MAP_READ | U::COPY_DST);
        let groups = vec![
            bind(d, &fixed, &[frame.as_entire_binding(), empty_buf.as_entire_binding(), params.as_entire_binding()]),
            bind(d, &kind, &[rows.as_entire_binding(), empty_buf.as_entire_binding(), count.as_entire_binding(), luts.as_entire_binding()]),
            bind(d, &empty, &[]), bind(d, &twin, &[input.as_entire_binding(), output.as_entire_binding()]),
        ];
        let mut enc = d.create_command_encoder(&Default::default());
        command::Pass::Compute(vec![Dispatch { pipeline: pipe, groups,
            work: Work::Direct([((samples.len()/8) as u32).div_ceil(64),1,1]) }]).record(&mut enc,None);
        enc.copy_buffer_to_buffer(&output, 0, &read, 0, None);
        g.queue.submit([enc.finish()]);
        read.map_async(wgpu::MapMode::Read, .., |r| r.expect("twin map"));
        wait_for(g, None)?;
        let view = read.get_mapped_range(..).map_err(|e| e.to_string())?;
        Ok(view.as_chunks::<4>().0.iter().copied().map(f32::from_le_bytes).collect())
    }

    fn rows(&self, k: usize) -> Result<Arc<wgpu::Buffer>, String> {
        self.runner.lock().unwrap().export_buffer(self.id, self.kind_tables[k]).ok_or("missing rows table".into())
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
