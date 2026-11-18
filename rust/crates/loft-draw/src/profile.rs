//! `loft-draw profile`: the load and frame timings the gates read (plan 2026-11-19 §8.1 frame-time gate, §7 frame
//! budgets; lane 4.2). Moved from `still --frames` (loft-view), with loft-view's repeated loads and Vulkan cache.
//!
//!   loft-draw profile <dir> [still's view flags] [--frames N] [--wait] [--pose-time] [--loads N]
//!                     [--pipeline-cache DIR] [--budget-ms MS]
//!
//! - `--loads N` (default 1) loads the directory N times on one device and times each load. With `--pipeline-cache DIR`
//!   every load compiles through one pipeline cache, read from DIR first and saved there after the last load. That is
//!   wgpu's `PIPELINE_CACHE`, which only Vulkan grants: another backend is refused as `environment` (Metal has its OS
//!   cache, which the second of `--loads 2` shows).
//! - `--frames N` (default 16) draws N frames of the view as a game would (planning with the last bounds read; `--wait`
//!   makes each wait for its own). CPU is the time inside the loader; GPU, with timestamp queries, is `loft profile`'s
//!   "normal frame": the pose pass plus the render passes, the gap between the two submits not counted.
//! - `--pose-time`: the pose stages alone, as tools/pose_time.ts measures them (40 repeats in one pass, least of 8).
//! - `--budget-ms MS`: a p95 GPU frame over MS exits 7 (`budget`) after the report.
//!
//! The last stdout line is one JSON object, `{"profile": {...}}`, which tools/parity/measure.ts reads.

use crate::still::{Args, aim};
use crate::{Gpu, Target, Verdict, gpu};
use loft_loader::{Bounds, Loft, PipelineCache, Timestamps, read_dir};
use serde_json::json;
use std::path::{Path, PathBuf};
use std::time::Instant;

pub struct Flags {
    pub frames: usize,
    pub wait: bool,
    pub pose_time: bool,
    pub loads: usize,
    pub cache: Option<PathBuf>,
    pub budget_ms: Option<f64>,
}

impl Default for Flags {
    fn default() -> Flags {
        Flags { frames: 16, wait: false, pose_time: false, loads: 1, cache: None, budget_ms: None }
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

/// `PipelineCache::open`'s contract: DIR holds only bytes this API saved. `--pipeline-cache` names a directory kept
/// for that alone. Lane 4.1's safe `open` (an exclusive flock on DIR) replaces this function.
#[allow(unsafe_code)]
fn open_cache(g: &Gpu, dir: &Path) -> Result<(PipelineCache, usize), Verdict> {
    // SAFETY: the contract above; loft-draw writes DIR only through `PipelineCache::save`.
    Ok(unsafe { PipelineCache::open(&g.device, dir) }?)
}

pub fn run(a: Args, f: Flags) -> Result<(), Verdict> {
    let g = gpu(true)?;
    let cache = f.cache.as_deref().map(|dir| open_cache(&g, dir)).transpose()?;
    let mut loads = vec![];
    let mut loft = None;
    for _ in 0..f.loads {
        let (t, artefact) = (ms(Instant::now()), read_dir(&a.dir)?);
        loft = Some(match &cache {
            Some((c, _)) => c.warm(artefact)?.load(&g.queue)?,
            None => Loft::load(&g.device, &g.queue, artefact)?,
        });
        loads.push(t());
    }
    let saved = cache.as_ref().map(|(c, _)| c.save()).transpose()?;
    let mut loft = loft.expect("--loads is at least 1");
    aim(&a, &mut loft)?;
    let target = Target::new(&g.device, a.size);
    target.picture(&g, &mut loft)?; // its own bounds first, as `still`
    let t = frames(&a, &f, &g, &mut loft, &target)?;
    let pose = f.pose_time.then(|| pose_time(&g, &loft)).transpose()?;

    println!("adapter {} ({:?}); precision {}", g.info.name, g.info.backend, loft.precision().as_str());
    println!("loads {}", loads.iter().map(|x| format!("{x:.1}")).collect::<Vec<_>>().join(" ") + " ms");
    let (cpu, gpu_q) = (quantiles(&t.cpu), (!t.gpu.is_empty()).then(|| quantiles(&t.gpu)));
    line(&format!("cpu x{}", f.frames), cpu);
    if let Some(q) = gpu_q {
        line("gpu frame", q);
        line("gpu pose pass", quantiles(&t.pose));
        line("gpu passes", quantiles(&t.passes));
    }
    if let Some(p) = pose {
        println!("pose stages {p:.3} ms GPU (least of 8 rounds of 40 repeats in one pass)");
    }
    let report = json!({"profile": {
        "dir": a.dir, "size": a.size, "adapter": g.info.name, "backend": format!("{:?}", g.info.backend),
        "precision": loft.precision().as_str(), "loads_ms": loads, "wait": f.wait, "frames": f.frames,
        "cache": cache.as_ref().map(|(_, read)| json!({"dir": f.cache, "read": read, "saved": saved})),
        "cpu_ms": cpu, "gpu_ms": gpu_q, "pose_stages_ms": pose, "quantiles": ["least", "median", "p95", "most"],
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

fn frames(a: &Args, f: &Flags, g: &Gpu, loft: &mut Loft, target: &Target) -> Result<Times, Verdict> {
    let poll =
        || g.device.poll(wgpu::PollType::wait_indefinitely()).map_err(|e| Verdict::Environment(format!("poll: {e}")));
    let n = 2 + 2 * loft.pass_labels().count() as u32;
    let clock = g.timing.then(|| {
        let set = g.device.create_query_set(&wgpu::QuerySetDescriptor {
            label: Some("loft frame"),
            ty: wgpu::QueryType::Timestamp,
            count: n,
        });
        loft.set_timestamps(Some(Timestamps { set }));
        Clock::new(g, n as u64)
    });
    let bounds = if f.wait { Bounds::Latest } else { Bounds::Last };
    let mut t = Times { cpu: vec![], gpu: vec![], pose: vec![], passes: vec![] };
    for _ in 0..f.frames {
        let start = Instant::now();
        let mut encoder = g.device.create_command_encoder(&Default::default());
        let s = loft.encode_frame(&g.device, &g.queue, &mut encoder, &target.view, a.size.0, a.size.1, bounds)?;
        // A waiting frame copies its picture out, as a headless frame (whose GPU time profile reports) does.
        if f.wait {
            target.copy(&mut encoder);
        }
        g.queue.submit([encoder.finish()]);
        t.cpu.push((start.elapsed() - s.gpu_wait).as_secs_f64() * 1e3);
        // The frame first, then the resolve: resolving in the next submit without waiting read the last two passes'
        // end-of-fragment samples before Metal had written them (the previous frame's values came back).
        poll()?;
        if let (Some(c), Some(ts)) = (&clock, loft.timestamps()) {
            let mut e = g.device.create_command_encoder(&Default::default());
            c.encode(&mut e, &ts.set, n);
            g.queue.submit([e.finish()]);
            let k = c.ticks(g)?;
            let last = n as usize - 1;
            t.pose.push(c.ms(k[0], k[1]));
            t.passes.push(c.ms(k[2], k[last]));
            t.gpu.push(c.ms(k[0], k[1]) + c.ms(k[2], k[last]));
        }
    }
    Ok(t)
}

fn pose_time(g: &Gpu, loft: &Loft) -> Result<f64, Verdict> {
    if !g.timing {
        return Err(Verdict::Environment("--pose-time: the adapter grants no timestamp queries".into()));
    }
    let set = g.device.create_query_set(&wgpu::QuerySetDescriptor {
        label: Some("pose time"),
        ty: wgpu::QueryType::Timestamp,
        count: 2,
    });
    let clock = Clock::new(g, 2);
    let (repeats, rounds) = (40, 8);
    let time = |n: usize| -> Result<f64, Verdict> {
        let mut best = f64::INFINITY;
        for _ in 0..n {
            let mut e = g.device.create_command_encoder(&Default::default());
            {
                let mut pass = e.begin_compute_pass(&wgpu::ComputePassDescriptor {
                    label: Some("pose x repeats"),
                    timestamp_writes: Some(wgpu::ComputePassTimestampWrites {
                        query_set: &set,
                        beginning_of_pass_write_index: Some(0),
                        end_of_pass_write_index: Some(1),
                    }),
                });
                for _ in 0..repeats {
                    loft.encode_pose(&mut pass);
                }
            }
            clock.encode(&mut e, &set, 2);
            g.queue.submit([e.finish()]);
            let t = clock.ticks(g)?;
            best = best.min(clock.ms(t[0], t[1]) / repeats as f64);
        }
        Ok(best)
    };
    time(4)?; // warm-up, as pose_time.ts
    time(rounds)
}
