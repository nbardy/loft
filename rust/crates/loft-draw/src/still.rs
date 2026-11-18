//! `loft-draw still`: one picture of a built directory, and the timings `profile` reports (moved from loft-view).
//!
//!   loft-draw still <dir> --view game --time 1.0 --size 1280x800 [--out f.png] [--input name=v[,v...]] [--param name=v]
//!                   [--frames N [--wait]] [--pose-time]
//!
//! The picture waits for its own level-of-detail bounds (as `loft render` does). `--frames N` then draws N frames of the
//! same view as a game would (planning with the last bounds read; `--wait` makes each wait for its own) and prints the
//! CPU time inside the loader and, with timestamp queries, the GPU time as `loft profile`'s "normal frame" defines it:
//! the pose pass plus the render passes, the gap between the two submits not counted. `--pose-time`: the pose stages
//! alone, as tools/pose_time.ts measures them (40 repeats in one timed pass, least of 8). The Vulkan pipeline cache and
//! repeated loads that loft-view also timed move here with lane 4.2.

use crate::{gpu, Gpu, Target, Verdict};
use loft_loader::{read_dir, Bounds, Loft, Timestamps};
use std::path::PathBuf;
use std::time::Instant;

pub struct Args {
    dir: PathBuf,
    view: String,
    time: f64,
    size: (u32, u32),
    out: Option<PathBuf>,
    inputs: Vec<(String, Vec<f32>)>,
    params: Vec<(String, f32)>,
    frames: usize,
    wait: bool,
    pose_time: bool,
}

fn usage(msg: String) -> ! {
    eprintln!("loft-draw still: {msg}");
    std::process::exit(2)
}

pub fn parse(mut it: impl Iterator<Item = String>) -> Args {
    let dir = it.next().map(PathBuf::from).unwrap_or_else(|| usage("expected a built directory".into()));
    let mut a = Args {
        dir,
        view: "game".into(),
        time: 1.0,
        size: (1280, 800),
        out: None,
        inputs: vec![],
        params: vec![],
        frames: 0,
        wait: false,
        pose_time: false,
    };
    let num = |s: &str, flag: &str| s.parse::<f64>().unwrap_or_else(|_| usage(format!("{flag}: not a number: {s}")));
    while let Some(flag) = it.next() {
        let mut value = || it.next().unwrap_or_else(|| usage(format!("{flag} needs a value")));
        match flag.as_str() {
            "--view" => a.view = value(),
            "--time" => a.time = num(&value(), "--time"),
            "--size" => {
                let v = value();
                let (w, h) = v.split_once('x').unwrap_or_else(|| usage(format!("--size: expected WxH, got {v}")));
                a.size = (num(w, "--size") as u32, num(h, "--size") as u32);
            }
            "--out" => a.out = Some(value().into()),
            "--input" | "--param" => {
                let v = value();
                let (name, vals) =
                    v.split_once('=').unwrap_or_else(|| usage(format!("{flag}: expected name=value, got {v}")));
                let vals: Vec<f32> = vals.split(',').map(|x| num(x, &flag) as f32).collect();
                match flag.as_str() {
                    "--input" => a.inputs.push((name.into(), vals)),
                    _ => a.params.push((name.into(), vals[0])),
                }
            }
            "--frames" => a.frames = num(&value(), "--frames") as usize,
            "--wait" => a.wait = true,
            "--pose-time" => a.pose_time = true,
            _ => usage(format!("unknown flag {flag}")),
        }
    }
    a
}

fn stats(xs: &mut [f64]) -> String {
    xs.sort_by(f64::total_cmp);
    let q = |p: f64| xs[((xs.len() - 1) as f64 * p).round() as usize];
    format!("least {:.3}  median {:.3}  most {:.3} ms", q(0.0), q(0.5), q(1.0))
}

pub fn run(a: Args) -> Result<(), Verdict> {
    let g = gpu(true)?;
    eprintln!("adapter {} ({:?}); timestamp queries {}", g.info.name, g.info.backend, g.timing);
    let t = Instant::now();
    let mut loft = Loft::load(&g.device, &g.queue, read_dir(&a.dir)?)?;
    eprintln!("load {:.3} ms, precision {}", t.elapsed().as_secs_f64() * 1e3, loft.precision().as_str());
    loft.set_camera(loft.camera(&a.view)?);
    loft.set_time(a.time);
    for (name, v) in &a.inputs {
        loft.set_input(name, v)?;
    }
    for (name, v) in &a.params {
        loft.set_param(name, *v)?;
    }
    let target = Target::new(&g.device, a.size);
    let (rgba, drawn) = target.picture(&g, &mut loft)?;
    eprintln!("frame: {} triangles in {} draw calls", drawn.triangles, drawn.draws);
    if let Some(out) = &a.out {
        let fail = |e: &dyn std::fmt::Display| Verdict::Environment(format!("{}: {e}", out.display()));
        let file = std::fs::File::create(out).map_err(|e| fail(&e))?;
        let mut png = png::Encoder::new(std::io::BufWriter::new(file), a.size.0, a.size.1);
        png.set_filter(png::Filter::NoFilter); // tools/picdiff.ts reads unfiltered rows only
        png.set_color(png::ColorType::Rgba);
        png.set_depth(png::BitDepth::Eight);
        png.write_header().and_then(|mut w| w.write_image_data(&rgba)).map_err(|e| fail(&e))?;
    }
    if a.frames > 0 {
        frames(&a, &g, &mut loft, &target)?;
    }
    if a.pose_time {
        pose_time(&g, &loft)?;
    }
    Ok(())
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
            g.device.create_buffer(&wgpu::BufferDescriptor { label: Some(label), size: n * 8, usage, mapped_at_creation: false })
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
        let t = self.read.get_mapped_range(..).expect("timestamp range").as_chunks::<8>().0.iter().map(|c| u64::from_le_bytes(*c)).collect();
        self.read.unmap();
        Ok(t)
    }
    fn ms(&self, a: u64, b: u64) -> f64 {
        b.wrapping_sub(a) as f64 * self.period_ns / 1e6
    }
}

fn frames(a: &Args, g: &Gpu, loft: &mut Loft, target: &Target) -> Result<(), Verdict> {
    let poll = || g.device.poll(wgpu::PollType::wait_indefinitely()).map_err(|e| Verdict::Environment(format!("poll: {e}")));
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
    let bounds = if a.wait { Bounds::Latest } else { Bounds::Last };
    let (mut cpu, mut gpu, mut pose, mut passes) = (vec![], vec![], vec![], vec![]);
    for _ in 0..a.frames {
        let t = Instant::now();
        let mut encoder = g.device.create_command_encoder(&Default::default());
        let s = loft.encode_frame(&g.device, &g.queue, &mut encoder, &target.view, a.size.0, a.size.1, bounds)?;
        // A waiting frame copies its picture out, as a headless frame (whose GPU time profile reports) does.
        if a.wait {
            target.copy(&mut encoder);
        }
        g.queue.submit([encoder.finish()]);
        cpu.push((t.elapsed() - s.gpu_wait).as_secs_f64() * 1e3);
        // The frame first, then the resolve: resolving in the next submit without waiting read the last two passes'
        // end-of-fragment samples before Metal had written them (the previous frame's values came back).
        poll()?;
        if let (Some(c), Some(ts)) = (&clock, loft.timestamps()) {
            let mut e = g.device.create_command_encoder(&Default::default());
            c.encode(&mut e, &ts.set, n);
            g.queue.submit([e.finish()]);
            let t = c.ticks(g)?;
            let last = n as usize - 1;
            pose.push(c.ms(t[0], t[1]));
            passes.push(c.ms(t[2], t[last]));
            gpu.push(c.ms(t[0], t[1]) + c.ms(t[2], t[last]));
        }
    }
    // The first frames include first-use costs (pipeline residency, the first bounds read): reported, not hidden.
    println!("frames {}  ({})", a.frames, if a.wait { "each waits for its own bounds" } else { "planning with the last bounds read" });
    println!("cpu  per frame  {}", stats(&mut cpu.clone()));
    println!("cpu  frames 10+ {}", stats(&mut cpu[10.min(cpu.len() - 1)..].to_vec()));
    if !gpu.is_empty() {
        println!("gpu  per frame  {}  (pose pass + render passes, as profile's normal frame)", stats(&mut gpu));
        println!("gpu  pose pass  {}", stats(&mut pose));
        println!("gpu  passes     {}", stats(&mut passes));
    }
    Ok(())
}

fn pose_time(g: &Gpu, loft: &Loft) -> Result<(), Verdict> {
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
    println!("pose stages {:.3} ms GPU (least of {rounds} rounds of {repeats} repeats in one pass)", time(rounds)?);
    Ok(())
}
