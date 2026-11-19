use crate::{budget::Failure, flag, sources};
use loft_cli::native::{self, Loaded};
use loft_passes::telemetry::{self, ModuleTelemetry};
use serde::Serialize;
use std::path::Path;

#[derive(Serialize)]
struct Profile {
    adapter: String,
    modules: Vec<ModuleTelemetry>,
    control_p50_ms: f64,
    passes: Vec<String>,
    gpu_frame_ms: Vec<f64>,
    gpu_pass_ms: Vec<Vec<f64>>,
}

pub(crate) fn write_json(path: &str, value: &impl Serialize) -> Result<(), Failure> {
    let data = serde_json::to_string_pretty(value).map_err(|e| Failure::Usage(e.to_string()))?;
    if path == "-" {
        println!("{data}");
        Ok(())
    } else {
        std::fs::write(path, data + "\n").map_err(|e| Failure::Usage(format!("{path}: {e}")))
    }
}

pub(crate) fn run(args: &[String], set: &[(String, f64)]) -> Result<(), Failure> {
    let mut args = args.to_vec();
    let json = flag(&mut args, "--json")?;
    let size = flag(&mut args, "--size")?.unwrap_or_else(|| "512x512".into());
    let count = |s: &str| s.parse::<u32>().ok().filter(|&n| n != 0);
    let size = size
        .split_once('x')
        .and_then(|(w, h)| Some((count(w)?, count(h)?)))
        .ok_or_else(|| Failure::Usage("--size takes positive WxH".into()))?;
    let frames = flag(&mut args, "--frames")?
        .map_or(Some(60), |s| count(&s))
        .ok_or_else(|| Failure::Usage("--frames takes a positive count".into()))?;
    let warmup = flag(&mut args, "--warmup")?.map_or(Some(30), |s| s.parse::<u32>().ok())
        .ok_or_else(|| Failure::Usage("--warmup takes a nonnegative integer".into()))?;
    let time = flag(&mut args, "--time")?.map(|s| s.parse::<f32>().ok()
        .filter(|x| x.is_finite()).ok_or_else(|| Failure::Usage("--time takes a finite value".into()))).transpose()?;
    let nonce = flag(&mut args, "--nonce")?
        .map(|s| count(&s).ok_or_else(|| Failure::Usage("--nonce takes a nonzero u32".into())))
        .transpose()?;
    let [scene] = args.as_slice() else {
        return Err(Failure::Usage(
            "usage: loft profile SCENE --size WxH [--frames N] [--nonce N] [--json OUT]".into(),
        ));
    };
    let path = Path::new(scene);
    let program = if path.extension().is_some_and(|ext| ext == "sexp") {
        if !set.is_empty() {
            return Err(Failure::Usage("--set requires an authored scene".into()));
        }
        let data = std::fs::read_to_string(path).map_err(|e| Failure::Usage(e.to_string()))?;
        loft_passes::sexp::read(&data).map_err(Failure::Compile)?
    } else {
        let src = sources(path).map_err(Failure::Usage)?;
        loft_passes::compile(&src, set).map_err(|d| Failure::Compile(d.render(&src)))?.0
    };
    let modules = telemetry::collect(&program);
    let gpu = native::gpu().map_err(Failure::Device)?;
    crate::budget::timestamps(gpu.timestamps)?;
    let control_p50_ms = gpu.triangle_control_ms(size).map_err(Failure::Device)?;
    let loaded = Loaded::new(&gpu, &program, size, nonce).map_err(Failure::Device)?;
    let mut report = Profile {
        adapter: gpu.adapter.clone(),
        modules,
        control_p50_ms,
        passes: program.passes.iter().map(|p| format!("{p:?}")).collect(),
        gpu_frame_ms: vec![],
        gpu_pass_ms: vec![],
    };
    for frame in 0..u64::from(frames) + u64::from(warmup) {
        loaded.frame(&gpu, time.unwrap_or_else(|| frame.saturating_sub(u64::from(warmup)) as f32 / 60.));
        let (total, passes) = loaded
            .gpu_ms(&gpu)
            .map_err(Failure::Device)?
            .ok_or_else(|| Failure::Device("GPU timestamps are unavailable".into()))?;
        if frame >= u64::from(warmup) {
            report.gpu_frame_ms.push(total);
            report.gpu_pass_ms.push(passes);
        }
    }
    if let Some(path) = json {
        write_json(&path, &report)
    } else {
        let mut times = report.gpu_frame_ms;
        times.sort_by(f64::total_cmp);
        println!(
            "{} frames, GPU p50 {:.3} ms, p95 {:.3} ms ({})",
            frames,
            times[(times.len() * 50).div_ceil(100) - 1],
            times[(times.len() * 95).div_ceil(100) - 1],
            report.adapter
        );
        Ok(())
    }
}
