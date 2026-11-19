//! loft-draw: the one headless executor (plan 2026-11-19 §4.1). It owns a wgpu device and draws built bundles
//! (`scene.loft`, Bundle V1) through loft-gpu; it never compiles. Three commands, each taking a bundle file or the
//! directory `loft build` wrote it into:
//!
//!   loft-draw serve                      JSON lines: `loft serve`'s draw child, and `render`/`frames`/`clip` (serve.rs)
//!   loft-draw still BUNDLE [flags]       one PNG (still.rs)
//!   loft-draw profile BUNDLE [flags]     load and frame timings, the Vulkan cache, the frame budget (profile.rs)

mod profile;
mod serve;
mod still;
mod trace;

use loft_gpu::{Canvas, Layers, Loft, Owner, View};
use loft_ir::identity::Precision;
use std::path::Path;

/// What every picture is drawn into, and read back as.
pub const OUTPUT_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;
/// 4x multisampled, resolved into the output: as TS src/host/lower.ts drew R0 (its edges are the 4x resolve's).
pub const CANVAS: Canvas = Canvas { color: OUTPUT_FORMAT, depth: loft_gpu::DEPTH, samples: 4 };

/// What failed, in `src/cli/verdict.ts`'s kinds: the supervisor turns it into the exit code and the `loft:` line.
#[derive(Debug, serde::Serialize)]
#[serde(tag = "kind", content = "message", rename_all = "lowercase")]
pub enum Verdict {
    /// The request names something the artefact lacks, or a bad size or camera.
    Usage(String),
    Check(String),
    /// The bundle is refused: format, ids, sizes, or library WGSL that validation refuses.
    Load(String),
    /// The author's WGSL in the bundle, refused at load.
    Scene(String),
    /// The machine: no adapter, a failed read-back.
    Environment(String),
    /// A Loft bug (a panic in this process).
    Internal(String),
    /// Over a budget the caller set (`profile --budget-ms`).
    Budget(String),
}

impl Verdict {
    fn code(&self) -> i32 {
        match self {
            Verdict::Internal(_) => 1,
            Verdict::Usage(_) => 2,
            Verdict::Check(_) => 5,
            Verdict::Load(_) => 3,
            Verdict::Scene(_) => 4,
            Verdict::Environment(_) => 6,
            Verdict::Budget(_) => 7,
        }
    }
}

impl From<loft_ir::Error> for Verdict {
    fn from(e: loft_ir::Error) -> Verdict {
        use loft_ir::Error as E;
        let m = e.to_string();
        match e {
            E::Usage(_) => Verdict::Usage(m),
            E::Scene { .. } => Verdict::Scene(m),
            E::Budget { .. } => Verdict::Budget(m),
            E::Load(_) => Verdict::Load(m),
            E::Environment(_) => Verdict::Environment(m),
            E::Internal(_) => Verdict::Internal(m),
        }
    }
}

/// The bundle at `path`: a `scene.loft` file, or the directory `loft build` wrote one into.
pub fn read_bundle(path: &Path) -> Result<Vec<u8>, Verdict> {
    let file = if path.is_dir() { path.join("scene.loft") } else { path.to_path_buf() };
    std::fs::read(&file).map_err(|e| {
        Verdict::Load(format!(
            "{}: {e}. Fix: LOFT_ENGINE=rust loft build <scene> --out <dir> writes it",
            file.display()
        ))
    })
}

/// Loads `bytes` on `g` for the one canvas this executor draws into.
pub fn load(g: &Gpu, bytes: &[u8], ask: Precision, cache: Option<&wgpu::PipelineCache>) -> Result<Loft, Verdict> {
    Ok(pollster::block_on(Loft::load(&g.device, &g.queue, bytes, ask, &[CANVAS], cache))?)
}

/// A device, and what a frame's identity records of it (`src/identity.ts` `Identity.device`).
pub struct Gpu {
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub info: wgpu::AdapterInfo,
    pub timing: bool,
}

/// A high-performance device with `SHADER_F16` when `f16` and the adapter has it, and timestamps when it grants them.
/// Its loss ends the process with exit 6: the supervisor answers what is in flight and starts nothing on it.
pub fn gpu(f16: bool) -> Result<Gpu, Verdict> {
    let env = |e: &dyn std::fmt::Display| Verdict::Environment(e.to_string());
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::HighPerformance,
        ..Default::default()
    }))
    .map_err(|e| env(&format!("no adapter: {e}")))?;
    // PIPELINE_CACHE where the adapter has it (Vulkan), for `profile --pipeline-cache`.
    let wanted = wgpu::Features::TIMESTAMP_QUERY
        | wgpu::Features::PIPELINE_CACHE
        | if f16 { wgpu::Features::SHADER_F16 } else { wgpu::Features::empty() };
    let features = adapter.features() & wanted;
    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        label: Some("loft-draw"),
        required_features: features,
        ..Default::default()
    }))
    .map_err(|e| env(&format!("no device: {e}")))?;
    device.set_device_lost_callback(|reason, message| match reason {
        wgpu::DeviceLostReason::Unknown => {
            eprintln!("loft-draw: device lost: {message}");
            std::process::exit(6)
        }
        wgpu::DeviceLostReason::Destroyed => {}
    });
    Ok(Gpu { device, queue, info: adapter.get_info(), timing: features.contains(wgpu::Features::TIMESTAMP_QUERY) })
}

/// The output texture and its read-back buffer for one size (rows padded to 256 bytes).
pub struct Target {
    texture: wgpu::Texture,
    view: wgpu::TextureView,
    /// The 4x colour attachment, resolved into `view`.
    many: wgpu::TextureView,
    /// Read by the scene resolve when a pass reads the opaque scene (loft-gpu view.rs), so it is a binding too.
    depth: wgpu::TextureView,
    readback: wgpu::Buffer,
    size: (u32, u32),
    row: u32,
}

impl Target {
    pub fn new(device: &wgpu::Device, (w, h): (u32, u32)) -> Target {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("loft-draw output"),
            size: wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: OUTPUT_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let many = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("loft-draw colour x4"),
            size: wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: CANVAS.samples,
            dimension: wgpu::TextureDimension::D2,
            format: OUTPUT_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        let depth = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("loft-draw depth"),
            size: wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: CANVAS.samples,
            dimension: wgpu::TextureDimension::D2,
            format: CANVAS.depth,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let row = (w * 4).div_ceil(256) * 256;
        let readback = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("loft-draw readback"),
            size: (row * h) as u64,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let depth = depth.create_view(&Default::default());
        let many = many.create_view(&Default::default());
        Target { view: texture.create_view(&Default::default()), many, texture, depth, readback, size: (w, h), row }
    }

    /// Records the copy of the drawn texture into the read-back buffer.
    pub fn copy(&self, encoder: &mut wgpu::CommandEncoder) {
        encoder.copy_texture_to_buffer(
            self.texture.as_image_copy(),
            wgpu::TexelCopyBufferInfo {
                buffer: &self.readback,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(self.row),
                    rows_per_image: Some(self.size.1),
                },
            },
            self.texture.size(),
        );
    }

    /// Records one frame of `loft` into `encoder`: every layer, into attachments Loft owns (it clears them).
    pub fn encode(&self, gpu: &Gpu, loft: &mut Loft, encoder: &mut wgpu::CommandEncoder) -> Result<(), Verdict> {
        let (w, h) = self.size;
        let view = View {
            canvas: CANVAS,
            color: &self.many,
            resolve: Some(&self.view),
            depth: &self.depth,
            size: [w, h],
            owner: Owner::Loft,
            layers: Layers::All,
        };
        Ok(loft.encode_into(&gpu.device, &gpu.queue, encoder, &view)?)
    }

    /// One headless frame, as tightly packed RGBA.
    pub fn picture(&self, gpu: &Gpu, loft: &mut Loft) -> Result<Vec<u8>, Verdict> {
        let (w, h) = self.size;
        let mut encoder = gpu.device.create_command_encoder(&Default::default());
        self.encode(gpu, loft, &mut encoder)?;
        self.copy(&mut encoder);
        gpu.queue.submit([encoder.finish()]);
        let (tx, rx) = std::sync::mpsc::channel();
        self.readback.map_async(wgpu::MapMode::Read, .., move |r| tx.send(r).expect("readback receiver"));
        gpu.device.poll(wgpu::PollType::wait_indefinitely()).map_err(|e| Verdict::Environment(format!("poll: {e}")))?;
        rx.recv()
            .expect("map_async calls back inside poll")
            .map_err(|e| Verdict::Environment(format!("readback: {e}")))?;
        let mut rgba = Vec::with_capacity((w * h * 4) as usize);
        {
            let mapped = self.readback.get_mapped_range(..).expect("mapped readback");
            for y in 0..h as usize {
                let at = y * self.row as usize;
                rgba.extend_from_slice(&mapped[at..at + w as usize * 4]);
            }
        }
        self.readback.unmap();
        Ok(rgba)
    }
}

enum Command {
    Serve,
    Show(std::path::PathBuf),
    Trace(std::path::PathBuf, usize, String),
    Still(still::Args),
    Profile(still::Args, profile::Flags),
}

fn main() {
    let mut args = std::env::args().skip(1);
    let command = match args.next().as_deref() {
        Some("show") => {
            let (Some(file), None) = (args.next(), args.next()) else { still::usage("show BUNDLE".into()) };
            Command::Show(file.into())
        }
        Some("trace") => {
            let (Some(dir), Some(step), Some(sites), None) =
                (args.next(), args.next().and_then(|n| n.parse().ok()), args.next(), args.next())
            else {
                still::usage("trace BUNDLE STEP SITES_JSON".into())
            };
            Command::Trace(dir.into(), step, sites)
        }
        Some("serve") => Command::Serve,
        Some("still") => Command::Still(still::parse(args, false).0),
        Some("profile") => {
            let (a, f) = still::parse(args, true);
            Command::Profile(a, f)
        }
        other => {
            eprintln!("loft-draw: expected serve, still, profile, show or trace, got {other:?}");
            std::process::exit(2)
        }
    };
    let ended = match command {
        Command::Trace(dir, step, sites) => trace::run(&dir, step, &sites),
        Command::Show(dir) => trace::modules(&dir),
        Command::Serve => serve::run(),
        Command::Still(a) => still::run(a),
        Command::Profile(a, f) => profile::run(a, f),
    };
    if let Err(v) = ended {
        eprintln!("loft-draw: {v:?}");
        std::process::exit(v.code())
    }
}
