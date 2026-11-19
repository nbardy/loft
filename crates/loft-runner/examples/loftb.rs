//! A compiler-free, device-free bundle loader used by the Phase 5 gate.
use loft_runner::bundle;
use std::{fs, time::Instant};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args_os().skip(1);
    let input = args
        .next()
        .ok_or("usage: loftb INPUT.loftb [REPACKED.loftb]")?;
    let output = args.next();
    if args.next().is_some() {
        return Err("expected one input and at most one output".into());
    }
    let load = || -> Result<_, Box<dyn std::error::Error>> {
        let started = Instant::now();
        let bytes = fs::read(&input)?;
        let decoded = bundle::decode(&bytes)?;
        Ok((bytes, decoded, started.elapsed()))
    };
    let _ = load()?;
    let (original, decoded, warm) = load()?;
    bundle::validate_wgsl(&decoded)?;
    let repacked = bundle::encode(&decoded);
    if repacked != original {
        return Err("bundle changed during round trip".into());
    }
    if let Some(path) = output {
        fs::write(path, repacked)?;
    }
    println!(
        "{} modules, {} tables, {} passes, {:.3} ms warm load",
        decoded.modules.len(),
        decoded.tables.len(),
        decoded.passes.len(),
        warm.as_secs_f64() * 1000.,
    );
    if warm.as_secs_f64() >= 0.010 {
        return Err("warm file read and decode exceeded 10 ms".into());
    }
    Ok(())
}
