//! `loft-draw profile`: the load and frame timings the gates read (plan 2026-11-19 §8.1 frame-time gate, §7 frame
//! budgets; lane 4.2), on loft-gpu.
//!
//!   loft-draw profile <bundle> [still's view flags] [--frames N] [--loads N] [--pipeline-cache DIR] [--budget-ms MS]
//!
//! - `--loads N` (default 1) loads the bundle N times on one device and times each load. With `--pipeline-cache DIR`
//!   every load compiles through one pipeline cache (loft-gpu `PipelineCache`, which holds DIR locked), read from DIR
//!   first and saved there after the last load. That is wgpu's `PIPELINE_CACHE`, which only Vulkan grants: another
//!   backend is refused as `environment` (Metal has its OS cache, which the second of `--loads 2` shows).
//! - `--frames N` (default 16) draws N frames of the view as a game would. CPU is the time to record a frame; GPU,
//!   with timestamp queries, is the kernel pass (frame kernels, the "pose" of the TS host) plus the render passes.
//! - `--budget-ms MS`: a p95 GPU frame over MS exits 7 (`budget`) after the report.
//!
//! The last stdout line is one JSON object, `{"profile": {...}}`.

use crate::still::{Args, aim};
use crate::{Gpu, Target, Verdict, gpu, load, read_bundle};
use loft_gpu::{Loft, PipelineCache};
use loft_ir::identity::Precision;
use serde_json::json;
use std::path::PathBuf;
use std::time::Instant;

pub struct Flags {
    pub frames: usize,
    pub loads: usize,
    pub cache: Option<PathBuf>,
    pub budget_ms: Option<f64>,
}

impl Default for Flags {
    fn default() -> Flags {
        Flags { frames: 16, loads: 1, cache: None, budget_ms: None }
    }
}

/// Least, median, p95 and most, in milliseconds.
fn quantiles(xs: &[f64]) -> [f64; 4] {
    let mut s = xs.to_vec();
    s.sort_by(f64::total_cmp);
    [0.0, 0.5, 0.95, 1.0].map(|p| s[((s.len() - 1) as f64 * p).round() as usize])
}

fn line(name: &str, q: [f64; 4]) {
    println!("{name:<15}least {:.3}  median {:.3}  p95 {:.3}  most {:.3} ms", q[0], q[1], q[2], q[3]);
}

fn ms(t: Instant) -> impl Fn() -> f64 {
    move || t.elapsed().as_secs_f64() * 1e3
}

pub fn run(a: Args, f: Flags) -> Result<(), Verdict> {
    let g = gpu(true)?;
    let cache = f.cache.as_deref().map(|dir| PipelineCache::open(&g.device, dir)).transpose()?;
    let bytes = read_bundle(&a.dir)?;
    let mut loads = vec![];
    let mut loft = None;
    for _ in 0..f.loads {
        let t = ms(Instant::now());
        loft = Some(load(&g, &bytes, Precision::F32, cache.as_ref().map(|(c, _)| c.raw()))?);
        loads.push(t());
    }
    let saved = cache.as_ref().map(|(c, _)| c.save()).transpose()?;
    let mut loft = loft.expect("--loads is at least 1");
    aim(&a, &mut loft)?;
    let target = Target::new(&g.device, a.size);
    target.picture(&g, &mut loft)?;
    let t = frames(&f, &g, &mut loft, &target)?;

    println!("adapter {} ({:?}); precision {:?}", g.info.name, g.info.backend, loft.precision());
    let [modules, render, compute] = loft.built();
    println!("built {modules} shader modules, {render} render and {compute} compute pipelines");
    println!("loads {}", loads.iter().map(|x| format!("{x:.1}")).collect::<Vec<_>>().join(" ") + " ms");
    let (cpu, gpu_q) = (quantiles(&t.cpu), (!t.gpu.is_empty()).then(|| quantiles(&t.gpu)));
    line(&format!("cpu x{}", f.frames), cpu);
    if let Some(q) = gpu_q {
        line("gpu frame", q);
        line("gpu kernel pass", quantiles(&t.pose));
        line("gpu passes", quantiles(&t.passes));
    }
    let report = json!({"profile": {
        "bundle": a.dir, "size": a.size, "adapter": g.info.name, "backend": format!("{:?}", g.info.backend),
        "precision": format!("{:?}", loft.precision()), "loads_ms": loads, "frames": f.frames,
        "frame_kernel_dispatches": loft.frame_dispatches(), "modules": modules, "render_pipelines": render, "compute_pipelines": compute,
        "cache": cache.as_ref().map(|(_, read)| json!({"dir": f.cache, "read": read, "saved": saved})),
        "cpu_ms": cpu, "gpu_ms": gpu_q, "quantiles": ["least", "median", "p95", "most"],
    }});
    println!("{report}");
    match (f.budget_ms, gpu_q) {
        (None, _) => Ok(()),
        (Some(limit), Some(q)) if q[2] <= limit => Ok(()),
        (Some(limit), Some(q)) => Err(Verdict::Budget(format!(
            "p95 GPU frame {:.3} ms is over --budget-ms {limit}: draw less, or raise the budget",
            q[2]
        ))),
        (Some(_), None) => {
            Err(Verdict::Environment("--budget-ms: the adapter grants no timestamp queries to measure GPU time".into()))
        }
    }
}

/// Per-frame milliseconds; the GPU vectors are empty without timestamp queries.
struct Times {
    cpu: Vec<f64>,
    gpu: Vec<f64>,
    pose: Vec<f64>,
    passes: Vec<f64>,
}

/// Timestamps resolved into a buffer, and the readable copy.
struct Clock {
    resolve: wgpu::Buffer,
    read: wgpu::Buffer,
    period_ns: f64,
}

impl Clock {
    fn new(g: &Gpu, n: u64) -> Clock {
        let buffer = |usage, label| {
            g.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(label),
                size: n * 8,
                usage,
                mapped_at_creation: false,
            })
        };
        Clock {
            resolve: buffer(wgpu::BufferUsages::QUERY_RESOLVE | wgpu::BufferUsages::COPY_SRC, "resolve"),
            read: buffer(wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST, "read"),
            period_ns: g.queue.get_timestamp_period() as f64,
        }
    }
    fn encode(&self, encoder: &mut wgpu::CommandEncoder, set: &wgpu::QuerySet, n: u32) {
        encoder.resolve_query_set(set, 0..n, &self.resolve, 0);
        encoder.copy_buffer_to_buffer(&self.resolve, 0, &self.read, 0, None);
    }
    /// After the submit: waits, and returns the ticks.
    fn ticks(&self, g: &Gpu) -> Result<Vec<u64>, Verdict> {
        self.read.map_async(wgpu::MapMode::Read, .., |r| r.expect("timestamp map"));
        g.device.poll(wgpu::PollType::wait_indefinitely()).map_err(|e| Verdict::Environment(format!("poll: {e}")))?;
        let t = self
            .read
            .get_mapped_range(..)
            .expect("timestamp range")
            .as_chunks::<8>()
            .0
            .iter()
            .map(|c| u64::from_le_bytes(*c))
            .collect();
        self.read.unmap();
        Ok(t)
    }
    fn ms(&self, a: u64, b: u64) -> f64 {
        b.wrapping_sub(a) as f64 * self.period_ns / 1e6
    }
}

fn frames(f: &Flags, g: &Gpu, loft: &mut Loft, target: &Target) -> Result<Times, Verdict> {
    let poll =
        || g.device.poll(wgpu::PollType::wait_indefinitely()).map_err(|e| Verdict::Environment(format!("poll: {e}")));
    // loft-gpu's timestamps: the kernel pass at 0 and 1, render pass k at 2 + 2k and 3 + 2k.
    let n = 2 + 2 * loft.bundle().passes.len() as u32;
    let clock = g.timing.then(|| {
        let set = g.device.create_query_set(&wgpu::QuerySetDescriptor {
            label: Some("loft frame"),
            ty: wgpu::QueryType::Timestamp,
            count: n,
        });
        (set, Clock::new(g, n as u64))
    });
    loft.set_timestamps(clock.as_ref().map(|(set, _)| set.clone()));
    let mut t = Times { cpu: vec![], gpu: vec![], pose: vec![], passes: vec![] };
    for _ in 0..f.frames {
        let start = Instant::now();
        let mut encoder = g.device.create_command_encoder(&Default::default());
        target.encode(g, loft, &mut encoder)?;
        g.queue.submit([encoder.finish()]);
        t.cpu.push(start.elapsed().as_secs_f64() * 1e3);
        // The frame first, then the resolve: resolving in the next submit without waiting read the last two passes'
        // end-of-fragment samples before Metal had written them (the previous frame's values came back).
        poll()?;
        if let Some((set, c)) = &clock {
            let mut e = g.device.create_command_encoder(&Default::default());
            c.encode(&mut e, set, n);
            g.queue.submit([e.finish()]);
            let k = c.ticks(g)?;
            let last = n as usize - 1;
            // An empty kernel pass or no pass at all writes no timestamps; resolved, they read 0.
            let (kernels, passes) = (c.ms(k[0], k[1]), if last > 1 { c.ms(k[2], k[last]) } else { 0.0 });
            t.pose.push(kernels);
            t.passes.push(passes);
            t.gpu.push(kernels + passes);
        }
    }
    Ok(t)
}
