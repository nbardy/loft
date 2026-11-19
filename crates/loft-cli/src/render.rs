//! Source rendering shared by stills and clips.
use crate::{budget, budget::Failure, flag, gates::write_png, sources};
use loft_cli::native::{self, Gpu, Loaded};
use loft_passes::emit::Aov;
use std::{path::{Path, PathBuf}, process::Command, time::Instant};

#[derive(Clone, Copy)]
pub(crate) enum Output {
    Still,
    Clip,
}

struct Options {
    scene: PathBuf,
    out: PathBuf,
    size: (u32, u32),
    samples: u32,
    start: f32,
    frames: u32,
    fps: f32,
    aov: Aov,
    engine: Option<String>,
    live: Vec<(String, f32)>,
    view: Option<String>,
    max_build: Option<f64>,
    max_frame: Option<f64>,
}

impl Options {
    fn read(args: &[String], output: Output) -> Result<Self, Failure> {
        let mut args = args.to_vec();
        let required = |value: Option<String>, opt: &str| value.ok_or_else(|| Failure::Usage(format!("{opt} is required")));
        let out = PathBuf::from(required(flag(&mut args, "--out")?, "--out")?);
        let positive = |s: &str| s.parse::<u32>().ok().filter(|x| *x > 0);
        let size = flag(&mut args, "--size")?.map_or(Ok((1280, 800)), |value| {
            value.split_once('x').and_then(|(w, h)| Some((positive(w)?, positive(h)?)))
                .ok_or_else(|| Failure::Usage("--size requires positive WxH dimensions".into()))
        })?;
        let finite = |value: String, option: &str| value.parse::<f64>().ok().filter(|x| x.is_finite() && *x >= 0.0)
            .ok_or_else(|| Failure::Usage(format!("{option} requires a nonnegative finite number")));
        let samples = flag(&mut args, "--samples")?.map_or(Ok(1), |s| positive(&s).ok_or_else(|| Failure::Usage("--samples requires a positive count".into())))?;
        let aov = flag(&mut args, "--aov")?.map_or(Ok(Aov::Beauty), |value| value.parse().map_err(|e: &str| Failure::Usage(e.into())))?;
        let engine = flag(&mut args, "--engine")?;
        let view = flag(&mut args, "--views")?;
        let mut live = Vec::new();
        while let Some(pair) = flag(&mut args, "--live")? {
            let (name, value) = pair.split_once('=').ok_or_else(|| Failure::Usage("--live requires PARAM=VALUE".into()))?;
            let value = value.parse::<f32>().ok().filter(|x| x.is_finite())
                .ok_or_else(|| Failure::Usage("--live requires a finite scalar".into()))?;
            live.push((name.into(), value));
        }
        let max_build = flag(&mut args, "--max-build-ms")?.map(|v| finite(v, "--max-build-ms")).transpose()?;
        let max_frame = flag(&mut args, "--max-frame-ms")?.map(|v| finite(v, "--max-frame-ms")).transpose()?;
        let (start, frames, fps) = match output {
            Output::Still => {
                let time = flag(&mut args, "--time")?.map_or(Ok(0.), |v| finite(v, "--time"))?;
                (time as f32, 1, 1.)
            }
            Output::Clip => {
                let start = flag(&mut args, "--start")?.map_or(Ok(0.), |v| finite(v, "--start"))?;
                let frames = required(flag(&mut args, "--frames")?, "--frames")?;
                let frames = positive(&frames).ok_or_else(|| Failure::Usage("--frames requires a positive count".into()))?;
                let fps = finite(required(flag(&mut args, "--fps")?, "--fps")?, "--fps")? as f32;
                if fps <= 0.0 || !fps.is_finite() { return Err(Failure::Usage("--fps requires a positive finite rate".into())); }
                (start as f32, frames, fps)
            }
        };
        let [scene] = args.as_slice() else { return Err(Failure::Usage("supply one scene and the output options".into())); };
        if scene.starts_with('-') || !start.is_finite() || !(start + (frames - 1) as f32 / fps).is_finite() {
            return Err(Failure::Usage("expected a scene path and finite frame times".into()));
        }
        Ok(Self { scene: scene.into(), out, size, samples, start, frames, fps, aov, engine, live, view, max_build, max_frame })
    }
}

pub(crate) fn run(args: &[String], set: &[(String, f64)], output: Output) -> Result<(), Failure> {
    let opts = Options::read(args, output)?;
    let began = Instant::now();
    let mut src = sources(&opts.scene).map_err(Failure::Usage)?;
    if let Some(engine) = &opts.engine {
        if !matches!(engine.as_str(), "realtime" | "reference" | "woodblock" | "spectral_caustic") {
            return Err(Failure::Usage("unknown engine".into()));
        }
        src.add("<engine selection>", format!("use engines::{engine}"));
    }
    let (program, _, compiled) = loft_passes::compile_aov(&src, set, opts.aov)
        .map_err(|d| Failure::Compile(d.render(&src)))?;
    if let Some(limit) = opts.max_build {
        let ms = began.elapsed().as_secs_f64() * 1000.0;
        if ms > limit { return Err(Failure::Budget(format!("CPU build {ms:.3} ms exceeds {limit:.3} ms"))); }
    }
    if let Some(view) = &opts.view {
        use loft_parse::ast::Item;
        let name = compiled.modules.iter().flat_map(|m| &m.items).find_map(|item| {
            if let Item::Camera(c) = item { Some(c.name.name.as_str()) } else { None }
        }).unwrap_or("Main");
        if !view.eq_ignore_ascii_case(name) {
            return Err(Failure::Usage(format!("the compiled scene exports one view, {name}")));
        }
    }
    let prog = compiled.prog();
    let live = opts.live.iter().map(|(name, value)| {
        Ok((prog.param_word(name).ok_or_else(|| Failure::Usage(format!("no live scalar param {name}")))?, *value))
    }).collect::<Result<Vec<_>, Failure>>()?;
    let g = native::gpu().map_err(Failure::Device)?;
    if opts.max_frame.is_some() { budget::timestamps(g.timestamps)?; }
    let loaded = Loaded::with_aov(&g, &program, opts.size, None, opts.aov).map_err(Failure::Device)?;
    for (word, value) in live { loaded.set_param_word(word, value).map_err(Failure::Usage)?; }
    if let Some(ms) = loaded.stats.load_bake_gpu_ms {
        println!("load bake and mips {ms:.3} ms GPU, {} dispatches", loaded.stats.load_bake_commands);
    }
    match output {
        Output::Still => still(&g, &loaded, &opts),
        Output::Clip => clip(&g, &loaded, &opts),
    }
}

fn frame(g: &Gpu, loaded: &Loaded, opts: &Options, t: f32) -> Result<(Vec<u8>, Option<f64>), Failure> {
    loaded.frame_samples(g, t, opts.samples);
    let time = if opts.max_frame.is_some() {
        Some(loaded.gpu_ms(g).map_err(Failure::Device)?.ok_or_else(|| Failure::Device("GPU timestamps are unavailable".into()))?.0)
    } else { None };
    Ok((loaded.pixels(g).map_err(Failure::Device)?, time))
}

fn still(g: &Gpu, loaded: &Loaded, opts: &Options) -> Result<(), Failure> {
    let (pixels, ms) = frame(g, loaded, opts, opts.start)?;
    if let Some((ms, limit)) = ms.zip(opts.max_frame) { budget::measured(ms, limit)?; }
    match opts.out.extension().and_then(|x| x.to_str()) {
        Some("png") => write_png(&opts.out, opts.size, &pixels).map_err(Failure::Usage),
        Some("webp") => {
            let temp = tempfile::tempdir().map_err(|e| Failure::Usage(e.to_string()))?;
            let path = temp.path().join("still.png");
            write_png(&path, opts.size, &pixels).map_err(Failure::Usage)?;
            create_parent(&opts.out)?;
            ffmpeg(Command::new("ffmpeg").args(["-y", "-v", "error", "-i"]).arg(path)
                .args(["-c:v", "libwebp", "-quality", "95"]).arg(&opts.out))
        }
        _ => Err(Failure::Usage("render --out requires a .png or .webp file".into())),
    }
}

fn clip(g: &Gpu, loaded: &Loaded, opts: &Options) -> Result<(), Failure> {
    let video = opts.out.extension().is_some_and(|s| s == "mp4");
    let temp = video.then(tempfile::tempdir).transpose().map_err(|e| Failure::Usage(e.to_string()))?;
    let dir = temp.as_ref().map_or(opts.out.as_path(), tempfile::TempDir::path);
    let mut times = Vec::new();
    for i in 0..opts.frames {
        let (pixels, ms) = frame(g, loaded, opts, opts.start + i as f32 / opts.fps)?;
        let path = dir.join(format!("{i:06}.png"));
        write_png(&path, opts.size, &pixels).map_err(Failure::Usage)?;
        if let Some(ms) = ms { times.push(ms); }
    }
    if let Some(limit) = opts.max_frame {
        times.sort_by(f64::total_cmp);
        budget::measured(times[(times.len() * 95).div_ceil(100) - 1], limit)?;
    }
    if video {
        create_parent(&opts.out)?;
        ffmpeg(Command::new("ffmpeg").args(["-y", "-v", "error", "-framerate", &opts.fps.to_string(), "-i"])
            .arg(dir.join("%06d.png")).args(["-vf", "pad=ceil(iw/2)*2:ceil(ih/2)*2", "-pix_fmt", "yuv420p",
                "-c:v", "libsvtav1", "-preset", "10", "-crf", "42"]).arg(&opts.out))?;
    }
    Ok(())
}

fn create_parent(path: &Path) -> Result<(), Failure> {
    if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
        std::fs::create_dir_all(parent).map_err(|e| Failure::Usage(e.to_string()))?;
    }
    Ok(())
}

fn ffmpeg(cmd: &mut Command) -> Result<(), Failure> {
    let result = cmd.output().map_err(|e| Failure::Device(format!("ffmpeg: {e}")))?;
    if !result.status.success() {
        return Err(Failure::Device(format!("ffmpeg: {}", String::from_utf8_lossy(&result.stderr).trim())));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn still_and_clip_share_aov_parsing() {
        for output in [Output::Still, Output::Clip] {
            let mut args = vec!["scene.loft", "--out", "out.png", "--aov", "lipschitz_ratio"];
            if matches!(output, Output::Clip) { args.extend(["--frames", "2", "--fps", "24"]); }
            let args: Vec<_> = args.into_iter().map(str::to_owned).collect();
            assert_eq!(Options::read(&args, output).unwrap().aov, Aov::LipschitzRatio);
        }
    }
}
