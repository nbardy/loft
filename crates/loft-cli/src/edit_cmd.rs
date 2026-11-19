use crate::{budget::{self, Failure}, gates::{quantile, write_png}};
use std::path::Path;

/// Exercise a source edit through the prewarmed visibility and shade pipelines.
pub fn edited_still(
    base: &Path, edit: &Path, kind: &str, set: &[(String, f64)], t: f32, out: &Path,
    max_frame: Option<f64>,
) -> Result<(), Failure> {
    use loft_cli::native::TraceSession;
    let sources = crate::sources(base).map_err(Failure::Usage)?;
    let (program, _, checked) = loft_passes::compile(&sources, set)
        .map_err(|d| Failure::Compile(d.render(&sources)))?;
    let tape = loft_passes::tier0::authored_trace(&checked, kind)
        .map_err(|d| Failure::Compile(d.render(&sources)))?;
    let gpu = loft_cli::native::gpu().map_err(Failure::Device)?;
    if max_frame.is_some() { budget::timestamps(gpu.timestamps)?; }
    let size = (1280, 800);
    let mut session = TraceSession::new(&gpu, &program, kind, &tape, size).map_err(Failure::Device)?;
    session.loaded.frame(&gpu, t);
    let before = session.loaded.pixels(&gpu).map_err(Failure::Device)?;
    let edited = crate::sources(edit).map_err(Failure::Usage)?;
    let upload = session.edit(&gpu, &program, &edited, set).map_err(Failure::Compile)?;
    session.loaded.frame(&gpu, t);
    let tier0 = session.loaded.pixels(&gpu).map_err(Failure::Device)?;
    if let Some(max) = max_frame { budget::check(&gpu, &session.loaded, max)?; }
    let mut timing = vec![];
    for _ in 0..12 {
        session.loaded.frame(&gpu, t);
        if let Some((ms, _)) = session.loaded.gpu_ms(&gpu).map_err(Failure::Device)? { timing.push(ms); }
    }
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
    while !session.poll_promotion(&gpu).map_err(Failure::Device)? {
        if std::time::Instant::now() > deadline { return Err(Failure::Device("trace promotion timed out".into())); }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    session.loaded.frame(&gpu, t);
    let tier1 = session.loaded.pixels(&gpu).map_err(Failure::Device)?;
    let mut promoted = vec![];
    for _ in 0..12 {
        session.loaded.frame(&gpu, t);
        if let Some((ms, _)) = session.loaded.gpu_ms(&gpu).map_err(Failure::Device)? { promoted.push(ms); }
    }
    let compare = |a: &[u8], b: &[u8], limit| a.chunks_exact(4).zip(b.chunks_exact(4))
        .filter(|(a,b)| a[..3].iter().zip(&b[..3]).any(|(x,y)| x.abs_diff(*y) > limit)).count();
    let changed = compare(&before, &tier0, 0);
    let over = compare(&tier0, &tier1, 1);
    println!("tape upload {:.6} ms, pipeline creations 0, changed pixels {changed}, over 1/255 after promotion {over}/{}",
        upload.as_secs_f64() * 1000., tier0.len() / 4);
    if !timing.is_empty() {
        println!("GPU p95 Tier 0 {:.6} ms, Tier 1 {:.6} ms", quantile(&mut timing, 0.95), quantile(&mut promoted, 0.95));
    }
    write_png(out, size, &tier0).map_err(Failure::Usage)?;
    if over * 1000 > tier0.len() / 4 {
        return Err(Failure::Truth("trace tape differs from Tier 1 on more than 0.1% of pixels".into()));
    }
    Ok(())
}
