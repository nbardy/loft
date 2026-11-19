//! `loft`: the meta engine's command line.
//!
//!   loft check SCENE [--set NAME=VALUE]...      compile without a GPU; print per-kind rows, derived L and pass times
//!   loft show SCENE [MODULE] [--set ...]        print the generated WGSL (every module, or the one named)
//!   loft still SCENE OUT.png [--t SECONDS] [--set ...]
//!   loft gate [--approve]                       the five gates on every scene in engines/gates.json
//!   loft load SCENE --nonce N [--set ...]       one load in this process with driver nonce N (cache experiments)
//!
//! A scene loads every `lib/*.loft` first (the library prelude), then the scene file.

mod gates;

use loft_ir::{Diag, Sources};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

/// The repository root: the nearest ancestor of the scene holding `lib/`.
fn root_of(scene: &Path) -> Result<PathBuf, String> {
    let abs = std::fs::canonicalize(scene).map_err(|e| format!("{}: {e}", scene.display()))?;
    abs.ancestors()
        .find(|d| d.join("lib").is_dir() && d.join("Cargo.toml").is_file())
        .map(Path::to_path_buf)
        .ok_or_else(|| format!("{}: no ancestor holds lib/ and Cargo.toml", scene.display()))
}

pub fn sources(scene: &Path) -> Result<Sources, String> {
    let root = root_of(scene)?;
    let mut lib: Vec<PathBuf> = std::fs::read_dir(root.join("lib"))
        .map_err(|e| e.to_string())?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == "loft"))
        .collect();
    lib.sort();
    let mut src = Sources::default();
    for p in lib.iter().chain([&scene.to_path_buf()]) {
        let text = std::fs::read_to_string(p).map_err(|e| format!("{}: {e}", p.display()))?;
        let shown = p.strip_prefix(&root).unwrap_or(p).display().to_string();
        src.add(shown, text);
    }
    Ok(src)
}

pub fn render(d: &Diag, src: &Sources) -> String {
    d.render(src)
}

/// `--set NAME=VALUE` pairs.
fn overrides(args: &mut Vec<String>) -> Result<Vec<(String, f64)>, String> {
    let mut out = vec![];
    while let Some(i) = args.iter().position(|a| a == "--set") {
        let kv = args.get(i + 1).cloned().ok_or("--set needs NAME=VALUE")?;
        args.drain(i..i + 2);
        let (k, v) = kv.split_once('=').ok_or("--set needs NAME=VALUE")?;
        out.push((k.to_string(), v.parse::<f64>().map_err(|_| format!("--set {k}: `{v}` is not a number"))?));
    }
    Ok(out)
}

fn flag(args: &mut Vec<String>, name: &str) -> Option<String> {
    let i = args.iter().position(|a| a == name)?;
    let v = args.get(i + 1).cloned();
    args.drain(i..(i + 2).min(args.len()));
    v
}

fn run() -> Result<(), String> {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let set = overrides(&mut args)?;
    let cmd = args.first().cloned().unwrap_or_default();
    match cmd.as_str() {
        "check" | "show" => {
            let scene = PathBuf::from(args.get(1).ok_or("usage: loft check|show SCENE")?);
            let src = sources(&scene)?;
            let (program, report, _) = loft_passes::compile(&src, &set).map_err(|d| render(&d, &src))?;
            if cmd == "show" {
                for m in program.modules.iter().filter(|m| args.get(2).is_none_or(|n| *n == m.name)) {
                    println!("// ===== module {} =====\n{}", m.name, m.wgsl);
                }
                return Ok(());
            }
            for k in &report.kinds {
                println!(
                    "kind {:<12} {:?}  rows {:>4}  shapes {:>3}  derived L {:.4}..{:.4}  bound evals {}",
                    k.name, k.draw, k.rows, k.shapes, k.l_min, k.l_max, k.evals
                );
            }
            let modules = program.modules.len();
            println!("modules {modules}  passes {:?}", program.passes);
            println!("{}", report.ms.iter().map(|(n, t)| format!("{n} {t:.2} ms")).collect::<Vec<_>>().join("  "));
            Ok(())
        }
        "still" => {
            let scene = PathBuf::from(args.get(1).ok_or("usage: loft still SCENE OUT.png [--t S]")?);
            let out = PathBuf::from(args.get(2).ok_or("usage: loft still SCENE OUT.png [--t S]")?);
            let t: f32 = flag(&mut args, "--t").map_or(Ok(0.0), |v| v.parse().map_err(|_| "--t takes seconds"))?;
            gates::still(&scene, &set, t, &out)
        }
        "gate" => gates::run(args.iter().any(|a| a == "--approve")),
        "load" => {
            let nonce: u32 = flag(&mut args, "--nonce").ok_or("load needs --nonce N")?.parse().map_err(|_| "--nonce takes a u32")?;
            let scene = PathBuf::from(args.get(1).ok_or("usage: loft load SCENE --nonce N")?);
            let g = loft_runner::gpu()?;
            let t = std::time::Instant::now();
            let src = sources(&scene)?;
            let (p, _, _) = loft_passes::compile(&src, &set).map_err(|d| render(&d, &src))?;
            let front = t.elapsed().as_secs_f64();
            let l = loft_runner::Loaded::new(&g, &p, (1280, 800), Some(nonce))?;
            l.frame(&g, 0.0);
            l.gpu_ms(&g)?;
            let st = &l.stats;
            println!(
                "load {:.3} s  front {:.3} s  pipelines {:.1} ms  disk cache {}  ({})",
                t.elapsed().as_secs_f64(),
                front,
                st.pipelines_ms,
                g.cache_key.as_ref().map_or("none on this backend".to_string(), |k| format!("{k}: {} bytes read", st.cache_bytes)),
                g.adapter
            );
            Ok(())
        }
        _ => Err("usage: loft check|show|still|gate (see crates/loft-cli/src/main.rs)".into()),
    }
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("{e}");
            ExitCode::FAILURE
        }
    }
}
