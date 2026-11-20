//! `loft-draw still`: one picture of a bundle. Its view flags are `profile`'s too.
//!
//!   loft-draw still <bundle> --view game --time 1.0 --size 1280x800 [--out f.png] [--input name=v[,v...]] [--param name=v]
//!
//! `--view` names one of the bundle's cameras, evaluated on the GPU at the frame's time. The bundle draws as f32.
//! `--shot VIEW@TIME@WxH=FILE` (repeatable) draws more frames after the first from the same load: a parity run draws
//! every frame of a scene with one load, which is most of a frame's cost when a bundle builds hundreds of pipelines.

use crate::profile::Flags;
use crate::{Target, Verdict, gpu, load, read_bundle};
use loft_gpu::Loft;
use loft_ir::identity::Precision;
use std::path::PathBuf;
use std::time::Instant;

pub struct Args {
    pub dir: PathBuf,
    view: String,
    time: f64,
    pub size: (u32, u32),
    out: Option<PathBuf>,
    inputs: Vec<(String, Vec<f32>)>,
    params: Vec<(String, f32)>,
    shots: Vec<(String, f64, (u32, u32), PathBuf)>,
}

const PROFILE: &str = "--frames, --loads, --pipeline-cache and --budget-ms are flags of `profile`";

pub fn usage(msg: String) -> ! {
    eprintln!("loft-draw: {msg}");
    std::process::exit(2)
}

/// Reads `<dir> [flags]`. The timing flags are `profile`'s alone: `still` refuses them, `profile` refuses `--out`.
pub fn parse(mut it: impl Iterator<Item = String>, profile: bool) -> (Args, Flags) {
    let mut f = Flags::default();
    let count = |v: String, flag: &str| match v.parse::<usize>() {
        Ok(n) if n > 0 => n,
        _ => usage(format!("{flag}: expected a count of at least 1, got {v}")),
    };
    let dir =
        it.next().map(PathBuf::from).unwrap_or_else(|| usage("expected a bundle (scene.loft or its directory)".into()));
    let mut a = Args {
        dir,
        view: "game".into(),
        time: 1.0,
        size: (1280, 800),
        out: None,
        inputs: vec![],
        params: vec![],
        shots: vec![],
    };
    let num = |s: &str, flag: &str| s.parse::<f64>().unwrap_or_else(|_| usage(format!("{flag}: not a number: {s}")));
    while let Some(flag) = it.next() {
        let mut value = || it.next().unwrap_or_else(|| usage(format!("{flag} needs a value")));
        match flag.as_str() {
            "--cold-nonce" | "--frames" | "--loads" | "--pipeline-cache" | "--budget-ms" if !profile => {
                usage(PROFILE.into())
            }
            "--out" if profile => usage("--out is a flag of `still`".into()),
            "--view" => a.view = value(),
            "--time" => a.time = num(&value(), "--time"),
            "--size" => {
                let v = value();
                let (w, h) = v.split_once('x').unwrap_or_else(|| usage(format!("--size: expected WxH, got {v}")));
                a.size = (num(w, "--size") as u32, num(h, "--size") as u32);
            }
            "--out" => a.out = Some(value().into()),
            "--shot" if !profile => {
                let v = value();
                let bad = || -> ! { usage(format!("--shot: expected VIEW@TIME@WxH=FILE, got {v}")) };
                let (what, file) = v.split_once('=').unwrap_or_else(|| bad());
                let mut parts = what.splitn(3, '@');
                let (view, time, size) = (parts.next(), parts.next(), parts.next());
                let (Some(view), Some(time), Some(size)) = (view, time, size) else { bad() };
                let (w, h) = size.split_once('x').unwrap_or_else(|| bad());
                a.shots.push((
                    view.into(),
                    num(time, "--shot"),
                    (num(w, "--shot") as u32, num(h, "--shot") as u32),
                    file.into(),
                ));
            }
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
            "--cold-nonce" => {
                f.nonce = Some(value().parse().unwrap_or_else(|_| usage("--cold-nonce needs a u32".into())))
            }
            "--frames" => f.frames = count(value(), "--frames"),
            "--loads" => f.loads = count(value(), "--loads"),
            "--pipeline-cache" => f.cache = Some(value().into()),
            "--budget-ms" => f.budget_ms = Some(num(&value(), "--budget-ms")),
            _ => usage(format!("unknown flag {flag}")),
        }
    }
    (a, f)
}

/// Points `loft` at the asked view, time, inputs and params.
pub fn aim(a: &Args, loft: &mut Loft) -> Result<(), Verdict> {
    loft.use_camera(&a.view).map_err(|e| {
        let have: Vec<&str> = loft.cameras().collect();
        Verdict::Usage(format!("{e} (it has: {})", if have.is_empty() { "none".into() } else { have.join(", ") }))
    })?;
    loft.set_time(a.time as f32);
    for (name, v) in &a.inputs {
        loft.set_input(name, v)?;
    }
    for (name, v) in &a.params {
        loft.set_param(name, *v)?;
    }
    Ok(())
}

pub fn run(mut a: Args) -> Result<(), Verdict> {
    let g = gpu(true)?;
    eprintln!("adapter {} ({:?}); timestamp queries {}", g.info.name, g.info.backend, g.timing);
    let t = Instant::now();
    let mut loft = load(&g, &read_bundle(&a.dir)?, Precision::F32, None)?;
    eprintln!("load {:.3} ms, precision {:?}", t.elapsed().as_secs_f64() * 1e3, loft.precision());
    let shots = std::mem::take(&mut a.shots);
    let first = (a.view.clone(), a.time, a.size, a.out.clone());
    for (k, (view, time, size, out)) in
        std::iter::once(first).chain(shots.into_iter().map(|(v, t, s, o)| (v, t, s, Some(o)))).enumerate()
    {
        (a.view, a.time, a.size) = (view, time, size);
        aim(&a, &mut loft)?;
        let target = Target::new(&g.device, a.size);
        let rgba = target.picture(&g, &mut loft)?;
        if k == 0 {
            let b = loft.bundle();
            let (draws, passes) = match &b.format {
                loft_ir::bundle::Format::V1 => (b.draws.len(), b.passes.len()),
                loft_ir::bundle::Format::V2(s) => (s.draws(), s.passes.len()),
            };
            eprintln!("frame: {draws} draws in {passes} passes");
        }
        if let Some(out) = &out {
            let fail = |e: &dyn std::fmt::Display| Verdict::Environment(format!("{}: {e}", out.display()));
            let file = std::fs::File::create(out).map_err(|e| fail(&e))?;
            let mut png = png::Encoder::new(std::io::BufWriter::new(file), a.size.0, a.size.1);
            png.set_filter(png::Filter::NoFilter); // tools/picdiff.ts reads unfiltered rows only
            png.set_color(png::ColorType::Rgba);
            png.set_depth(png::BitDepth::Eight);
            png.write_header().and_then(|mut w| w.write_image_data(&rgba)).map_err(|e| fail(&e))?;
        }
    }
    Ok(())
}
