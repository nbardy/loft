//! Native execution of the Phase 1.5 picture and motion fixtures.
mod motion;
mod edit;
mod shadow;
pub use edit::TraceSession;
use loft_passes::emit::Aov;
use std::sync::{Mutex, atomic::{AtomicU32, Ordering}};
pub mod ray_query;
pub use motion::{sample_motion, sample_motion_timed};

use loft_passes::program::{abi, Pass, Program};
use loft_ir::{HostCaps, GpuFeatures, DepthConvention, InstanceId, ModuleId, TableId, KindId, ParamId, ParamMeta, IrTy};
use loft_runner::{Runner, frame::FrameOptions};
use std::sync::Arc;
use std::time::Instant;

pub struct Gpu {
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub adapter: String,
    raw_adapter: wgpu::Adapter,
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
    params: Mutex<(Vec<f32>, bool)>,
    shadow: Option<(u16, u32, Mutex<shadow::Reuse>)>,
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

fn param_words(words: &[f32]) -> Vec<ParamMeta> {
    words.iter().enumerate().map(|(word, &value)| ParamMeta {
        id: ParamId(word as u32), name: word.to_string(), ty: IrTy::F32, offset_bytes: word as u32 * 4,
        default: [value, 0., 0., 0.], range: None,
    }).collect()
}

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
        use loft_ir::{Bundle, IndirectDraw, ModuleBlob, PassDecl, PassId, PassRun, QueueClass, ShaderTarget, TableDecl,
            TableLifetime, TargetDesc, ElemFormat};
        let source = b"@vertex fn vs(@builtin(vertex_index) i: u32) -> @builtin(position) vec4f {
            let p = array(vec2(-1., -1.), vec2(3., -1.), vec2(-1., 3.)); return vec4(p[i], 0., 1.); }
            @fragment fn fs() -> @location(0) vec4f { return vec4(0.25, 0.5, 0.75, 1.); }";
        let bundle = Bundle {
            modules: vec![ModuleBlob { id: ModuleId(0), target: ShaderTarget::Wgsl,
                hash: *blake3::hash(source).as_bytes(), bytes: source.as_slice().into() }],
            tables: vec![TableDecl { id: TableId(0), format: ElemFormat::U32, lifetime: TableLifetime::Static,
                stride_bytes: 4, capacity: 4, initial: bytes_of(&[3, 1, 0, 0]).into() }],
            passes: vec![PassDecl { id: PassId(0), wave: 0, queue: QueueClass::Graphics, bindings: Box::new([]),
                run: PassRun::RenderBundle { draws: Box::new([IndirectDraw { module: ModuleId(0),
                    vertex_entry: "vs".into(), fragment_entry: "fs".into(), args: TableId(0), offset_bytes: 0, vertex: None, index: None }]),
                    color_targets: Box::new([TargetDesc { table: TableId(1) }]), depth: None, msaa: 1 } }],
            ..Default::default()
        };
        let mut runner = attach_runner(self);
        let id = runner.load_bundle(&bundle).map_err(runner_error)?;
        runner.prepare_frame(id, &view).map_err(runner_error)?;
        let c = Clock::new(self, 2);
        let mut times = [0.; 11];
        for ms in &mut times {
            let mut encoder = d.create_command_encoder(&Default::default());
            runner.encode_frame(id, &mut encoder, &view, 0., FrameOptions {
                timestamps: Some(&c.set), ..Default::default()
            }).map_err(runner_error)?;
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

fn attach_runner(g: &Gpu) -> Runner {
    Runner::attach(Arc::new(g.device.clone()), Arc::new(g.queue.clone()), HostCaps {
        depth_convention: DepthConvention::StandardZeroToOne, msaa_samples: 1, features: GpuFeatures::empty(),
    }).with_pipeline_clock(|| {
        static START: std::sync::OnceLock<Instant> = std::sync::OnceLock::new();
        START.get_or_init(Instant::now).elapsed()
    })
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
        let editable = edit.map(|(kind, _)| kind);
        let edit = edit.map(|(i, tape)| Ok::<_, String>((i, tape.shader(&p.modules[p.kinds[i].module].wgsl)?, tape.bytes())))
            .transpose()?;
        let params = param_words(&p.params);
        let frame = |sun, alternate: Option<([f32; 3], [f32; 3], u32)>| frame_bytes(
            p.camera, p.sun, size, 0., sun, alternate.map(|(lo, hi, side)| sun_camera(p.sun, lo, hi, side)));
        let plan = loft_passes::bundle::lower_program(p, params, size, loft_passes::bundle::Options {
            nonce, overdraw: aov == Aov::Overdraw, picture_only: true,
            edit: edit.as_ref().map(|(i, source, bytes)| (*i, source.as_str(), bytes.as_slice())),
        }, frame)?;
        let color_format = if aov == Aov::Beauty { COLOR } else { wgpu::TextureFormat::Rgba8Unorm };
        let color = tex(d, "color", size, color_format, wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC);
        let color_view = color.create_view(&Default::default());
        let cache_path = g.cache_key.as_ref().map(|key| std::path::Path::new("out/meta/pipeline_cache")
            .join(key).with_extension(format!("{}.bin", nonce.unwrap_or(0))));
        let cache_data = cache_path.as_ref().and_then(|path| std::fs::read(path).ok());
        // The cache key identifies the adapter and driver, and wgpu checks the header again.
        let cache = cache_path.as_ref().map(|_| {
            #[allow(unsafe_code)]
            unsafe {
                d.create_pipeline_cache(&wgpu::PipelineCacheDescriptor {
                    label: Some("loft"), data: cache_data.as_deref(), fallback: true,
                })
            }
        });
        let mut runner = attach_runner(g).with_adapter(g.raw_adapter.clone(), cache.clone());
        let layouts_start = Instant::now();
        let scope = d.push_error_scope(wgpu::ErrorFilter::Validation);
        let prepared = runner.load_bundle_for_frame(&plan.bundle, &color_view, None);
        if let Some(err) = pollster::block_on(scope.pop()) { return Err(err.to_string()); }
        let id = prepared.map_err(runner_error)?;
        d.poll(wgpu::PollType::wait_indefinitely()).map_err(|e| e.to_string())?;
        let creates = runner.pipeline_creates(id).map_err(runner_error)?;
        let mut stats = LoadStats { modules: plan.bundle.modules.len(),
            render_pipelines: creates.render.len(), compute_pipelines: creates.compute.len(),
            modules_ms: (layouts_start - started).as_secs_f64() * 1e3,
            pipelines_ms: layouts_start.elapsed().as_secs_f64() * 1e3,
            cache_bytes: cache_data.as_ref().map_or(0, Vec::len),
            pipeline_ms: creates.render.iter().chain(&creates.compute).cloned().collect(), ..Default::default() };
        if let (Some(path), Some(data)) = (cache_path, cache.and_then(|cache| cache.get_data())) {
            let parent = path.parent().expect("cache directory");
            if std::fs::create_dir_all(parent).is_ok()
                && let Ok(mut tmp) = tempfile::NamedTempFile::new_in(parent) {
                use std::io::Write;
                if tmp.write_all(&data).is_ok() { let _ = tmp.persist(path); }
            }
        }

        let resource_started = Instant::now();
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
        let shadow = plan.shadow_wave.and_then(|wave| {
            let [(i, casters)] = p.passes.iter().enumerate().filter_map(|(i, pass)| match pass {
                Pass::Shadow(casters) => Some((i, casters)),
                _ => None,
            }).collect::<Vec<_>>()[..] else { return None };
            (i > 0 && i + 1 < p.passes.len() && !p.passes[..i].iter().any(|pass| matches!(pass, Pass::Shade(_))))
                .then(|| {
                    let indexed = |kind: &loft_passes::program::KindTable| aov != Aov::Overdraw
                        && p.modules[kind.module].naga.entry_points.iter().any(|e| e.name == "vs_indexed");
                    (wave, i as u32 + u32::from(photons), Mutex::new(shadow::Reuse::prepare(p, casters, indexed, editable)))
                })
        });
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
        stats.resources_ms = resource_started.elapsed().as_secs_f64() * 1e3;
        Ok(Self { size, camera: p.camera, sun: p.sun, light_view: plan.shadow.map(|(lo, hi, side)| sun_camera(p.sun, lo, hi, side)),
            frame_table, runner: Mutex::new(runner), id, modules: plan.modules, kind_tables,
            twin_sources: p.kinds.iter().map(|k| p.modules[k.module].naga.clone()).collect(),
            luts: p.luts.clone(), kind_words, params: Mutex::new((p.params.clone(), false)), instance_bounds, shadow,
            color, color_view, clock, stats })
    }

    pub fn set_param_word(&self, word: usize, value: f32) -> Result<(), String> {
        let mut params = self.params.lock().unwrap();
        let slot = params.0.get_mut(word).ok_or("unknown parameter word")?;
        if slot.to_bits() != value.to_bits() {
            self.runner.lock().unwrap().set_param(self.id, ParamId(word as u32), &[value]).map_err(runner_error)?;
            *slot = value;
            params.1 = true;
        }
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
        // Hold the state and runner locks until submission. A caller-owned encoder
        // records all waves because this host cannot observe when it is submitted.
        let mut params = self.params.lock().unwrap();
        let mut runner = self.runner.lock().unwrap();
        let skip = self.shadow.as_ref().filter(|(_, _, reuse)| {
            !reuse.lock().unwrap().needs_draw(&self.uniform(t, samples, true), params.1)
        });
        let mut enc = g.device.create_command_encoder(&Default::default());
        let options = FrameOptions {
            timestamps: clock.map(|c| &c.set), omit_wave: skip.map(|x| x.0), ..Default::default()
        };
        let commands = self.record(&mut runner, &mut enc, &self.color_view, t, samples, options)
            .expect("prepared frame");
        if let Some(clock) = clock {
            clock.skipped_pass.store(skip.map_or(u32::MAX, |x| x.1), Ordering::Relaxed);
        }
        params.1 = false;
        (commands, g.queue.submit([enc.finish()]))
    }

    pub fn record_frame(&self, encoder: &mut wgpu::CommandEncoder, target: &wgpu::TextureView,
        t: f32, samples: u32, timestamps: Option<&wgpu::QuerySet>, timestamp_start: u32) -> Result<u32, String> {
        let mut params = self.params.lock().unwrap();
        params.1 = true;
        self.record(&mut self.runner.lock().unwrap(), encoder, target, t, samples,
            FrameOptions { timestamps, timestamp_start, ..Default::default() })
    }

    fn uniform(&self, t: f32, samples: u32, sun: bool) -> Vec<u8> {
        let mut bytes = frame_bytes(self.camera, self.sun, self.size, t, sun, self.light_view);
        bytes[188..192].copy_from_slice(&samples.to_le_bytes());
        bytes
    }

    fn record(&self, runner: &mut Runner, encoder: &mut wgpu::CommandEncoder, target: &wgpu::TextureView,
        t: f32, samples: u32, options: FrameOptions<'_>) -> Result<u32, String> {
        let uniform = |table| self.uniform(t, samples, table != self.frame_table);
        runner.encode_frame(self.id, encoder, target, t.into(), FrameOptions { uniform: Some(&uniform), ..options })
            .map(|r| r.commands).map_err(runner_error)
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
        loft_runner::read_texture(&g.device, &g.queue, &self.color, 0).map_err(runner_error)
    }

    /// The twin: runs kind `k`'s queries on the GPU at samples of 8 floats each (p.xyz or (u, v, 0), row as bits,
    /// dir.xyz, 0); returns 16 floats each (trace, visible, 0, 0 | material rgb, 0 | surface point, 0 | surface
    /// normal, 0). Built on demand; not part of a frame.
    pub fn twin(&self, g: &Gpu, k: usize, samples: &[f32]) -> Result<Vec<f32>, String> {
        if samples.is_empty() { return Ok(vec![]); }
        let src_rows = self.rows(k)?;
        let (bundle, rows, output) = loft_passes::bundle::lower_twin(self.twin_sources[k].clone(), &self.luts,
            self.kind_words[k], src_rows.size(), param_words(&self.params.lock().unwrap().0),
            frame_bytes(self.camera, self.sun, self.size, 0., false, self.light_view), samples)?;
        let mut runner = attach_runner(g);
        let id = runner.load_bundle(&bundle).map_err(runner_error)?;
        runner.bind_external_buffer(id, rows, src_rows).map_err(runner_error)?;
        let mut enc = g.device.create_command_encoder(&Default::default());
        runner.encode_frame(id, &mut enc, &self.color_view, 0., Default::default()).map_err(runner_error)?;
        g.queue.submit([enc.finish()]);
        let output = runner.export_buffer(id, output).ok_or("missing twin output")?;
        let view = loft_runner::read_buffer(&g.device, &g.queue, &output).map_err(runner_error)?;
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
