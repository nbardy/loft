//! `loft`: the meta engine's command line.
//!
//!   loft check SCENE [--set NAME=VALUE]...      compile and run any scheduled checks at --t (default 0)
//!   loft show SCENE [MODULE] [--set ...]        print the generated WGSL (every module, or the one named)
//!   loft still SCENE OUT.png [--t SECONDS] [--set ...]
//!   loft still SCENE OUT.png --edit EDITED --kind K   recompile kind K from EDITED, swap it in live, check against a fresh load
//!   loft gate [--approve]                       the five gates on every scene in engines/gates.json
//!   loft diff REFERENCE.png FRAME.png          compare a Phase 5 frame with its reference
//!   loft load SCENE --nonce N [--set ...]       one load in this process with driver nonce N (cache experiments)
//!
//! A scene loads the core prelude and its reachable authored modules.

mod diff;
#[cfg(not(target_arch = "wasm32"))]
mod edit_cmd;
mod build;
mod render;
mod clip;
mod profile;
mod fmt;
mod fork;
mod context;
mod lint;

use loft_cli::{authored_checks, budget, budget::Failure, flag, overrides, render, sources, write_json};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

fn command(mut args: Vec<String>, set: &[(String, f64)]) -> Result<(), Failure> {
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
                loft_passes::compile(&src, set).map_err(|d| Failure::Compile(render(&d, &src)))?.0
            };
            if let Some(path) = telemetry {
                write_json(&path, &loft_passes::telemetry::collect(&program))?;
            }
            let text = loft_passes::sexp::write(&program).map_err(Failure::Compile)?;
            if let Some(out) = output { std::fs::write(out, text).map_err(|e| Failure::Usage(e.to_string()))?; }
            else if cmd == "ir" { println!("{text}"); }
            Ok(())
        }
        "build" if let Some(path) = flag(&mut args, "--json")? => {
            exact(&args, 2, 2)?;
            let src = sources(&scene(&args)?).map_err(Failure::Usage)?;
            let (program, _, _) = loft_passes::compile(&src, set)
                .map_err(|d| Failure::Compile(render(&d, &src)))?;
            write_json(&path, &loft_passes::telemetry::collect(&program))
        }
        "build" => build::run(&args[1..], set),
        "render" => render::run(&args[1..], set),
        "diff" => diff::run(&args[1..]),
        "clip" => clip::run(&args[1..], set),
        "profile" => profile::run(&args[1..], set),
        "lint" if set.is_empty() => lint::run(&args[1..]),
        "fmt" => fmt::run(&args[1..]),
        "show" => {
            let kind = flag(&mut args, "--kind")?;
            let json = args.iter().any(|a| a == "--json");
            let plan = args.iter().any(|a| a == "--plan");
            args.retain(|a| a != "--json" && a != "--plan");
            exact(&args, 2, if kind.is_some() { 2 } else { 3 })?;
            let src = sources(&scene(&args)?).map_err(Failure::Usage)?;
            let (program, _, _) = loft_passes::compile(&src, set)
                .map_err(|d| Failure::Compile(render(&d, &src)))?;
            profile::show(&program, kind.as_deref().or_else(|| args.get(2).map(String::as_str)), plan, json)
        }
        "check" => {
            exact(&args, 2, 2)?;
            let src = sources(&scene(&args)?).map_err(Failure::Usage)?;
            let (program, report, _) = loft_passes::compile(&src, set).map_err(|d| Failure::Compile(render(&d, &src)))?;
            loft_cli::native::motion_layout(&program).map_err(Failure::Compile)?;
            let mut failed = false;
            if program.dispatches.iter().any(|m| !m.checks.is_empty()) {
                let g = loft_cli::native::gpu().map_err(Failure::Device)?;
                for (name, verdict) in authored_checks(&g, &program, time).map_err(Failure::Device)? {
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
                #[cfg(target_arch = "wasm32")]
                {
                    let _ = (edit, kind);
                    return Err(Failure::Device("edited stills need the native renderer".into()));
                }
                #[cfg(not(target_arch = "wasm32"))]
                return edit_cmd::edited_still(&scene(&args)?, Path::new(&edit), &kind, set, time, Path::new(&args[2]), max_frame);
            }
            if kind.is_some() { return Err(Failure::Usage("still --kind needs --edit SOURCE".into())); }
            exact(&args, 3, 3)?;
            render::source_still(&scene(&args)?, set, time, Path::new(&args[2]), max_frame)
        }
        "load" => {
            let nonce: u32 = flag(&mut args, "--nonce")?.and_then(|s| s.parse().ok()).filter(|&n| n != 0)
                .ok_or_else(|| Failure::Usage("load needs --nonce N (a nonzero u32)".into()))?;
            exact(&args, 2, 2)?;
            let path = scene(&args)?;
            let g = loft_cli::native::gpu().map_err(Failure::Device)?;
            let src = sources(&path).map_err(Failure::Usage)?;
            // The Load gate's pair: a cold load (fresh derivation cache, new driver key), then the same load again.
            let cache = std::env::temp_dir().join(format!("loft_load_{nonce}"));
            std::fs::create_dir_all(&cache).map_err(|e| Failure::Usage(e.to_string()))?;
            for pass in ["cold", "warm"] {
                let t = std::time::Instant::now();
                let (p, report, _) = loft_passes::compile_cached(&src, set, &cache).map_err(|d| Failure::Compile(render(&d, &src)))?;
                let front = t.elapsed().as_secs_f64();
                let l = loft_cli::native::Loaded::new(&g, &p, (1280, 800), Some(nonce)).map_err(Failure::Device)?;
                println!("{pass} {:.3} s  front {front:.3} s  pipelines {:.1} ms ({}, slowest {:.0} ms)  ({}, precision {})",
                    t.elapsed().as_secs_f64(), l.stats.pipelines_ms, l.stats.pipeline_ms.len(),
                    l.stats.pipeline_ms.iter().map(|x| x.1).fold(0., f64::max), g.adapter, l.precision.name());
                println!("  front stages (ms): {}", report.ms.iter().map(|(n, ms)| format!("{n} {ms:.0}")).collect::<Vec<_>>().join("  "));
                if pass == "cold" {
                    let mut each = l.stats.pipeline_ms.clone();
                    each.sort_by(|a, b| b.1.total_cmp(&a.1));
                    for (name, ms) in each { println!("  {ms:9.1} ms  {name}"); }
                }
            }
            Ok(())
        }
        _ => Err(Failure::Usage("usage: loft build|check|show|still|clip|gate|load|ir|render|profile|controls|fmt|fork|context|diff|lint".into())),
    }
}

fn exec_sibling(name: &str, subcommand: &str, args: &[String]) -> Result<ExitCode, Failure> {
    let exe = std::env::current_exe().map_err(|e| Failure::Device(e.to_string()))?;
    let sibling = exe.with_file_name(format!("{name}{}", std::env::consts::EXE_SUFFIX));
    let mut child = std::process::Command::new(&sibling);
    child.arg(subcommand).args(args);
    let launch_error = |e: std::io::Error| match e.kind() {
        std::io::ErrorKind::NotFound => Failure::Device(format!("{} is missing. Build it with tools/dev.sh build {name} using the same profile as loft.", sibling.display())),
        _ => Failure::Device(format!("{}: {e}", sibling.display())),
    };
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        Err(launch_error(child.exec()))
    }
    #[cfg(not(unix))]
    child.status().map(|status| status.code().map_or(ExitCode::FAILURE, |code| ExitCode::from(code as u8))).map_err(launch_error)
}

fn dispatch(mut args: Vec<String>) -> Result<ExitCode, Failure> {
    match args.first().map(String::as_str) {
        Some("context") => return context::run(&args[1..]).map(|()| ExitCode::SUCCESS),
        Some("fork") => return fork::run(&args[1..]).map(|()| ExitCode::SUCCESS),
        Some(_) | None => {}
    }
    // Route using the same argument view as the original commands, then forward overrides unchanged.
    let mut parsed = args.clone();
    let set = overrides(&mut parsed).map_err(Failure::Usage)?;
    // The one precision control: widths are decided by role unless the author forces f32 everywhere.
    if let Some(choice) = flag(&mut parsed, "--precision")? {
        if choice != "f32" {
            return Err(Failure::Usage("--precision takes f32, the only choice; without it each role gets the width it needs".into()));
        }
        loft_cli::native::set_precision(loft_ir::precision::Precision::F32);
    }
    let trace_requested = |tail: &[String]| tail.iter().any(|a| {
        matches!(a.as_str(), "--trace" | "--record" | "--replay" | "--format")
    });
    let subcommand = match parsed.as_slice() {
        [cmd, ..] if matches!(cmd.as_str(), "controls" | "gate") => cmd.as_str(),
        [cmd, tail @ ..] if cmd == "check" && trace_requested(tail) => "trace-check",
        [cmd, tail @ ..] if cmd == "render" && trace_requested(tail) => "trace-render",
        [cmd, speed] if cmd == "profile" && speed == "--speed-gate" && set.is_empty() =>
            return exec_sibling("loft-verify", "gate", &parsed[1..]),
        _ => return command(parsed, &set).map(|()| ExitCode::SUCCESS),
    };
    // Overrides can precede the subcommand. Forward them in their original order.
    let i = args.chunks(2).take_while(|pair| pair[0] == "--set").count() * 2;
    args.remove(i);
    exec_sibling("loft-verify", subcommand, &args)
}

fn main() -> ExitCode {
    match dispatch(std::env::args().skip(1).collect()) {
        Ok(code) => code,
        Err(failure) => {
            let (code, message) = failure.exit();
            eprintln!("{message}");
            ExitCode::from(code)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fake_foot_20mm() {
        let fixture = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/fixtures/pin-drift.loft"
        );
        let check = |drift: &str| {
            dispatch(vec![
                "check".into(),
                fixture.into(),
                "--set".into(),
                format!("drift_m={drift}"),
            ])
        };
        check("0").unwrap();
        assert_eq!(check("0.020").unwrap_err().exit().0, 2);
    }
}
