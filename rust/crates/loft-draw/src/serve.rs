//! `loft-draw serve`: the draw child of `loft serve` (src/cli/serve.ts), one JSON line per request on stdin and one
//! answer line per request on stdout, in order. The supervisor has already checked the bundle (names, sizes against
//! --limits, the --check hook), so a request here is only what the frames need:
//!
//!   in   {"bundle": ".../scene.loft", "camera": {"kind": "named", "name": "three"}
//!                                            | {"kind": "look", "eye": [..], "target": [..], "up": [..], "fovY": 0.6},
//!         "times": [0, 1.5], "size": [512, 512], "params": {"a.b": 1}, "inputs": {"strike": [1]},
//!         "precision": "f32" | "mixed", "out": "..."}
//!   out  first and unasked: {"ready": true, "wire": 2, "adapter": {...}, "device": {"runtime", "adapter", "driver"}}
//!        then per request:  {"ok": true, "precision": "f32", "frames": [{"file", "sha256"}], "ms": {"load", "frames"}}
//!                       or  {"ok": false, "verdict": {"kind": "usage" | "load" | "scene" | "environment" | "internal", "message"}}
//!
//! Frame k is written to `out/k.rgba`. A lost device ends the process with exit 6 (main.rs `gpu`), which the supervisor
//! answers for every request in flight. A panic answers `internal` and the server goes on: every load is fresh.

use crate::{Gpu, Target, Verdict, gpu, load, read_bundle};
use loft_ir::identity::Precision;
use serde::Deserialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::io::{BufRead, Write};
use std::num::NonZeroU32;
use std::path::PathBuf;
use std::time::Instant;

/// The protocol's version, checked by the supervisor at `ready`: a stale binary is an environment error, not a mismatch.
const WIRE: u32 = 2;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Request {
    bundle: PathBuf,
    camera: Camera,
    times: Vec<f64>,
    size: [NonZeroU32; 2],
    params: BTreeMap<String, f32>,
    inputs: BTreeMap<String, Vec<f32>>,
    precision: Ask,
    out: PathBuf,
}

/// The view: one of the bundle's cameras, or a host look camera (built on the GPU as a bundle's are).
#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase", deny_unknown_fields)]
enum Camera {
    Named {
        name: String,
    },
    Look {
        eye: [f32; 3],
        target: [f32; 3],
        up: [f32; 3],
        #[serde(rename = "fovY")]
        fov_y: f32,
    },
}

/// The precision asked for: `f32` draws on a device without `SHADER_F16`; `mixed` draws f16 where the device has it,
/// else f32 (the answer says which ran). Each runs on its own device.
#[derive(Deserialize, Clone, Copy)]
#[serde(rename_all = "lowercase")]
enum Ask {
    F32 = 0,
    Mixed = 1,
}

pub fn run() -> Result<(), Verdict> {
    let io = |e: std::io::Error| Verdict::Environment(format!("stdio: {e}"));
    let mut gpus: [Option<Gpu>; 2] = [None, Some(gpu(true)?)];
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
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| draw(g, &req))).unwrap_or_else(|p| {
                    let what =
                        p.downcast_ref::<String>().cloned().or_else(|| p.downcast_ref::<&str>().map(|s| s.to_string()));
                    Err(Verdict::Internal(format!("loft-draw panicked: {}", what.unwrap_or_default())))
                })
            })
            .unwrap_or_else(|v| json!({"ok": false, "verdict": v}));
        writeln!(out, "{answer}").and_then(|()| out.flush()).map_err(io)?;
    }
    Ok(())
}

fn draw(g: &Gpu, req: &Request) -> Result<Value, Verdict> {
    let ms = |t: Instant| t.elapsed().as_secs_f64() * 1e3;
    let t0 = Instant::now();
    let ask = match req.precision {
        Ask::F32 => Precision::F32,
        Ask::Mixed => Precision::Mixed,
    };
    let mut loft = load(g, &read_bundle(&req.bundle)?, ask, None)?;
    match &req.camera {
        Camera::Named { name } => loft.use_camera(name)?,
        Camera::Look { eye, target, up, fov_y } => loft.set_look(*eye, *target, *up, *fov_y),
    }
    for (name, v) in &req.params {
        loft.set_param(name, *v)?;
    }
    for (name, v) in &req.inputs {
        loft.set_input(name, v)?;
    }
    let precision = match loft.precision() {
        Precision::F32 => "f32",
        Precision::Mixed => "mixed",
    };
    let load = ms(t0);
    let t1 = Instant::now();
    let env = |e: std::io::Error| Verdict::Environment(format!("{}: {e}", req.out.display()));
    std::fs::create_dir_all(&req.out).map_err(env)?;
    let target = Target::new(&g.device, (req.size[0].get(), req.size[1].get()));
    let mut frames = Vec::with_capacity(req.times.len());
    for (k, &time) in req.times.iter().enumerate() {
        loft.set_time(time as f32);
        let rgba = target.picture(g, &mut loft)?;
        let file = req.out.join(format!("{k}.rgba"));
        std::fs::write(&file, &rgba).map_err(env)?;
        let mut frame = json!({"file": file, "sha256": format!("{:x}", Sha256::digest(&rgba))});
        let steps = loft.frame_key(0, req.size.map(|n| n.get())).steps;
        if !steps.is_empty() {
            frame["steps"] = json!(steps);
        }
        frames.push(frame);
    }
    Ok(json!({"ok": true, "precision": precision, "frames": frames, "ms": {"load": load, "frames": ms(t1)}}))
}

/// `src/identity.ts` `Identity.device`. The runtime names this executable by its hash: the same binary, the same pixels.
fn device(g: &Gpu) -> Result<Value, Verdict> {
    let exe = std::env::current_exe()
        .and_then(std::fs::read)
        .map_err(|e| Verdict::Environment(format!("loft-draw cannot read its own executable to name it: {e}")))?;
    let exe = &format!("{:x}", Sha256::digest(&exe))[..16];
    Ok(json!({
        "runtime": format!("loft-draw {exe}, wgpu 30.0.1"),
        "adapter": format!("{} / {:?}", g.info.name, g.info.backend),
        "driver": format!("{} {}", g.info.driver, g.info.driver_info).trim(),
    }))
}

/// The `ready` line's adapter, in the fields `loft serve` has always printed.
fn adapter(g: &Gpu) -> Value {
    let i = &g.info;
    json!({"vendor": format!("{:#06x}", i.vendor), "architecture": format!("{:?}", i.backend), "device": i.name, "description": i.driver})
}
