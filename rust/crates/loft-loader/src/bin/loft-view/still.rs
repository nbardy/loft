//! `loft-view still`: one picture of a built directory (moved from loft-view). Its view flags are `profile`'s too.
//!
//!   loft-view still <dir> --view game --time 1.0 --size 1280x800 [--out f.png] [--input name=v[,v...]] [--param name=v]
//!
//! The picture waits for its own level-of-detail bounds (as `loft render` does).

use crate::profile::Flags;
use crate::{gpu, Target, Verdict};
use loft_loader::{read_dir, Loft};
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
}

const PROFILE: &str = "--frames, --loads, --wait, --pose-time, --pipeline-cache and --budget-ms are flags of `profile`";

pub fn usage(msg: String) -> ! {
    eprintln!("loft-view: {msg}");
    std::process::exit(2)
}

/// Reads `<dir> [flags]`. The timing flags are `profile`'s alone: `still` refuses them, `profile` refuses `--out`.
pub fn parse(mut it: impl Iterator<Item = String>, profile: bool) -> (Args, Flags) {
    let mut f = Flags::default();
    let count = |v: String, flag: &str| match v.parse::<usize>() {
        Ok(n) if n > 0 => n,
        _ => usage(format!("{flag}: expected a count of at least 1, got {v}")),
    };
    let dir = it.next().map(PathBuf::from).unwrap_or_else(|| usage("expected a built directory".into()));
    let mut a =
        Args { dir, view: "game".into(), time: 1.0, size: (1280, 800), out: None, inputs: vec![], params: vec![] };
    let num = |s: &str, flag: &str| s.parse::<f64>().unwrap_or_else(|_| usage(format!("{flag}: not a number: {s}")));
    while let Some(flag) = it.next() {
        let mut value = || it.next().unwrap_or_else(|| usage(format!("{flag} needs a value")));
        match flag.as_str() {
            "--frames" | "--loads" | "--wait" | "--pose-time" | "--pipeline-cache" | "--budget-ms" if !profile => {
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
            "--frames" => f.frames = count(value(), "--frames"),
            "--loads" => f.loads = count(value(), "--loads"),
            "--wait" => f.wait = true,
            "--pose-time" => f.pose_time = true,
            "--pipeline-cache" => f.cache = Some(value().into()),
            "--budget-ms" => f.budget_ms = Some(num(&value(), "--budget-ms")),
            _ => usage(format!("unknown flag {flag}")),
        }
    }
    (a, f)
}

/// Points `loft` at the asked view, time, inputs and params.
pub fn aim(a: &Args, loft: &mut Loft) -> Result<(), Verdict> {
    loft.set_camera(loft.camera(&a.view)?);
    loft.set_time(a.time);
    for (name, v) in &a.inputs {
        loft.set_input(name, v)?;
    }
    for (name, v) in &a.params {
        loft.set_param(name, *v)?;
    }
    Ok(())
}

pub fn run(a: Args) -> Result<(), Verdict> {
    let g = gpu(true)?;
    eprintln!("adapter {} ({:?}); timestamp queries {}", g.info.name, g.info.backend, g.timing);
    let t = Instant::now();
    let mut loft = Loft::load(&g.device, &g.queue, read_dir(&a.dir)?)?;
    eprintln!("load {:.3} ms, precision {}", t.elapsed().as_secs_f64() * 1e3, loft.precision().as_str());
    aim(&a, &mut loft)?;
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
    Ok(())
}
