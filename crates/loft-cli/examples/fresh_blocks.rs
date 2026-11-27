//! Warm the device before the first native load control, then bracket a full 120-frame orbit.
use loft_cli::{native, sources};
use loft_passes::{compile, program::Pass, sexp};
use std::{env, fs, path::Path};

fn main() -> Result<(), String> {
    let args: Vec<_> = env::args().skip(1).collect();
    let [source, width, height, destination] = &args[..] else { return Err("expected input, width, height and output".into()); };
    let path = Path::new(source);
    let size = (width.parse().map_err(|_| "invalid width")?, height.parse().map_err(|_| "invalid height")?);
    let p = if path.extension().is_some_and(|x| x == "ir") {
        sexp::read(&fs::read_to_string(path).map_err(|e| e.to_string())?)?
    } else {
        let src = sources(path)?;
        compile(&src, &[]).map_err(|d| d.render(&src))?.0
    };
    if p.camera.fixed_eye.is_some() { return Err("requires a moving camera".into()); }
    let vis = p.passes.iter().position(|p| matches!(p, Pass::Vis(_))).ok_or("missing Vis")?;
    let g = native::gpu()?;
    let loaded = native::Loaded::new(&g, &p, size, None)?;
    loaded.profile_frames(&g, (0..30).map(|i| i as f32 / 30.0), true)?;
    let times = (0..120).map(|i| i as f32 * (1.0/60.0)).collect::<Vec<_>>();
    let attempts = loaded.profile_blocks(&g, size, &times, 120, 2)?;
    // Measure the ordinary native host call. Drain its preceding frame outside
    // the timed interval, after finishing every load-control-bracketed sample.
    let mut host_submit_ms = Vec::new();
    for &t in times.iter().step_by(4) {
        g.device.poll(wgpu::PollType::Wait { submission_index: None, timeout: None }).map_err(|e| e.to_string())?;
        let start = std::time::Instant::now();
        loaded.frame(&g, t);
        host_submit_ms.push(start.elapsed().as_secs_f64() * 1000.0);
    }
    let rows = attempts.iter().map(|b| {
        assert_eq!(b.frames.len(), 120);
        assert!(b.frames.iter().all(|f| f.pass_ms[vis] > 0.0));
        serde_json::json!({
            "adapter":g.adapter, "passes":p.passes.iter().map(|p|format!("{p:?}")).collect::<Vec<_>>(),
            "gpu_frame_ms":b.frames.iter().map(|f|f.gpu_ms).collect::<Vec<_>>(),
            "gpu_pass_ms":b.frames.iter().map(|f|&f.pass_ms).collect::<Vec<_>>(),
            "commands":b.frames.iter().map(|f|f.commands).collect::<Vec<_>>(),
            "load_control_before_p50_ms":b.control_before[0], "load_control_after_p50_ms":b.control_after[0],
            "contended":!b.quiet(), "host_submit_ms":host_submit_ms
        })
    }).collect::<Vec<_>>();
    fs::write(destination, serde_json::to_vec(&rows).unwrap()).map_err(|e| e.to_string())
}
