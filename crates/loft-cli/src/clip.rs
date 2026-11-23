//! The training rollout contract (06 §17). Only a complete clip is published.
use crate::{
    budget::{self, Failure},
    flag, sources,
};
use loft_cli::{check_records, native::{self, Loaded}, write_png};
use loft_ir::Verdict;
use loft_passes::{emit::Aov, program::Pass};
use serde_json::json;
use std::{
    fs, io,
    path::{Path, PathBuf},
    process::Command,
    time::Instant,
};

struct Clip {
    scene: PathBuf,
    out: PathBuf,
    json: Option<PathBuf>,
    size: (u32, u32),
    frames: u32,
    fps: f64,
    start: f64,
    aov: Aov,
    live: Vec<(String, f32)>,
    max_build: Option<f64>,
    max_frame: Option<f64>,
}

fn number(name: &str, s: &str) -> Result<f64, Failure> {
    s.parse::<f64>()
        .ok()
        .filter(|n| n.is_finite() && *n >= 0.)
        .ok_or_else(|| Failure::Usage(format!("{name} requires a finite nonnegative number")))
}

impl Clip {
    fn parse(mut args: Vec<String>) -> Result<Self, Failure> {
        let dest = flag(&mut args, "--out")?.map(PathBuf::from);
        let mut required = |key| flag(&mut args, key)?.ok_or_else(|| Failure::Usage(format!("clip requires {key}")));
        let size = required("--size")?;
        let positive = |s: &str| s.parse::<u32>().ok().filter(|&x| x > 0);
        let size = size
            .split_once('x')
            .and_then(|(w, h)| Some((positive(w)?, positive(h)?)))
            .ok_or_else(|| Failure::Usage("--size requires positive WxH".into()))?;
        let frames = positive(&required("--frames")?)
            .ok_or_else(|| Failure::Usage("--frames requires a positive integer".into()))?;
        let fps = number("--fps", &required("--fps")?)?;
        let mut optional = |key| flag(&mut args, key)?.map(|v| number(key, &v)).transpose();
        let start = optional("--start")?.unwrap_or(0.);
        let max_build = optional("--max-build-ms")?;
        let max_frame = optional("--max-frame-ms")?;
        let json = flag(&mut args, "--json")?.map(std::path::absolute).transpose().map_err(output_error)?;
        let aov = flag(&mut args, "--aov")?.map_or(Ok(Aov::Beauty), |value| {
            value.parse().map_err(|e: &str| Failure::Usage(e.into()))
        })?;
        let mut live = Vec::new();
        while let Some(pair) = flag(&mut args, "--live")? {
            let (name, value) = pair.split_once('=').ok_or_else(|| Failure::Usage("--live requires PARAM=VALUE".into()))?;
            let value = value.parse::<f32>().ok().filter(|v| v.is_finite())
                .ok_or_else(|| Failure::Usage("--live requires a finite scalar".into()))?;
            live.push((name.to_owned(), value));
        }
        let [scene] = args.as_slice() else {
            return Err(Failure::Usage("clip expects one scene and one of each option".into()));
        };
        if scene.starts_with('-') || fps == 0. || !((start + f64::from(frames - 1) / fps) as f32).is_finite() {
            return Err(Failure::Usage("clip requires a scene and finite frame times at positive fps".into()));
        }
        let stem = Path::new(scene).file_stem().ok_or_else(|| Failure::Usage("clip requires a scene filename".into()))?;
        let out = std::path::absolute(dest.unwrap_or_else(|| Path::new("out/clip").join(stem))).map_err(output_error)?;
        Ok(Self { scene: scene.into(), out, json, size, frames, fps, start, aov, live, max_build, max_frame })
    }
}

fn output_error(e: io::Error) -> Failure {
    Failure::Usage(e.to_string())
}

fn limit(label: &str, ms: f64, cap: Option<f64>) -> Result<(), Failure> {
    if let Some(cap) = cap
        && ms > cap
    {
        return Err(Failure::Budget(format!("{label} {ms:.3} ms exceeds {cap:.3} ms")));
    }
    Ok(())
}

fn frame_name(i: u32) -> String {
    format!("frame_{i:03}.png")
}

// Nearest rank. Once more than floor(N/20) samples exceed the limit, even
// zero-time remaining frames could not rescue this rollout's p95.
fn rank(n: usize, percent: usize) -> usize {
    (n as u64 * percent as u64).div_ceil(100) as usize - 1
}

fn observe_frame(ms: f64, cap: f64, frames: u32, slow: &mut u32) -> Result<(), Failure> {
    if !ms.is_finite() || ms < 0. {
        return Err(Failure::Device("invalid GPU timestamp interval".into()));
    }
    *slow += u32::from(ms > cap);
    if *slow > frames / 20 {
        return Err(Failure::Budget(format!("GPU p95 exceeds {cap} ms ({slow} over-budget frames)")));
    }
    Ok(())
}

pub(crate) fn run(args: &[String], set: &[(String, f64)]) -> Result<(), Failure> {
    let c = Clip::parse(args.to_vec())?;
    let began = Instant::now();
    let src = sources(&c.scene).map_err(Failure::Usage)?;
    let (p, _, compiled) = loft_passes::compile_aov(&src, set, c.aov).map_err(|d| Failure::Compile(d.render(&src)))?;
    let mut build_ms = began.elapsed().as_secs_f64() * 1000.;
    limit("CPU build", build_ms, c.max_build)?;
    let g = native::gpu().map_err(Failure::Device)?;
    if c.max_frame.is_some() {
        budget::timestamps(g.timestamps)?;
    }
    let (w, h) = c.size;
    let caps = g.device.limits();
    if w.max(h) > caps.max_texture_dimension_2d
        || (u64::from(w) * 4).next_multiple_of(256) * u64::from(h) > caps.max_buffer_size
    {
        return Err(Failure::Usage("--size exceeds device limits".into()));
    }

    let parent = c.out.parent().ok_or_else(|| Failure::Usage("--out requires a file or directory name".into()))?;
    fs::create_dir_all(parent).map_err(output_error)?;
    let stage = tempfile::tempdir_in(parent).map_err(output_error)?;
    let live = c.live.iter().map(|(name, value)| compiled.prog().param_word(name)
        .map(|word| (word, *value)).ok_or_else(|| Failure::Usage(format!("no live scalar param {name}"))))
        .collect::<Result<Vec<_>, _>>()?;
    let mut load_ms = 0.;
    let mut load = |program: &loft_passes::program::Program| {
        let start = Instant::now();
        let loaded = Loaded::with_aov(&g, program, c.size, None, c.aov).map_err(Failure::Device)?;
        for &(word, value) in &live { loaded.set_param_word(word, value).map_err(Failure::Usage)?; }
        load_ms += start.elapsed().as_secs_f64() * 1000.;
        Ok::<_, Failure>(loaded)
    };
    let mut loaded = None;
    let mut times = vec![];
    let mut motion = vec![];
    let mut frame_timings = vec![];
    let mut slow = 0u32;
    let mut verdicts = vec![];
    let mut commands = 0;
    let mut pipelines = 0;
    for i in 0..c.frames {
        let t = (c.start + f64::from(i) / c.fps) as f32;
        let (samples, motion_ms) = native::sample_motion_timed(&g, &p, t).map_err(Failure::Device)?;
        for (name, verdict) in check_records(&p, &samples) {
            let Verdict::Held { margin } = verdict else {
                return Err(Failure::Truth(format!("frame {i} {name}: {verdict}")));
            };
            verdicts.push(json!({"frame": i, "check": name, "held": margin}));
        }
        if !p.passes.contains(&Pass::Present) {
            return Err(Failure::Parity(format!("missing frame {i}: schedule does not present")));
        }
        if samples.iter().any(|s| !s.fields.is_empty()) {
            let start = Instant::now();
            let (snapshot, _, _) =
                loft_passes::compile_snapshot_aov(&src, set, &samples, c.aov).map_err(|d| Failure::Compile(d.render(&src)))?;
            build_ms += start.elapsed().as_secs_f64() * 1000.;
            limit("CPU build", build_ms, c.max_build)?;
            loaded = Some(load(&snapshot)?);
        } else if loaded.is_none() {
            loaded = Some(load(&p)?);
        }
        let frame = loaded.as_ref().expect("loaded frame");
        pipelines = frame.stats.render_pipelines + frame.stats.compute_pipelines + p.dispatches.len();
        commands = frame.frame(&g, t) as usize + p.dispatches.len();
        if let Some(mut timing) = frame.gpu_timing(&g).map_err(Failure::Device)? {
            timing.serial = i as usize;
            timing.commands = commands as u32;
            motion.push(motion_ms);
            let ms = motion_ms + timing.gpu_ms;
            frame_timings.push(timing);
            times.push(ms);
            if let Some(cap) = c.max_frame {
                observe_frame(ms, cap, c.frames, &mut slow)?;
            }
        }
        let pixels = frame.pixels(&g).map_err(Failure::Device)?;
        if pixels.len() as u64 != u64::from(w) * u64::from(h) * 4 {
            return Err(Failure::Parity(format!("frame {i} has incomplete RGBA pixels")));
        }
        write_png(&stage.path().join(frame_name(i)), c.size, &pixels).map_err(Failure::Parity)?;
    }
    times.sort_by(f64::total_cmp);
    let quantile = |percent| (!times.is_empty()).then(|| times[rank(times.len(), percent)]);
    let mut hash = blake3::Hasher::new();
    serde_json::to_writer(
        &mut hash,
        &(
            "loft-clip-v1/f32/rgba8srgb",
            &src.files,
            set,
            c.size,
            c.start,
            c.frames,
            c.fps,
            format!("{:?}", c.aov),
            &c.live,
            p.modules.iter().map(|m| &m.wgsl).collect::<Vec<_>>(),
        ),
    )
    .map_err(|e| Failure::Compile(e.to_string()))?;
    let meta = json!({
        "frame_timings": frame_timings,
        // GPU ms of each frame's motion: the fixed-step dispatches from tick 0 to the frame's tick.
        "motion_gpu_ms": motion,
        "build_cpu_ms": build_ms, "load_gpu_ms": load_ms,
        "frame_gpu_p50_ms": quantile(50), "frame_gpu_p95_ms": quantile(95),
        "modules": p.modules.len(), "pipelines": pipelines, "commands": commands,
        "verdicts": verdicts, "identity_hash": hash.finalize().to_hex().as_str(),
    });
    publish(&c, stage.path(), &meta)
}

fn publish(c: &Clip, stage: &Path, meta: &serde_json::Value) -> Result<(), Failure> {
    for i in 0..c.frames {
        if !stage.join(frame_name(i)).is_file() {
            return Err(Failure::Parity(format!("missing frame {i}")));
        }
    }
    let video = c.out.extension().is_some_and(|s| s == "mp4");
    let target = if video {
        let mp4 = stage.join("clip.mp4");
        let status = Command::new("ffmpeg")
            .args(["-nostdin", "-v", "error", "-framerate", &c.fps.to_string(), "-i"])
            .arg(stage.join("frame_%03d.png"))
            .args([
                "-vf",
                "pad=ceil(iw/2)*2:ceil(ih/2)*2",
                "-pix_fmt",
                "yuv420p",
                "-c:v",
                "libsvtav1",
                "-preset",
                "10",
                "-crf",
                "42",
            ])
            .arg(&mp4)
            .status()
            .map_err(|e| Failure::Device(format!("ffmpeg: {e}")))?;
        if !status.success() {
            return Err(Failure::Device(format!("ffmpeg: {status}")));
        }
        mp4
    } else {
        stage.into()
    };
    let json = c.json.clone().or_else(|| (!video).then(|| c.out.join("meta.json")));
    let mut sidecar = None;
    if let Some(path) = &json {
        let data = serde_json::to_vec_pretty(meta).map_err(|e| Failure::Usage(e.to_string()))?;
        if let Ok(rel) = path.strip_prefix(&c.out) {
            if video
                || rel.as_os_str().is_empty()
                || rel.components().any(|c| !matches!(c, std::path::Component::Normal(_)))
            {
                return Err(Failure::Usage("--json must name a distinct metadata file".into()));
            }
            let staged = stage.join(rel);
            fs::create_dir_all(staged.parent().unwrap()).map_err(output_error)?;
            use std::io::Write;
            fs::File::create_new(staged).and_then(|mut f| f.write_all(&data)).map_err(output_error)?;
        } else {
            let parent = path.parent().ok_or_else(|| Failure::Usage("--json requires a filename".into()))?;
            fs::create_dir_all(parent).map_err(output_error)?;
            let file = tempfile::NamedTempFile::new_in(parent).map_err(output_error)?;
            fs::write(file.path(), data).map_err(output_error)?;
            sidecar = Some(file);
        }
    }
    // A directory rename publishes every frame together, including in-directory JSON.
    fs::rename(target, &c.out).map_err(|e| Failure::Usage(format!("publish {}: {e}", c.out.display())))?;
    if let (Some(file), Some(path)) = (sidecar, json) {
        file.persist(path).map_err(|e| output_error(e.error))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_budget_and_earliest_decisive_p95() {
        limit("CPU build", 2000., Some(2000.)).unwrap();
        assert_eq!(limit("CPU build", 2000.01, Some(2000.)).unwrap_err().exit().0, 7);
        for n in [1, 16, 19, 20, 21, 100] {
            let mut slow = 0;
            observe_frame(50., 50., n, &mut slow).unwrap();
            for _ in 0..n / 20 {
                observe_frame(51., 50., n, &mut slow).unwrap();
            }
            assert_eq!(observe_frame(51., 50., n, &mut slow).unwrap_err().exit().0, 7);
            assert_eq!(rank(n as usize, 95), (n - slow) as usize);
        }
        assert_eq!(budget::timestamps(false).unwrap_err().exit().0, 6);
    }

    #[test]
    fn missing_frame_is_three_and_never_publishes_partial_output() {
        let root = tempfile::tempdir().unwrap();
        let stage = tempfile::tempdir_in(root.path()).unwrap();
        fs::write(stage.path().join(frame_name(0)), b"first").unwrap();
        let out = root.path().join("clip");
        let c = Clip::parse(
            ["scene.loft", "--out", out.to_str().unwrap(), "--size", "1x1", "--frames", "2", "--fps", "16"]
                .map(str::to_owned)
                .to_vec(),
        )
        .unwrap();
        assert_eq!(publish(&c, stage.path(), &json!({})).unwrap_err().exit().0, 3);
        assert!(!out.exists());
    }
}
