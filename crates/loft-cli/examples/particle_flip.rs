//! Run under tools/gpu_lock.sh. See the cap1b lane note for the browser host.
#[path = "particle_flip/twin.rs"]
mod twin;

use loft_ir::{
    BindAccess, Bundle, DepthConvention, DispatchExtent, ElemFormat, GpuFeatures, HostCaps,
    IndirectDraw, InstanceId, ModuleBlob, ModuleId, PassBinding, PassDecl, PassId, PassRun,
    QueueClass, ShaderTarget, TableDecl, TableId, TableLifetime, TargetDesc,
};
use loft_passes::{emit::Aov, source::Graph};
use loft_runner::{Runner, bundle, frame::FrameOptions};
use serde::Serialize;
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    sync::{Arc, mpsc},
    time::Instant,
};
use twin::{CELLS, COUNT, Distribution, Stencil, bytes, words};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;
const SIZE: (u32, u32) = (1280, 800);
const GRID_BYTES: u64 = CELLS as u64 * 16;
const SORT_BYTES: usize = 1_836_032;

#[derive(Clone, Copy)]
enum Route {
    Fixed8,
    Fixed27,
    Native,
    Sorted,
}

impl Route {
    fn name(self) -> &'static str {
        match self {
            Self::Fixed8 => "fixed8",
            Self::Fixed27 => "fixed27",
            Self::Native => "native-f32",
            Self::Sorted => "sorted-fixed8",
        }
    }

    fn scene(self) -> &'static str {
        match self {
            Self::Fixed8 => "scenes/particle_flip/linear.loft",
            Self::Fixed27 => "scenes/particle_flip.loft",
            Self::Native => "scenes/particle_flip/native.loft",
            Self::Sorted => "scenes/particle_flip/sorted.loft",
        }
    }

    fn stencil(self) -> Stencil {
        match self {
            Self::Fixed27 => Stencil::Quadratic,
            Self::Fixed8 | Self::Native | Self::Sorted => Stencil::Linear,
        }
    }

    fn taps(self) -> usize {
        match self.stencil() {
            Stencil::Linear => 8,
            Stencil::Quadratic => 27,
        }
    }

    fn scratch(self) -> usize {
        match self {
            Self::Fixed8 | Self::Fixed27 => GRID_BYTES as usize,
            Self::Native => 4 * GRID_BYTES as usize,
            Self::Sorted => SORT_BYTES,
        }
    }

    fn published(self) -> &'static str {
        match self {
            Self::Fixed8 | Self::Fixed27 | Self::Sorted => "Grid",
            Self::Native => "PublishedFloat",
        }
    }

    fn link(self, granted: wgpu::Features) -> Result<GpuFeatures> {
        match self {
            Self::Fixed8 | Self::Fixed27 | Self::Sorted => Ok(GpuFeatures::empty()),
            Self::Native if granted.contains(wgpu::Features::SHADER_FLOAT32_ATOMIC) => {
                Ok(GpuFeatures::FLOAT32_ATOMICS)
            }
            Self::Native => Err("Unsupported: device did not grant SHADER_FLOAT32_ATOMIC".into()),
        }
    }
}

struct Plan {
    route: Route,
    features: GpuFeatures,
    bundle: Bundle,
    tables: BTreeMap<String, TableId>,
}

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

impl Plan {
    fn new(route: Route, granted: wgpu::Features) -> Result<Self> {
        let features = route.link(granted)?;
        let src = Graph::file(&root().join(route.scene()), "")?;
        let (_, _, compiled) = loft_passes::compile_for_device(&src, &[], Aov::Beauty, features)
            .map_err(|d| d.render(&src))?;
        if let Route::Native = route {
            let scatter = compiled
                .compute
                .bundle
                .modules
                .iter()
                .filter_map(|m| std::str::from_utf8(&m.bytes).ok())
                .find(|s| s.contains("atomic<f32>"))
                .ok_or("native link did not select f32 atomics")?;
            assert!(scatter.contains("atomicAdd"));
            assert_eq!(
                compiled.compute.tiers["p2g"],
                loft_parse::ast::Determinism::NonDet
            );
            assert_eq!(
                compiled.compute.determinism[&compiled.compute.tables["PublishedFloat"]],
                loft_parse::ast::Determinism::NonDet
            );
        }
        Ok(Self {
            route,
            features,
            bundle: compiled.compute.bundle,
            tables: compiled.compute.tables,
        })
    }

    fn fixture(&self) -> Bundle {
        let mut out = self.bundle.clone();
        out.passes.retain(|pass| {
            !matches!(
                entry(pass),
                "pack_state" | "update_fixed" | "update_float" | "g2p8" | "g2p27"
            )
        });
        out.modules.retain(|module| {
            out.passes
                .iter()
                .any(|p| matches!(p.run, PassRun::Compute { module: id, .. } if id == module.id))
        });
        out
    }

    fn showcase(&self) -> Bundle {
        let mut out = self.bundle.clone();
        let module = ModuleId(out.modules.len() as u32);
        let shader = include_bytes!("../../../stdlib/physics/particle_splash.wgsl");
        out.modules.push(ModuleBlob {
            id: module,
            target: ShaderTarget::Wgsl,
            hash: *blake3::hash(shader).as_bytes(),
            bytes: shader.as_slice().into(),
        });
        let args = TableId(out.tables.len() as u32);
        out.tables.push(TableDecl {
            id: args,
            format: ElemFormat::U32,
            lifetime: TableLifetime::Static,
            stride_bytes: 4,
            capacity: 4,
            initial: bytes([6, COUNT as u32 + 2, 0, 0]).into(),
        });
        let wave = out.passes.len() as u16;
        out.passes.push(PassDecl {
            id: PassId(u32::from(wave)),
            wave,
            queue: QueueClass::Graphics,
            bindings: Box::new([PassBinding {
                group: 1,
                slot: 0,
                resource: self.tables["State"],
                access: BindAccess::ReadStorage,
            }]),
            run: PassRun::RenderBundle {
                draws: Box::new([IndirectDraw {
                    module,
                    vertex_entry: "vs".into(),
                    fragment_entry: "fs".into(),
                    args,
                    offset_bytes: 0,
                    vertex: None,
                    index: None,
                }]),
                color_targets: Box::new([TargetDesc {
                    table: TableId(args.0 + 1),
                }]),
                depth: None,
                msaa: 1,
            },
        });
        out
    }

    fn save(&self, out: &Path, label: &str, bundle: &Bundle) -> Result<()> {
        let dir = out.join(label);
        fs::create_dir_all(&dir)?;
        fs::write(dir.join("scene.loftb"), bundle::encode(bundle))?;
        let mut hashes = Vec::new();
        for m in &bundle.modules {
            let name = format!("{}.wgsl", m.id.0);
            fs::write(dir.join(&name), &m.bytes)?;
            hashes.push(format!("{}  {name}", blake3::Hash::from_bytes(m.hash)));
        }
        fs::write(dir.join("shaders.blake3"), hashes.join("\n") + "\n")?;
        Ok(())
    }
}

fn entry(pass: &PassDecl) -> &str {
    match &pass.run {
        PassRun::Compute { entry, .. } if entry == "main" => "scatter_f32",
        PassRun::Compute { entry, .. } => entry,
        PassRun::RenderBundle { .. } => "splash",
        other => panic!("unexpected particle pass {other:?}"),
    }
}

fn diagnostic(out: &Path, granted: wgpu::Features) -> Result<()> {
    let path = Path::new(Route::Native.scene());
    let source = fs::read_to_string(root().join(path))?;
    let incompatible = source
        .replace(
            "schedule Main {",
            r"
kernel exact_consumer(src: [FloatCell], dst: [Word]) allow(atomics) {
  atomic_add(dst[0].value, u32(src[0].mass))
}
schedule Main {
  table(Exact, row: Word, capacity: 1)",
        )
        .replace(
            "  pass publish",
            r"  pass exact_opt_in reads [FloatGrid] writes [Exact] {
    compute(entry: exact_consumer, extent: 1, src: FloatGrid, dst: Exact)
  }
  pass publish",
        );
    let src = Graph::text(&root(), path, &incompatible);
    let features = Route::Native
        .link(granted)
        .unwrap_or_else(|_| GpuFeatures::empty());
    let err = loft_passes::compile_for_device(&src, &[], Aov::Beauty, features)
        .err()
        .ok_or("E504 was not raised")?;
    let report = err.render(&src);
    fs::write(out.join("exact-consumer.loft"), incompatible)?;
    fs::write(out.join("E504.txt"), &report)?;
    if err.code != loft_ir::Code::E504
        || !report.contains("pass p2g")
        || !report.contains("NonDet producer")
    {
        return Err(report.into());
    }
    Ok(())
}

struct Device {
    device: Arc<wgpu::Device>,
    queue: Arc<wgpu::Queue>,
    info: wgpu::AdapterInfo,
}

impl Device {
    fn new() -> Result<Self> {
        let instance =
            wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            ..Default::default()
        }))?;
        let features = adapter.features()
            & (wgpu::Features::SHADER_FLOAT32_ATOMIC | wgpu::Features::TIMESTAMP_QUERY);
        let (device, queue) =
            pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
                required_features: features,
                ..Default::default()
            }))?;
        Ok(Self {
            device: Arc::new(device),
            queue: Arc::new(queue),
            info: adapter.get_info(),
        })
    }

    fn buffer(&self, size: u64, usage: wgpu::BufferUsages) -> wgpu::Buffer {
        self.device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size,
            usage,
            mapped_at_creation: false,
        })
    }

    fn encoder(&self) -> wgpu::CommandEncoder {
        self.device.create_command_encoder(&Default::default())
    }

    fn wait(&self) -> Result<()> {
        self.device
            .poll(wgpu::PollType::wait_indefinitely())
            .map_err(|e| format!("{e:?}"))?;
        Ok(())
    }

    fn map(&self, buffer: &wgpu::Buffer, len: u64) -> Result<Vec<u8>> {
        let slice = buffer.slice(..len);
        let (send, recv) = mpsc::channel();
        slice.map_async(wgpu::MapMode::Read, move |result| {
            let _ = send.send(result);
        });
        self.wait()?;
        recv.recv()??;
        let data = slice.get_mapped_range().to_vec();
        buffer.unmap();
        Ok(data)
    }

    fn attach(&self, features: GpuFeatures) -> Runner {
        Runner::attach(
            self.device.clone(),
            self.queue.clone(),
            HostCaps {
                depth_convention: DepthConvention::StandardZeroToOne,
                msaa_samples: 1,
                features,
            },
        )
    }
}

struct Loaded<'a> {
    plan: &'a Plan,
    runner: Runner,
    id: InstanceId,
    target: wgpu::Texture,
    view: wgpu::TextureView,
    stages: Vec<String>,
}

impl<'a> Loaded<'a> {
    fn new(gpu: &Device, plan: &'a Plan, b: &Bundle) -> Result<Self> {
        let target = gpu.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("particle tank"),
            size: wgpu::Extent3d {
                width: SIZE.0,
                height: SIZE.1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let view = target.create_view(&Default::default());
        let mut runner = gpu.attach(plan.features);
        let id = runner
            .load_bundle_bytes(&bundle::encode(b))
            .map_err(|e| format!("{} load: {e:?}", plan.route.name()))?;
        runner
            .prepare_frame(id, &view)
            .map_err(|e| format!("{} prepare: {e:?}", plan.route.name()))?;
        let stages = b.passes.iter().map(|p| entry(p).to_string()).collect();
        Ok(Self {
            plan,
            runner,
            id,
            target,
            view,
            stages,
        })
    }

    fn buffer(&self, table: &str) -> Arc<wgpu::Buffer> {
        self.runner
            .export_buffer(self.id, self.plan.tables[table])
            .expect("declared particle buffer")
    }

    fn upload(&self, gpu: &Device, table: &str, bytes: &[u8]) {
        let buffer = self.buffer(table);
        gpu.queue.write_buffer(&buffer, 0, bytes);
    }

    fn record(
        &mut self,
        enc: &mut wgpu::CommandEncoder,
        timer: Option<&wgpu::QuerySet>,
    ) -> Result<()> {
        let frame = self
            .runner
            .encode_frame(
                self.id,
                enc,
                &self.view,
                0.,
                FrameOptions {
                    timestamps: timer,
                    ..Default::default()
                },
            )
            .map_err(|e| format!("{} dispatch: {e:?}", self.plan.route.name()))?;
        assert_eq!(frame.stages.len(), self.stages.len());
        Ok(())
    }

    fn step(&mut self, gpu: &Device) -> Result<()> {
        let mut enc = gpu.encoder();
        self.record(&mut enc, None)?;
        gpu.queue.submit([enc.finish()]);
        Ok(())
    }

    fn snapshot(&self, gpu: &Device, table: &str, n: u64) -> Result<Vec<u8>> {
        let read = gpu.buffer(
            n,
            wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        );
        let mut enc = gpu.encoder();
        enc.copy_buffer_to_buffer(&self.buffer(table), 0, &read, 0, n);
        gpu.queue.submit([enc.finish()]);
        gpu.map(&read, n)
    }

    fn grid(&self, gpu: &Device) -> Result<Vec<u8>> {
        self.snapshot(gpu, self.plan.route.published(), GRID_BYTES)
    }

    fn status(&self, gpu: &Device) -> Result<u32> {
        let status = self.snapshot(gpu, "Status", 4)?;
        Ok(words(&status).next().unwrap())
    }

    fn picture(&self, gpu: &Device, path: &Path) -> Result<()> {
        let stride = SIZE.0 * 4;
        let n = u64::from(stride) * u64::from(SIZE.1);
        let read = gpu.buffer(
            n,
            wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        );
        let mut enc = gpu.encoder();
        enc.copy_texture_to_buffer(
            self.target.as_image_copy(),
            wgpu::TexelCopyBufferInfo {
                buffer: &read,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(stride),
                    rows_per_image: Some(SIZE.1),
                },
            },
            self.target.size(),
        );
        gpu.queue.submit([enc.finish()]);
        let rgba = gpu.map(&read, n)?;
        let mut encoder = png::Encoder::new(fs::File::create(path)?, SIZE.0, SIZE.1);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        encoder.write_header()?.write_image_data(&rgba)?;
        Ok(())
    }
}

fn exact(raw: &[u8], grid: &[[i64; 4]], label: &str) -> Result<()> {
    assert_eq!(raw.len(), GRID_BYTES as usize);
    let reference = twin::integer_words(grid);
    if let Some((i, (actual, expected))) = words(raw)
        .zip(reference)
        .enumerate()
        .find(|(_, (a, b))| a != b)
    {
        return Err(format!(
            "{label}: word {i}, actual {}, expected {}",
            actual as i32, expected as i32
        )
        .into());
    }
    Ok(())
}

fn fixed_fixtures(gpu: &Device, loaded: &mut Loaded<'_>, out: &Path) -> Result<()> {
    let route = loaded.plan.route;
    let dir = out.join(route.name());
    for (label, rows) in [
        ("spread", twin::fixture(0, Distribution::Spread)),
        ("clumped", twin::fixture(0, Distribution::Clumped)),
        ("boundaries", twin::edge_fixture()),
        ("cancellation", twin::cancellation_fixture()),
    ] {
        let cpu = twin::integer_grid(&rows, route.stencil());
        fs::write(
            dir.join(format!("{label}.i32")),
            bytes(twin::integer_words(&cpu)),
        )?;
        for reverse in [false, true] {
            let input = if reverse {
                bytes(rows.iter().rev().flatten().copied())
            } else {
                bytes(rows.iter().flatten().copied())
            };
            let name = format!("{label}-{}", if reverse { "reversed" } else { "forward" });
            fs::write(dir.join(format!("{name}.particles")), &input)?;
            loaded.upload(gpu, "Packed", &input);
            loaded.upload(gpu, "Control", &(COUNT as u32).to_le_bytes());
            loaded.step(gpu)?;
            let actual = loaded.grid(gpu)?;
            fs::write(dir.join(format!("{name}.grid")), &actual)?;
            exact(&actual, &cpu, &name)?;
            assert_eq!(loaded.status(gpu)?, 0);
        }
        loaded.upload(gpu, "Control", &(COUNT as u32 + 1).to_le_bytes());
        loaded.step(gpu)?;
        let actual = loaded.grid(gpu)?;
        fs::write(dir.join(format!("{label}-capacity.grid")), &actual)?;
        let status = loaded.status(gpu)?;
        fs::write(
            dir.join(format!("{label}-capacity.status")),
            status.to_le_bytes(),
        )?;
        assert_eq!(status, 1, "{label}: expected Capacity");
        exact(&actual, &cpu, &format!("{label} after Capacity"))?;
    }
    Ok(())
}

fn float_fixtures(gpu: &Device, loaded: &mut Loaded<'_>, out: &Path) -> Result<()> {
    let dir = out.join("native-f32");
    let mut scores = Vec::new();
    let mut pass = true;
    loaded.upload(gpu, "Control", &(COUNT as u32).to_le_bytes());
    for layout in [Distribution::Spread, Distribution::Clumped] {
        for seed in 0..32 {
            let rows = twin::fixture(seed, layout);
            let (truth, abs) = twin::float_grid(&rows);
            for reverse in [false, true] {
                let label = format!(
                    "{}-{seed:02}-{}",
                    layout.name(),
                    if reverse { "reversed" } else { "forward" }
                );
                let input = if reverse {
                    bytes(rows.iter().rev().flatten().copied())
                } else {
                    bytes(rows.iter().flatten().copied())
                };
                if !reverse {
                    fs::write(dir.join(format!("{label}.particles")), &input)?;
                }
                loaded.upload(gpu, "Packed", &input);
                loaded.step(gpu)?;
                let grid = loaded.grid(gpu)?;
                fs::write(dir.join(format!("{label}.grid")), &grid)?;
                let error = twin::float_error(&grid, &truth, abs);
                pass &= error.passes();
                scores.push(json!({"fixture": label, "seed": seed, "reverse": reverse, "error": error, "passed": error.passes()}));
            }
        }
    }
    write_json(
        &dir.join("scores.json"),
        &json!({"passed": pass, "results": scores}),
    )?;
    if pass {
        Ok(())
    } else {
        Err("native f32 error budget exceeded; see native-f32/scores.json".into())
    }
}

#[derive(Serialize)]
struct Sample {
    cpu_encode_ms: f64,
    cpu_submit_ms: f64,
    cpu_wait_ms: f64,
    gpu_ms: f64,
    pass_ms: Vec<f64>,
    ticks: Vec<u64>,
}

struct Clock {
    query: wgpu::QuerySet,
    resolve: wgpu::Buffer,
    output: wgpu::Buffer,
    n: u32,
}

impl Clock {
    fn new(gpu: &Device, stages: usize) -> Self {
        let n = stages as u32 * 2;
        let query = gpu.device.create_query_set(&wgpu::QuerySetDescriptor {
            label: None,
            ty: wgpu::QueryType::Timestamp,
            count: n,
        });
        let resolve = gpu.buffer(
            u64::from(n) * 8,
            wgpu::BufferUsages::QUERY_RESOLVE | wgpu::BufferUsages::COPY_SRC,
        );
        let output = gpu.buffer(
            u64::from(n) * 8,
            wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        );
        Self {
            query,
            resolve,
            output,
            n,
        }
    }

    fn sample(&self, gpu: &Device, loaded: &mut Loaded<'_>) -> Result<Sample> {
        let start = Instant::now();
        let mut enc = gpu.encoder();
        loaded.record(&mut enc, Some(&self.query))?;
        let commands = enc.finish();
        let cpu_encode_ms = ms(start);
        let start = Instant::now();
        gpu.queue.submit([commands]);
        let cpu_submit_ms = ms(start);
        let start = Instant::now();
        gpu.wait()?;
        let cpu_wait_ms = ms(start);
        let mut enc = gpu.encoder();
        enc.resolve_query_set(&self.query, 0..self.n, &self.resolve, 0);
        let len = u64::from(self.n) * 8;
        enc.copy_buffer_to_buffer(&self.resolve, 0, &self.output, 0, len);
        gpu.queue.submit([enc.finish()]);
        let raw = gpu.map(&self.output, len)?;
        let ticks = raw
            .chunks_exact(8)
            .map(|b| u64::from_le_bytes(b.try_into().unwrap()))
            .collect::<Vec<_>>();
        let to_ms = |d: u64| d as f64 * f64::from(gpu.queue.get_timestamp_period()) / 1e6;
        if ticks[0] == 0
            || ticks.chunks_exact(2).any(|t| t[1] < t[0])
            || ticks[0] == ticks[ticks.len() - 1]
        {
            return Err(format!("invalid timestamp sequence: {ticks:?}").into());
        }
        Ok(Sample {
            cpu_encode_ms,
            cpu_submit_ms,
            cpu_wait_ms,
            gpu_ms: to_ms(ticks[ticks.len() - 1] - ticks[0]),
            pass_ms: ticks.chunks_exact(2).map(|t| to_ms(t[1] - t[0])).collect(),
            ticks,
        })
    }
}

fn ms(since: Instant) -> f64 {
    since.elapsed().as_secs_f64() * 1000.
}

fn percentile(mut samples: Vec<f64>, fraction: f64) -> f64 {
    samples.sort_by(f64::total_cmp);
    let index = ((samples.len() as f64 * fraction).ceil() as usize).saturating_sub(1);
    samples[index]
}

fn pair_interval(a: &[f64], b: &[f64]) -> [f64; 2] {
    let mut estimates = Vec::with_capacity(2048);
    for i in 0..2048 {
        let indices = (0..a.len()).map(|j| twin::mix((i * a.len() + j) as u32) as usize % a.len());
        let (left, right): (Vec<_>, Vec<_>) = indices.map(|j| (a[j], b[j])).unzip();
        estimates.push(percentile(right, 0.95) - percentile(left, 0.95));
    }
    [
        percentile(estimates.clone(), 0.025),
        percentile(estimates, 0.975),
    ]
}

fn summarize(loaded: &mut Loaded<'_>, rows: &[Sample]) -> Result<Value> {
    let stats = |samples: Vec<f64>| {
        json!({
            "p50": percentile(samples.clone(), 0.50), "p95": percentile(samples, 0.95),
        })
    };
    let route = loaded.plan.route;
    let p2g = loaded
        .stages
        .iter()
        .position(|e| {
            matches!(
                e.as_str(),
                "scatter8" | "scatter27" | "scatter_f32" | "gather_sorted8"
            )
        })
        .unwrap();
    let scatter = percentile(rows.iter().map(|r| r.pass_ms[p2g]).collect(), 0.50);
    let pipelines = loaded
        .runner
        .pipeline_creates(loaded.id)
        .map_err(|e| format!("{e:?}"))?;
    let passes = loaded.stages.iter().enumerate().map(|(i, name)| {
        json!({"name": name, "gpu_ms": stats(rows.iter().map(|r| r.pass_ms[i]).collect())})
    }).collect::<Vec<_>>();
    let solver = |r: &Sample| {
        loaded
            .stages
            .iter()
            .zip(&r.pass_ms)
            .filter(|(s, _)| s.starts_with("update_") || s.starts_with("g2p"))
            .map(|(_, d)| *d)
            .sum()
    };
    Ok(json!({
        "route": route.name(), "scratch_bytes": route.scratch(), "particles": COUNT,
        "contributions": COUNT * route.taps(), "compute_pipelines": pipelines.compute.len(),
        "render_pipelines": pipelines.render.len(), "passes": passes,
        "gpu_ms": stats(rows.iter().map(|r| r.gpu_ms).collect()),
        "solver_ms": stats(rows.iter().map(solver).collect()),
        "cpu_encode_ms": stats(rows.iter().map(|r| r.cpu_encode_ms).collect()),
        "cpu_submit_ms": stats(rows.iter().map(|r| r.cpu_submit_ms).collect()),
        "cpu_wait_ms": stats(rows.iter().map(|r| r.cpu_wait_ms).collect()),
        "particles_per_s_p2g": COUNT as f64 * 1000. / scatter,
        "contributions_per_s_p2g": (COUNT * route.taps()) as f64 * 1000. / scatter,
        "samples": rows,
    }))
}

fn paired(
    gpu: &Device,
    a: &mut Loaded<'_>,
    b: &mut Loaded<'_>,
    count: usize,
    out: &Path,
) -> Result<()> {
    let ca = Clock::new(gpu, a.stages.len());
    let cb = Clock::new(gpu, b.stages.len());
    let mut sa = Vec::with_capacity(count);
    let mut sb = Vec::with_capacity(count);
    for i in 0..count + 8 {
        let (left, right) = if i % 2 == 0 {
            (ca.sample(gpu, a)?, cb.sample(gpu, b)?)
        } else {
            let right = cb.sample(gpu, b)?;
            (ca.sample(gpu, a)?, right)
        };
        if i >= 8 {
            sa.push(left);
            sb.push(right);
        }
    }
    let durations = |rows: &[Sample]| rows.iter().map(|r| r.gpu_ms).collect::<Vec<_>>();
    let interval = pair_interval(&durations(&sa), &durations(&sb));
    write_json(
        out,
        &json!({"a": summarize(a, &sa)?, "b": summarize(b, &sb)?,
                            "paired_b_minus_a_gpu_p95_ci95_ms": interval}),
    )
}

fn measure_drift(gpu: &Device, loaded: &mut Loaded<'_>, out: &Path) -> Result<()> {
    let mut truth = twin::splash();
    loaded.upload(gpu, "State", &twin::state_bytes(&truth));
    loaded.upload(gpu, "Control", &(COUNT as u32).to_le_bytes());
    let mut samples = Vec::new();
    for step in 1..=120 {
        loaded.step(gpu)?;
        twin::step(&mut truth);
        let actual = loaded.snapshot(gpu, "State", (COUNT * 32) as u64)?;
        samples.push(json!({"step": step, "error": twin::drift(&actual, &truth)}));
        if step == 120 {
            fs::write(out.join("drift-120.gpu-state"), actual)?;
            fs::write(out.join("drift-120.cpu-state"), twin::state_bytes(&truth))?;
        }
    }
    write_json(&out.join("drift.json"), &json!(samples))
}

fn write_json(path: &Path, value: &Value) -> Result<()> {
    serde_json::to_writer_pretty(fs::File::create(path)?, value)?;
    Ok(())
}

fn browser_plan(plan: &Plan, b: &Bundle) -> Result<Value> {
    // Project the decoded .loftb pass ABI for the JS host. No shader or bind
    // layout is hand-copied into the browser.
    let b = bundle::decode(&bundle::encode(b))?;
    bundle::validate_wgsl(&b)?;
    let modules = b.modules.iter().map(|m| {
        Ok(json!({"id": m.id.0, "wgsl": std::str::from_utf8(&m.bytes)?, "hash": blake3::Hash::from_bytes(m.hash).to_hex().as_str()}))
    }).collect::<Result<Vec<_>>>()?;
    let tables = b.tables.iter().map(|t| {
        json!({"id": t.id.0, "size": u64::from(t.capacity) * u64::from(t.stride_bytes), "initial": words(&t.initial).collect::<Vec<_>>()})
    }).collect::<Vec<_>>();
    let passes = b.passes.iter().map(|p| {
        let binding = p.bindings.iter().map(|b| match b.access {
            BindAccess::ReadStorage => Ok(json!([b.group, b.slot, b.resource.0, "read-only-storage"])),
            BindAccess::ReadWriteStorage => Ok(json!([b.group, b.slot, b.resource.0, "storage"])),
            other => Err(format!("Unsupported browser particle binding {other:?}")),
        }).collect::<std::result::Result<Vec<_>, _>>()?;
        let mut work = match &p.run {
            PassRun::Compute { module, entry, dispatch: DispatchExtent::Fixed(groups) } => {
                json!({"kind": "compute", "module": module.0, "entry": entry, "groups": groups})
            }
            PassRun::RenderBundle { draws, .. } if draws.len() == 1 => {
                let draw = &draws[0];
                json!({"kind": "render", "module": draw.module.0, "vertex": draw.vertex_entry, "fragment": draw.fragment_entry, "args": draw.args.0})
            }
            other => return Err(format!("Unsupported browser particle pass {other:?}").into()),
        };
        work["name"] = json!(entry(p));
        work["bindings"] = json!(binding);
        Ok(work)
    }).collect::<Result<Vec<_>>>()?;
    Ok(
        json!({"route": plan.route.name(), "scratch_bytes": plan.route.scratch(), "particles": COUNT,
              "contributions": COUNT * plan.route.taps(), "modules": modules, "buffers": tables,
              "tables": plan.tables.iter().map(|(name, id)| (name, id.0)).collect::<BTreeMap<_, _>>(),
              "passes": passes}),
    )
}

fn export_browser(
    out: &Path,
    linear: &Plan,
    quadratic: &Plan,
    sorted: &Plan,
    splash: &[u8],
) -> Result<()> {
    for (name, plan, bundle) in [
        ("fixed-browser", linear, linear.bundle.clone()),
        ("sorted-browser", sorted, sorted.bundle.clone()),
        ("splash-browser", quadratic, quadratic.showcase()),
    ] {
        plan.save(out, name, &bundle)?;
        write_json(
            &out.join(format!("{name}.json")),
            &browser_plan(plan, &bundle)?,
        )?;
    }
    fs::write(out.join("splash.state"), splash)?;
    Ok(())
}

fn triangle() -> Result<[f64; 2]> {
    loft_cli::native::gpu()?
        .triangle_control(SIZE)
        .map_err(Into::into)
}

fn main() -> Result<()> {
    let dir = std::env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .ok_or("usage: particle_flip EVIDENCE_DIR")?;
    fs::create_dir_all(&dir)?;
    let gpu = Device::new()?;
    let granted = gpu.device.features();
    let linear = Plan::new(Route::Fixed8, granted)?;
    let quadratic = Plan::new(Route::Fixed27, granted)?;
    let sorted = Plan::new(Route::Sorted, granted)?;
    let splash = twin::state_bytes(&twin::splash());
    export_browser(&dir, &linear, &quadratic, &sorted, &splash)?;
    diagnostic(&dir, granted)?;

    let mut witnesses = Vec::new();
    let mut fixed8 = Loaded::new(&gpu, &linear, &linear.fixture())?;
    for (plan, loaded) in [
        (&linear, &mut fixed8),
        (
            &quadratic,
            &mut Loaded::new(&gpu, &quadratic, &quadratic.fixture())?,
        ),
    ] {
        plan.save(&dir, plan.route.name(), &plan.fixture())?;
        fixed_fixtures(&gpu, loaded, &dir)?;
        witnesses.push(plan.route.name());
    }
    eprintln!("exact grids held: {witnesses:?}");
    let native = if Route::Native.link(granted).is_ok() {
        Some(Plan::new(Route::Native, granted)?)
    } else {
        None
    };
    let before = triangle().ok();
    let mut misses = Vec::new();
    if let Some(native) = &native {
        native.save(&dir, native.route.name(), &native.fixture())?;
        let mut floating = Loaded::new(&gpu, native, &native.fixture())?;
        if let Err(error) = float_fixtures(&gpu, &mut floating, &dir) {
            misses.push(error.to_string());
        }
        measure_drift(&gpu, &mut Loaded::new(&gpu, native, &native.bundle)?, &dir)?;
        if granted.contains(wgpu::Features::TIMESTAMP_QUERY) {
            for layout in [Distribution::Spread, Distribution::Clumped] {
                let input = bytes(twin::fixture(0, layout).into_iter().flatten());
                for loaded in [&fixed8, &floating] {
                    loaded.upload(&gpu, "Packed", &input);
                    loaded.upload(&gpu, "Control", &(COUNT as u32).to_le_bytes());
                }
                paired(
                    &gpu,
                    &mut fixed8,
                    &mut floating,
                    64,
                    &dir.join(format!("paired-{}.json", layout.name())),
                )?;
            }
        }
    }
    let scene8 = linear.showcase();
    let scene27 = quadratic.showcase();
    quadratic.save(&dir, "showcase", &scene27)?;
    let mut render8 = Loaded::new(&gpu, &linear, &scene8)?;
    let mut render27 = Loaded::new(&gpu, &quadratic, &scene27)?;
    for loaded in [&render8, &render27] {
        loaded.upload(&gpu, "State", &splash);
        loaded.upload(&gpu, "Control", &(COUNT as u32).to_le_bytes());
    }
    for frame in 1..=120 {
        render27.step(&gpu)?;
        if [1, 48, 120].contains(&frame) {
            render27.picture(&gpu, &dir.join(format!("splash-{frame:03}.png")))?;
        }
    }
    render27.upload(&gpu, "State", &splash);
    if granted.contains(wgpu::Features::TIMESTAMP_QUERY) {
        paired(
            &gpu,
            &mut render8,
            &mut render27,
            300,
            &dir.join("paired-showcase.json"),
        )?;
    }
    let after = triangle().ok();
    let admitted = before
        .zip(after)
        .is_some_and(|(a, b)| a[1] < 0.15 && b[1] < 0.15);
    write_json(
        &dir.join("native.json"),
        &json!({
            "adapter": gpu.info.name, "backend": format!("{:?}", gpu.info.backend), "device_features": format!("{granted:?}"),
            "triangle_before_p50_p95_ms": before, "triangle_after_p50_p95_ms": after,
            "triangle_admitted": admitted, "viewport": [SIZE.0, SIZE.1], "fixed_witnesses": witnesses,
            "float_route": if native.is_some() { "native-f32" } else { "Unsupported: SHADER_FLOAT32_ATOMIC" },
            "timestamp_granted": granted.contains(wgpu::Features::TIMESTAMP_QUERY), "misses": misses,
        }),
    )?;
    if misses.is_empty() {
        Ok(())
    } else {
        Err(misses.join("\n").into())
    }
}
