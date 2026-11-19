//! `loft`: the meta engine's command line.
//!
//!   loft check SCENE [--set NAME=VALUE]...      compile and run any scheduled checks at --t (default 0)
//!   loft show SCENE [MODULE] [--set ...]        print the generated WGSL (every module, or the one named)
//!   loft still SCENE OUT.png [--t SECONDS] [--set ...]
//!   loft gate [--approve]                       the five gates on every scene in engines/gates.json
//!   loft load SCENE --nonce N [--set ...]       one load in this process with driver nonce N (cache experiments)
//!
//! A scene loads every `lib/*.loft` first (the library prelude), then the scene file.

mod gates;
mod build;
mod render;
mod clip;
mod profile;
mod fmt;
mod fork;
mod budget;

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

use budget::Failure;

fn flag(args: &mut Vec<String>, name: &str) -> Result<Option<String>, Failure> {
    let Some(i) = args.iter().position(|a| a == name) else { return Ok(None); };
    let v = args.get(i + 1).filter(|a| !a.starts_with("--")).cloned()
        .ok_or_else(|| Failure::Usage(format!("{name} requires a value")))?;
    args.drain(i..i + 2);
    Ok(Some(v))
}

fn command(mut args: Vec<String>) -> Result<(), Failure> {
    let set = overrides(&mut args).map_err(Failure::Usage)?;
    let cmd = args.first().cloned().unwrap_or_default();
    let time = if matches!(cmd.as_str(), "check" | "still") {
        flag(&mut args, "--t")?.map_or(Ok(0.0), |v| {
            v.parse::<f32>().ok().filter(|x| x.is_finite() && *x >= 0.)
                .ok_or_else(|| Failure::Usage("--t takes nonnegative finite seconds".into()))
        })?
    } else { 0.0 };
    let max_frame = if cmd == "still" {
        flag(&mut args, "--max-frame-ms")?.map(|s| s.parse::<f64>().ok().filter(|x| x.is_finite() && *x >= 0.)
            .ok_or_else(|| Failure::Usage("--max-frame-ms takes a nonnegative finite budget".into()))).transpose()?
    } else { None };
    let scene = |args: &[String]| args.get(1).map(PathBuf::from)
        .ok_or_else(|| Failure::Usage(format!("usage: loft {cmd} SCENE")));
    let exact = |args: &[String], min: usize, max: usize| {
        if !(min..=max).contains(&args.len()) || args.iter().skip(1).any(|s| s.starts_with('-')) {
            return Err(Failure::Usage(format!("invalid arguments for loft {cmd}")));
        }
        Ok(())
    };
    match cmd.as_str() {
        "ir" | "build" if cmd == "ir" || args.get(1).is_some_and(|p| p.ends_with(".sexp")) => {
            let telemetry = if cmd == "build" { flag(&mut args, "--json")? } else { None };
            let output = flag(&mut args, "-o")?;
            args.retain(|a| a != "--sexp");
            exact(&args, 2, 2)?;
            let path = scene(&args)?;
            let program = if path.extension().is_some_and(|e| e == "sexp") {
                let text = std::fs::read_to_string(&path).map_err(|e| Failure::Usage(format!("{}: {e}", path.display())))?;
                loft_passes::sexp::read(&text).map_err(Failure::Compile)?
            } else {
                let src = sources(&path).map_err(Failure::Usage)?;
                loft_passes::compile(&src, &set).map_err(|d| Failure::Compile(render(&d, &src)))?.0
            };
            if let Some(path) = telemetry {
                profile::write_json(&path, &loft_passes::telemetry::collect(&program))?;
            }
            let text = loft_passes::sexp::write(&program).map_err(Failure::Compile)?;
            if let Some(out) = output { std::fs::write(out, text).map_err(|e| Failure::Usage(e.to_string()))?; }
            else if cmd == "ir" { println!("{text}"); }
            Ok(())
        }
        "build" if let Some(path) = flag(&mut args, "--json")? => {
            exact(&args, 2, 2)?;
            let src = sources(&scene(&args)?).map_err(Failure::Usage)?;
            let (program, _, _) = loft_passes::compile(&src, &set)
                .map_err(|d| Failure::Compile(render(&d, &src)))?;
            profile::write_json(&path, &loft_passes::telemetry::collect(&program))
        }
        "build" => build::run(&args[1..], &set),
        "render" => render::run(&args[1..]),
        "clip" => clip::run(&args[1..], &set),
        "profile" => profile::run(&args[1..], &set),
        "fmt" => fmt::run(&args[1..]),
        "fork" => fork::run(&args[1..]),
        "show" => {
            let kind = flag(&mut args, "--kind")?;
            let json = args.iter().any(|a| a == "--json");
            let plan = args.iter().any(|a| a == "--plan");
            args.retain(|a| a != "--json" && a != "--plan");
            exact(&args, 2, if kind.is_some() { 2 } else { 3 })?;
            let src = sources(&scene(&args)?).map_err(Failure::Usage)?;
            let (program, _, _) = loft_passes::compile(&src, &set)
                .map_err(|d| Failure::Compile(render(&d, &src)))?;
            profile::show(&program, kind.as_deref().or_else(|| args.get(2).map(String::as_str)), plan, json)
        }
        "check" => {
            exact(&args, 2, 2)?;
            let src = sources(&scene(&args)?).map_err(Failure::Usage)?;
            let (program, report, _) = loft_passes::compile(&src, &set).map_err(|d| Failure::Compile(render(&d, &src)))?;
            let mut failed = false;
            for (name, &header) in &program.priors {
                let held = program.luts[header as usize + 3].to_bits() == 1;
                println!("plan {name}: {}", if held { "Held" } else { "Unknown(SolveFailed)" });
                failed |= !held;
            }
            if program.dispatches.iter().any(|m| !m.checks.is_empty()) {
                let g = loft_cli::native::gpu().map_err(Failure::Device)?;
                for (name, verdict) in gates::authored_checks(&g, &program, time).map_err(Failure::Device)? {
                    println!("check {name} at t={time}: {verdict}");
                    failed |= !matches!(verdict, loft_ir::Verdict::Held { .. });
                }
            }
            if failed { return Err(Failure::Truth("authored checks failed".into())); }
            for k in &report.kinds {
                println!("kind {:<12} {:?}  rows {:>4}  shapes {:>3}  derived L {:.4}..{:.4}  bound evals {}",
                    k.name, k.draw, k.rows, k.shapes, k.l_min, k.l_max, k.evals);
            }
            println!("modules {}  passes {:?}", program.modules.len(), program.passes);
            Ok(())
        }
        "still" => {
            let edit = flag(&mut args, "--edit")?;
            let kind = flag(&mut args, "--kind")?;
            if let Some(edit) = edit {
                exact(&args, 3, 3)?;
                let kind = kind.ok_or_else(|| Failure::Usage("still --edit needs --kind K".into()))?;
                return gates::edited_still(&scene(&args)?, Path::new(&edit), &kind, &set, time, Path::new(&args[2]), max_frame);
            }
            if kind.is_some() { return Err(Failure::Usage("still --kind needs --edit SOURCE".into())); }
            exact(&args, 3, 3)?;
            gates::still(&scene(&args)?, &set, time, Path::new(&args[2]), max_frame)
        }
        "gate" => {
            let only = flag(&mut args, "--only")?;
            let approve = args.get(1).is_some_and(|s| s == "--approve");
            if !set.is_empty() || args.len() != 1 + usize::from(approve) {
                return Err(Failure::Usage("usage: loft gate [--only NAME,...] [--approve]".into()));
            }
            gates::run(approve, only.as_deref())
        }
        "load" => {
            let nonce: u32 = flag(&mut args, "--nonce")?.and_then(|s| s.parse().ok()).filter(|&n| n != 0)
                .ok_or_else(|| Failure::Usage("load needs --nonce N (a nonzero u32)".into()))?;
            exact(&args, 2, 2)?;
            let path = scene(&args)?;
            let g = loft_cli::native::gpu().map_err(Failure::Device)?;
            let t = std::time::Instant::now();
            let src = sources(&path).map_err(Failure::Usage)?;
            let (p, _, _) = loft_passes::compile(&src, &set).map_err(|d| Failure::Compile(render(&d, &src)))?;
            let front = t.elapsed().as_secs_f64();
            let l = loft_cli::native::Loaded::new(&g, &p, (1280, 800), Some(nonce)).map_err(Failure::Device)?;
            l.frame(&g, 0.0);
            let gpu = l.gpu_ms(&g).map_err(Failure::Device)?.map_or_else(|| "unavailable".into(), |(ms, _)| format!("{ms:.2} ms"));
            println!("load {:.3} s  front {front:.3} s  pipelines {:.1} ms  frame {gpu}  ({})",
                t.elapsed().as_secs_f64(), l.stats.pipelines_ms, g.adapter);
            Ok(())
        }
        _ => Err(Failure::Usage("usage: loft build|check|show|still|clip|gate|load|ir".into())),
    }
}

fn main() -> ExitCode {
    match command(std::env::args().skip(1).collect()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(failure) => {
            let (code, message) = failure.exit();
            eprintln!("{message}");
            ExitCode::from(code)
        }
    }
}
