//! Repository verification commands, also invoked by the legacy Loft CLI.
mod controls;
mod suite;
mod trace;

use loft_cli::{budget::Failure, flag, overrides};
use std::process::ExitCode;

fn command(mut args: Vec<String>) -> Result<(), Failure> {
    let set = overrides(&mut args).map_err(Failure::Usage)?;
    let usage = || Failure::Usage(
        "usage: loft-verify controls|gate|score-pictures|trace-check|trace-render".into(),
    );
    let cmd = args.first().ok_or_else(usage)?.clone();
    match cmd.as_str() {
        "controls" => controls::run(&args[1..], &set),
        "trace-check" | "trace-render" => {
            args.retain(|a| a != "--trace");
            trace::run(&args[1..], &set, cmd == "trace-render")
        }
        "gate" if args[1..] == ["--speed-gate"] && set.is_empty() => suite::speed_gate(),
        "gate" if args.get(1).is_some_and(|x| x == "--score-pictures") => {
            score_pictures(args.len() == 2 && set.is_empty())
        }
        "score-pictures" => score_pictures(args.len() == 1 && set.is_empty()),
        "gate" => {
            let only = flag(&mut args, "--only")?;
            let approve = args.get(1).is_some_and(|s| s == "--approve");
            if !set.is_empty() || args.len() != 1 + usize::from(approve) {
                return Err(Failure::Usage("usage: loft-verify gate [--only NAME,...] [--approve]".into()));
            }
            suite::run(approve, only.as_deref())
        }
        _ => Err(usage()),
    }
}

fn score_pictures(valid: bool) -> Result<(), Failure> {
    if !valid {
        return Err(Failure::Usage("picture scoring reads capture metadata from stdin".into()));
    }
    suite::score_pictures()
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
