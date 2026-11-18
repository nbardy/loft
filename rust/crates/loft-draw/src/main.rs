//! loft-draw: the one headless executor (plan 2026-11-19 §4.1). It owns a wgpu device and draws built scenes; it
//! never compiles. Two commands:
//!
//!   loft-draw serve                      JSON lines: `loft serve`'s draw child, and `render`/`frames`/`clip` (serve.rs)
//!   loft-draw still DIR [flags]          one PNG, plus frame and pose timings for `profile` (still.rs; was loft-view)

mod serve;
mod still;

use loft_loader::{Bounds, Loft, OUTPUT_FORMAT};

/// What failed, in `src/cli/verdict.ts`'s kinds: the supervisor turns it into the exit code and the `loft:` line.
#[derive(Debug, serde::Serialize)]
#[serde(tag = "kind", content = "message", rename_all = "lowercase")]
pub enum Verdict {
    /// The request names something the artefact lacks, or a bad size or camera.
    Usage(String),
    /// The artefact is refused: format, links, sizes, or WGSL that validation refuses.
    Load(String),
    /// The machine: no adapter, a failed read-back.
    Environment(String),
    /// A Loft bug (a panic in this process).
    Internal(String),
}

impl Verdict {
    fn code(&self) -> i32 {
        match self {
            Verdict::Internal(_) => 1,
            Verdict::Usage(_) => 2,
            Verdict::Load(_) => 3,
            Verdict::Environment(_) => 6,
        }
    }
}

impl From<loft_loader::Error> for Verdict {
    fn from(e: loft_loader::Error) -> Verdict {
        use loft_loader::Error as E;
        let m = e.to_string();
        match e {
            E::Io { .. }
            | E::Manifest { .. }
            | E::NotLoft(..)
            | E::Version(..)
            | E::BlobLength { .. }
            | E::CpuTables(..)
            | E::BadId { .. }
            | E::MissingBlob(..)
            | E::Invalid(_)
            | E::ClearWithoutValue(..)
            | E::Gpu { .. } => Verdict::Load(m),
            E::UnknownName { .. } | E::InputFloats { .. } | E::TimeInput | E::Camera(_) | E::Size(..) | E::Curves(..) => {
                Verdict::Usage(m)
            }
            E::PipelineCacheUnsupported(_) | E::PipelineCacheIo { .. } | E::PipelineCacheNoData(_) | E::Map(_) => {
                Verdict::Environment(m)
            }
        }
    }
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
    let wanted = wgpu::Features::TIMESTAMP_QUERY | if f16 { wgpu::Features::SHADER_F16 } else { wgpu::Features::empty() };
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
        let row = (w * 4).div_ceil(256) * 256;
        let readback = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("loft-draw readback"),
            size: (row * h) as u64,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        Target { view: texture.create_view(&Default::default()), texture, readback, size: (w, h), row }
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

    /// One headless frame: it waits for its own level-of-detail bounds (as `loft render` does), and comes back as
    /// tightly packed RGBA.
    pub fn picture(&self, gpu: &Gpu, loft: &mut Loft) -> Result<(Vec<u8>, loft_loader::FrameStats), Verdict> {
        let (w, h) = self.size;
        let mut encoder = gpu.device.create_command_encoder(&Default::default());
        let stats = loft.encode_frame(&gpu.device, &gpu.queue, &mut encoder, &self.view, w, h, Bounds::Latest)?;
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
        Ok((rgba, stats))
    }
}

enum Command {
    Serve,
    Still(still::Args),
}

fn main() {
    let mut args = std::env::args().skip(1);
    let command = match args.next().as_deref() {
        Some("serve") => Command::Serve,
        Some("still") => Command::Still(still::parse(args)),
        other => {
            eprintln!("loft-draw: expected `serve` or `still DIR ...`, got {other:?}");
            std::process::exit(2)
        }
    };
    let ended = match command {
        Command::Serve => serve::run(),
        Command::Still(a) => still::run(a),
    };
    if let Err(v) = ended {
        eprintln!("loft-draw: {v:?}");
        std::process::exit(v.code())
    }
}
