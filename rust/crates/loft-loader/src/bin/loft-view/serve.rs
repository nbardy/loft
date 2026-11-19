//! `loft-view serve`: the draw child of `loft serve` and of `loft render`/`frames`/`clip` (src/cli/view.ts), one JSON
//! line per request on stdin and one answer line per request on stdout, in order. The caller has already checked the
//! artefact (names, links, sizes against --limits, the --check hook) and resolved the camera, so a request here is only
//! what the frames need:
//!
//!   in   {"dir": "...", "camera": {"kind": "perspective", "eye": [..], "target": [..], "fovY": 0.6}, "times": [0, 1.5],
//!         "size": [512, 512], "params": {"a.b": 1}, "inputs": {"strike": [1]}, "precision": "f32" | "mixed",
//!         "construction": false, "still": null | {"samples": 16, "shutter": 0.02, "aperture": 0.05}, "out": "..."}
//!   out  first and unasked: {"ready": true, "wire": 1, "adapter": {...}, "device": {"runtime", "adapter", "driver"}}
//!        then per request:  {"ok": true, "precision": "f32", "frames": [{"file", "sha256", "triangles", "draws"}],
//!                            "ms": {"load", "frames"}}
//!                       or  {"ok": false, "verdict": {"kind": "usage" | "load" | "environment" | "internal", "message"}}
//!
//! Frame k is written to `out/k.rgba`. A lost device ends the process with exit 6 (main.rs `gpu`), which the supervisor
//! answers for every request in flight. A panic answers `internal`, drops the warm artefact and the server goes on.
//!
//! The last artefact loaded stays warm per precision, keyed by its directory and the hash of every file in it: a
//! request for the same bytes skips the load (a clip is one request per frame) after `Loft::reset`, which puts the view
//! and the level-of-detail memory back as a fresh load leaves them, so the frames are the ones a fresh load draws.

use crate::{gpu, Gpu, Target, Verdict};
use loft_loader::{read_dir, Camera, Loft, Precision, Still};
use serde::Deserialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::io::{BufRead, Write};
use std::num::NonZeroU32;
use std::path::PathBuf;
use std::time::Instant;

/// The protocol's version, checked by the supervisor at `ready`: a stale binary is an environment error, not a mismatch.
const WIRE: u32 = 1;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Request {
    dir: PathBuf,
    camera: Camera,
    times: Vec<f64>,
    size: [NonZeroU32; 2],
    params: BTreeMap<String, f32>,
    inputs: BTreeMap<String, Vec<f32>>,
    precision: Ask,
    construction: bool,
    still: Option<Still>,
    out: PathBuf,
}

/// The artefact last drawn at one precision, kept loaded: its directory, the hash of its files, and the load.
struct Warm {
    dir: PathBuf,
    files: [u8; 32],
    loft: Loft,
}

/// The precision asked for (src/host/precision.ts): `f32` draws the f32 shaders; `mixed` draws f16 where the device
/// has it, else the artefact's named f32 fallback. Each runs on its own device, so `f32` never sees `SHADER_F16`.
#[derive(Deserialize, Clone, Copy)]
#[serde(rename_all = "lowercase")]
enum Ask {
    F32 = 0,
    Mixed = 1,
}

pub fn run() -> Result<(), Verdict> {
    let io = |e: std::io::Error| Verdict::Environment(format!("stdio: {e}"));
    let mut gpus: [Option<Gpu>; 2] = [None, Some(gpu(true)?)];
    let mut warm: [Option<Warm>; 2] = [None, None];
    let g = gpus[Ask::Mixed as usize].as_ref().expect("made above");
    let mut out = std::io::stdout().lock();
    let ready = json!({"ready": true, "wire": WIRE, "adapter": adapter(g), "device": device(g)?});
    writeln!(out, "{ready}").and_then(|()| out.flush()).map_err(io)?;
    for line in std::io::stdin().lock().lines() {
        let line = line.map_err(io)?;
        let answer = serde_json::from_str::<Request>(&line)
            .map_err(|e| Verdict::Usage(format!("draw request: {e}")))
            .and_then(|req| {
                let slot = &mut gpus[req.precision as usize];
                let g = match slot {
                    Some(g) => g,
                    None => slot.insert(gpu(false)?),
                };
                let kept = &mut warm[req.precision as usize];
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| draw(g, kept, &req))).unwrap_or_else(|p| {
                    *kept = None;
                    let what =
                        p.downcast_ref::<String>().cloned().or_else(|| p.downcast_ref::<&str>().map(|s| s.to_string()));
                    Err(Verdict::Internal(format!("loft-view panicked: {}", what.unwrap_or_default())))
                })
            })
            .unwrap_or_else(|v| json!({"ok": false, "verdict": v}));
        writeln!(out, "{answer}").and_then(|()| out.flush()).map_err(io)?;
    }
    Ok(())
}

/// The sha256 of every regular file under `dir`, by relative path in sorted order (paths and contents both hashed).
fn files_hash(dir: &std::path::Path) -> Result<[u8; 32], Verdict> {
    let io = |p: &std::path::Path, e: std::io::Error| Verdict::Load(format!("{}: {e}", p.display()));
    let mut files = vec![];
    let mut todo = vec![dir.to_path_buf()];
    while let Some(d) = todo.pop() {
        for e in std::fs::read_dir(&d).map_err(|e| io(&d, e))? {
            let p = e.map_err(|e| io(&d, e))?.path();
            if p.is_dir() {
                todo.push(p);
            } else {
                files.push(p);
            }
        }
    }
    files.sort();
    let mut h = Sha256::new();
    for p in files {
        h.update(p.strip_prefix(dir).unwrap_or(&p).to_string_lossy().as_bytes());
        h.update([0]);
        h.update(std::fs::read(&p).map_err(|e| io(&p, e))?);
    }
    Ok(h.finalize().into())
}

fn draw(g: &Gpu, warm: &mut Option<Warm>, req: &Request) -> Result<Value, Verdict> {
    let ms = |t: Instant| t.elapsed().as_secs_f64() * 1e3;
    let t0 = Instant::now();
    let files = files_hash(&req.dir)?;
    let loft = match warm.take() {
        Some(mut w) if w.dir == req.dir && w.files == files => {
            w.loft.reset(&g.device)?;
            warm.insert(w)
        }
        _ => {
            let loft = Loft::load(&g.device, &g.queue, read_dir(&req.dir)?)?;
            warm.insert(Warm { dir: req.dir.clone(), files, loft })
        }
    };
    let loft = &mut loft.loft;
    loft.set_camera(req.camera);
    loft.set_construction(req.construction);
    for (name, v) in &req.params {
        loft.set_param(name, *v)?;
    }
    for (name, v) in &req.inputs {
        loft.set_input(name, v)?;
    }
    let precision = match (req.precision, loft.precision()) {
        // The f32 device has no SHADER_F16, so a mixed artefact drew its f32 shaders: what was asked, and ran.
        (Ask::F32, Precision::F32 | Precision::F32Fallback) => "f32",
        (Ask::F32, Precision::Mixed) => unreachable!("the f32 device is made without SHADER_F16"),
        (Ask::Mixed, p @ (Precision::F32 | Precision::Mixed | Precision::F32Fallback)) => p.as_str(),
    };
    let load = ms(t0);
    let t1 = Instant::now();
    let env = |e: std::io::Error| Verdict::Environment(format!("{}: {e}", req.out.display()));
    std::fs::create_dir_all(&req.out).map_err(env)?;
    let target = Target::new(&g.device, (req.size[0].get(), req.size[1].get()));
    let mut frames = Vec::with_capacity(req.times.len());
    for (k, &time) in req.times.iter().enumerate() {
        loft.set_time(time);
        let (rgba, drawn) = match req.still {
            None => target.picture(g, loft)?,
            Some(s) => target.still(g, loft, s)?,
        };
        let file = req.out.join(format!("{k}.rgba"));
        std::fs::write(&file, &rgba).map_err(env)?;
        frames.push(json!({
            "file": file, "sha256": format!("{:x}", Sha256::digest(&rgba)), "triangles": drawn.triangles, "draws": drawn.draws,
        }));
    }
    Ok(json!({"ok": true, "precision": precision, "frames": frames, "ms": {"load": load, "frames": ms(t1)}}))
}

/// `src/identity.ts` `Identity.device`. The runtime names this executable by its hash: the same binary, the same pixels.
fn device(g: &Gpu) -> Result<Value, Verdict> {
    let exe = std::env::current_exe()
        .and_then(std::fs::read)
        .map_err(|e| Verdict::Environment(format!("loft-view cannot read its own executable to name it: {e}")))?;
    let exe = &format!("{:x}", Sha256::digest(&exe))[..16];
    Ok(json!({
        "runtime": format!("loft-view {exe}, wgpu 30.0.1"),
        "adapter": format!("{} / {:?}", g.info.name, g.info.backend),
        "driver": format!("{} {}", g.info.driver, g.info.driver_info).trim(),
    }))
}

/// The `ready` line's adapter, in the fields `loft serve` has always printed.
fn adapter(g: &Gpu) -> Value {
    let i = &g.info;
    json!({"vendor": format!("{:#06x}", i.vendor), "architecture": format!("{:?}", i.backend), "device": i.name, "description": i.driver})
}
