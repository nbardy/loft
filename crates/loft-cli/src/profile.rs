use crate::{budget::Failure, flag, sources};
use loft_cli::{percentile_nearest_rank, shader_hashes, write_json};
use loft_cli::native::{self, FrameTiming, Loaded};
use loft_passes::program::Program;
use loft_passes::telemetry::{self, ModuleTelemetry};
use serde::Serialize;
use std::{path::Path, time::Instant};

#[derive(Serialize)]
struct Profile {
    adapter: String,
    precision: &'static str,
    modules: Vec<ModuleTelemetry>,
    control_p50_ms: f64,
    control_p95_ms: f64,
    /// A scene-length control (`load_control`) before and after the frames. Either p50 above the quiet bound
    /// marks the run contended; the triangle above is too short to see contention.
    load_control_before_p50_ms: f64,
    load_control_after_p50_ms: f64,
    contended: bool,
    hardware_ray_query: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    device_loads_ms: Option<Vec<f64>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pipelines: Option<usize>,
    passes: Vec<String>,
    gpu_frame_ms: Vec<f64>,
    gpu_pass_ms: Vec<Vec<f64>>,
    frame_time_bits: Vec<u32>,
    frame_stamps: Vec<FrameTiming>,
    shader_sha256: Vec<(String, String)>,
}

pub(crate) fn run(args: &[String], set: &[(String, f64)]) -> Result<(), Failure> {
    let mut args = args.to_vec();
    let json = flag(&mut args, "--json")?;
    let timestamps = flag(&mut args, "--timestamps")?.unwrap_or_else(|| "immediate".into());
    if !matches!(timestamps.as_str(), "immediate" | "ring" | "ab") {
        return Err(Failure::Usage("--timestamps takes immediate, ring or ab".into()));
    }
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
    let loads = flag(&mut args, "--loads")?
        .map(|s| count(&s).ok_or_else(|| Failure::Usage("--loads takes a positive count".into())))
        .transpose()?;
    let [scene] = args.as_slice() else {
        return Err(Failure::Usage(
            "usage: loft profile SCENE --size WxH [--frames N] [--nonce N] [--loads N] [--timestamps immediate|ring|ab] [--json OUT]".into(),
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
    let gpu = native::gpu().map_err(Failure::Device)?;
    crate::budget::timestamps(gpu.timestamps)?;
    // Keep the device and driver compilation key fixed across the warm loads.
    let mut device_loads_ms = Vec::new();
    let mut loaded = None;
    for _ in 0..loads.unwrap_or(1) {
        let started = Instant::now();
        loaded = Some(Loaded::new(&gpu, &program, size, nonce).map_err(Failure::Device)?);
        device_loads_ms.push(started.elapsed().as_secs_f64() * 1e3);
    }
    let loaded = loaded.expect("at least one device load");
    let times = (0..warmup).map(|i| time.unwrap_or(i as f32 / 30.0))
        .chain((0..frames).map(|i| time.unwrap_or(i as f32 * (1.0 / 60.0))))
        .collect::<Vec<_>>();
    let run = |deferred| -> Result<Profile, Failure> {
        let [control_p50_ms, control_p95_ms] = gpu.triangle_control(size).map_err(Failure::Device)?;
        let [load_control_before_p50_ms, _] = gpu.load_control(size).map_err(Failure::Device)?;
        let frame_stamps = loaded.profile_frames(&gpu, times.iter().copied(), deferred)
            .map_err(Failure::Device)?.into_iter().skip(warmup as usize).collect::<Vec<_>>();
        let [load_control_after_p50_ms, _] = gpu.load_control(size).map_err(Failure::Device)?;
        Ok(Profile {
            adapter: gpu.adapter.clone(),
            precision: loaded.precision.name(),
            modules: telemetry::collect(&program),
            control_p50_ms,
            control_p95_ms,
            load_control_before_p50_ms,
            load_control_after_p50_ms,
            contended: load_control_before_p50_ms.max(load_control_after_p50_ms) > native::QUIET_CONTROL_P50_MS,
            hardware_ray_query: loaded.stats.hardware_ray_query,
            device_loads_ms: loads.map(|_| device_loads_ms.clone()),
            pipelines: loads.map(|_| loaded.stats.compute_pipelines + loaded.stats.render_pipelines),
            passes: program.passes.iter().map(|p| format!("{p:?}")).collect(),
            gpu_frame_ms: frame_stamps.iter().map(|f| f.gpu_ms).collect(),
            gpu_pass_ms: frame_stamps.iter().map(|f| f.pass_ms.clone()).collect(),
            frame_time_bits: times[warmup as usize..].iter().map(|t| t.to_bits()).collect(),
            frame_stamps,
            shader_sha256: shader_hashes(&program).map_err(Failure::Device)?,
        })
    };
    if timestamps == "ab" {
        let immediate = run(false)?;
        let ring = run(true)?;
        return write_json(json.as_deref().unwrap_or("-"), &serde_json::json!({ "immediate": immediate, "ring": ring }));
    }
    let report = run(timestamps == "ring")?;
    if let Some(path) = json {
        write_json(&path, &report)
    } else {
        if let Some(ms) = &report.device_loads_ms {
            println!("device loads (ms): {ms:.3?}");
        }
        let contended = report.contended;
        let (c0, c1) = (report.load_control_before_p50_ms, report.load_control_after_p50_ms);
        let mut times = report.gpu_frame_ms;
        times.sort_by(f64::total_cmp);
        println!(
            "{} frames, GPU p50 {:.3} ms, p95 {:.3} ms ({}), load control p50 {c0:.3} before / {c1:.3} after{}",
            frames,
            percentile_nearest_rank(&times, 0.5),
            percentile_nearest_rank(&times, 0.95),
            report.adapter,
            if contended { ": CONTENDED, do not trust these times" } else { "" }
        );
        Ok(())
    }
}

pub(crate) fn show(
    program: &Program,
    kind: Option<&str>,
    plan: bool,
    json: bool,
) -> Result<(), Failure> {
    let modules: Vec<_> = program
        .modules
        .iter()
        .filter(|m| kind.is_none_or(|kind| m.name == kind))
        .collect();
    if modules.is_empty() {
        return Err(Failure::Usage(format!(
            "unknown module {}",
            kind.unwrap_or_default()
        )));
    }
    let steps = || {
        program.dispatches.iter().map(|step| {
        let (table, rows, stride, _) = step.input.view(&program.kinds).expect("checked dispatch input");
        serde_json::json!({
            "module": program.modules[step.module].name, "entry": step.entry,
            "table": table, "rows": rows, "stride_words": stride, "workgroup_x": step.workgroup_x,
        })
    }).collect::<Vec<_>>()
    };
    if json {
        let modules = modules
            .iter()
            .map(|m| {
                serde_json::json!({
                    "name": m.name, "span": m.span, "origins": m.origins, "wgsl": m.wgsl,
                })
            })
            .collect::<Vec<_>>();
        let mut output = serde_json::json!({ "modules": modules });
        if plan {
            output["passes"] =
                serde_json::to_value(&program.passes).map_err(|e| Failure::Usage(e.to_string()))?;
            output["fixed_steps"] = serde_json::json!(steps());
        }
        return write_json("-", &output);
    }
    if plan {
        println!("// Passes {:?}", program.passes);
        for step in steps() {
            println!("// Fixed step {step}");
        }
    }
    for m in modules {
        println!("// ===== module {} =====", m.name);
        if let Some((path, line, col)) = &m.span {
            println!("// Source {path}:{line}:{col}");
        }
        println!("{}", m.wgsl);
        for origin in &m.origins {
            let (path, line, col) = &origin.span;
            println!(
                "// fn {} expr {} {path}:{line}:{col}",
                origin.function, origin.expression
            );
        }
    }
    Ok(())
}
