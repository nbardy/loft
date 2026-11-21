//! Rendering and command support shared by the CLI and repository instruments.
#[path = "build/artifact.rs"]
pub mod artifact;
pub mod budget;
pub use loft_native as native;

use budget::Failure;
use loft_ir::{Diag, Sources, Verdict};
use loft_passes::{program::Program, source::Graph};
use native::Gpu;
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::path::Path;

pub fn sources(scene: &Path) -> Result<Graph, String> {
    Graph::file(scene, "")
}

pub fn render(d: &Diag, src: &Sources) -> String {
    d.render(src)
}

/// `--set NAME=VALUE` pairs.
pub fn overrides(args: &mut Vec<String>) -> Result<Vec<(String, f64)>, String> {
    let mut out = vec![];
    while let Some(i) = args.iter().position(|a| a == "--set") {
        let kv = args.get(i + 1).cloned().ok_or("--set needs NAME=VALUE")?;
        args.drain(i..i + 2);
        let (k, v) = kv.split_once('=').ok_or("--set needs NAME=VALUE")?;
        out.push((k.to_string(), v.parse::<f64>().map_err(|_| format!("--set {k}: `{v}` is not a number"))?));
    }
    Ok(out)
}

pub fn flag(args: &mut Vec<String>, name: &str) -> Result<Option<String>, Failure> {
    let Some(i) = args.iter().position(|a| a == name) else { return Ok(None); };
    let v = args.get(i + 1).filter(|a| !a.starts_with("--")).cloned()
        .ok_or_else(|| Failure::Usage(format!("{name} requires a value")))?;
    args.drain(i..i + 2);
    Ok(Some(v))
}

pub fn shader_hashes(program: &Program) -> Result<Vec<(String, String)>, String> {
    program.modules.iter().map(|module| {
        let bytes = serde_json::to_vec(&module.naga).map_err(|err| err.to_string())?;
        Ok((module.name.clone(), format!("{:x}", Sha256::digest(bytes))))
    }).collect()
}

pub fn write_json(path: &str, value: &impl Serialize) -> Result<(), Failure> {
    let data = serde_json::to_string_pretty(value).map_err(|e| Failure::Usage(e.to_string()))?;
    if path == "-" {
        println!("{data}");
        Ok(())
    } else {
        std::fs::write(path, data + "\n").map_err(|e| Failure::Usage(format!("{path}: {e}")))
    }
}

pub fn write_png(path: &Path, size: (u32, u32), rgba: &[u8]) -> Result<(), String> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let f = std::fs::File::create(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut e = png::Encoder::new(std::io::BufWriter::new(f), size.0, size.1);
    e.set_color(png::ColorType::Rgba);
    e.set_depth(png::BitDepth::Eight);
    e.write_header().and_then(|mut w| w.write_image_data(rgba)).map_err(|e| e.to_string())
}

fn decode_verdict(record: Option<[f32; 2]>) -> Verdict {
    match record {
        Some([1.0, value]) if value.is_finite() && value >= 0.0 => Verdict::Held { margin: value.into() },
        Some([2.0, value]) if value.is_finite() && value >= 0.0 => Verdict::Missed { by: value.into() },
        Some([3.0, _]) => Verdict::Unknown { reason: "authored check returned Unknown".into() },
        _ => Verdict::Unknown { reason: "missing or nonfinite check record".into() },
    }
}

pub fn check_records(p: &Program, samples: &[loft_passes::program::MotionSample]) -> Vec<(String, Verdict)> {
    let mut results = vec![];
    for step in &p.dispatches {
        let (kind, rows, _, _) = step.input.view(&p.kinds).expect("compiled dispatch input");
        for name in &step.checks {
            let mut records = samples.iter().filter(|s| s.kind == kind)
                .flat_map(|s| &s.checks).filter(|(n, _)| n == name).map(|(_, v)| v);
            let values = records.next().filter(|v| records.next().is_none() && v.len() == rows as usize);
            for row in 0..rows as usize {
                results.push((format!("{kind}[{row}].{name}"), decode_verdict(values.map(|v| v[row]))));
            }
        }
    }
    results
}

pub fn authored_checks(g: &Gpu, p: &Program, t: f32) -> Result<Vec<(String, Verdict)>, String> {
    if p.dispatches.iter().all(|m| m.checks.is_empty()) { return Ok(vec![]); }
    native::sample_motion(g, p, t).map(|samples| check_records(p, &samples))
}

pub fn quantile(xs: &[f64], q: f64) -> f64 {
    let mut s = xs.to_vec();
    s.sort_by(f64::total_cmp);
    s[((s.len() - 1) as f64 * q).round() as usize]
}

pub fn write(path: &Path, bytes: &[u8]) -> Result<(), Failure> {
    use std::io::Write;
    let err = |e: std::io::Error| Failure::Usage(format!("{}: {e}", path.display()));
    let dir = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let mut file = tempfile::NamedTempFile::new_in(dir).map_err(err)?;
    file.write_all(bytes).map_err(err)?;
    file.persist(path).map_err(|e| err(e.error))?;
    Ok(())
}

pub fn live_names<'a>(prog: &'a loft_passes::Prog<'_>) -> impl Iterator<Item = &'a str> {
    use loft_parse::ast::TyExpr;
    prog.consts.iter().filter_map(|(&name, c)| {
        (c.comptime && matches!(&c.ty, Some(TyExpr::Name(n)) if n.name == "f32")).then_some(name)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn authored_verdicts_require_a_record_and_finite_evidence() {
        for record in [None, Some([0.0, 0.0]), Some([1.0, f32::NAN]), Some([3.0, 0.0])] {
            assert!(matches!(decode_verdict(record), Verdict::Unknown { .. }));
        }
        assert!(matches!(decode_verdict(Some([2.0, 1.0])), Verdict::Missed { .. }));
        assert!(matches!(decode_verdict(Some([1.0, 1.0])), Verdict::Held { .. }));
    }
}
