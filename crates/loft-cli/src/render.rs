//! Source rendering shared by stills and clips.
use crate::{budget, budget::Failure, flag, sources};
use loft_cli::{native::{self, Gpu, Loaded}, write_png};
use loft_passes::emit::Aov;
use std::{path::{Path, PathBuf}, process::Command, time::Instant};

pub(crate) fn source_still(scene: &Path, set: &[(String, f64)], t: f32, out: &Path, max_frame: Option<f64>) -> Result<(), Failure> {
    let src = sources(scene).map_err(Failure::Usage)?;
    let (mut p, _, _) = loft_passes::compile(&src, set).map_err(|d| Failure::Compile(d.render(&src)))?;
    native::motion_layout(&p).map_err(Failure::Compile)?;
    let g = native::gpu().map_err(Failure::Device)?;
    if max_frame.is_some() { budget::timestamps(g.timestamps)?; }
    if !p.dispatches.is_empty() {
        let sampled = native::sample_motion(&g, &p, t).map_err(Failure::Device)?;
        for kind in &sampled {
            for (n, values) in &kind.fields { println!("{}.{} {:?}", kind.kind, n, values); }
            for (n, values) in &kind.checks { println!("check {} {:?}", n, values); }
        }
        p = loft_passes::compile_snapshot(&src, set, &sampled).map_err(|d| Failure::Compile(d.render(&src)))?.0;
    }
    let size = (1280, 800);
    let l = Loaded::new(&g, &p, size, None).map_err(Failure::Device)?;
    l.frame(&g, t);
    if let Some(max) = max_frame { budget::check(&g, &l, max)?; }
    let px = l.pixels(&g).map_err(Failure::Device)?;
    write_png(out, size, &px).map_err(Failure::Usage)?;
    println!("wrote {}", out.display());
    Ok(())
}

struct Options {
    scene: PathBuf,
    out: PathBuf,
    size: (u32, u32),
    samples: u32,
    start: f32,
    aov: Aov,
    engine: Option<String>,
    live: Vec<(String, f32)>,
    view: Option<String>,
    max_build: Option<f64>,
    max_frame: Option<f64>,
}

impl Options {
    fn read(args: &[String]) -> Result<Self, Failure> {
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
        let start = flag(&mut args, "--time")?.map_or(Ok(0.), |v| finite(v, "--time"))? as f32;
        let [scene] = args.as_slice() else { return Err(Failure::Usage("supply one scene and the output options".into())); };
        if scene.starts_with('-') || !start.is_finite() {
            return Err(Failure::Usage("expected a scene path and finite frame times".into()));
        }
        Ok(Self { scene: scene.into(), out, size, samples, start, aov, engine, live, view, max_build, max_frame })
    }
}

pub(crate) fn run(args: &[String], set: &[(String, f64)]) -> Result<(), Failure> {
    let opts = Options::read(args)?;
    let began = Instant::now();
    let src = if let Some(engine) = &opts.engine {
        if engine.is_empty() || !engine.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'_') {
            return Err(Failure::Usage("expected an engine module name".into()));
        }
        loft_passes::source::Graph::file(&opts.scene, &format!("use stdlib::engines::{engine}"))
    } else {
        sources(&opts.scene)
    }.map_err(Failure::Usage)?;
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
    still(&g, &loaded, &opts)
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
    fn still_accepts_diagnostic_aov() {
        let args = ["scene.loft", "--out", "out.png", "--aov", "lipschitz_ratio"].map(str::to_owned);
        assert_eq!(Options::read(&args).unwrap().aov, Aov::LipschitzRatio);
    }
}
