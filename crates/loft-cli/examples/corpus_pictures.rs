//! Paired ordered GPU captures of the generated corpus, scored by loft gate.
use std::{error::Error, fs, io::Write, path::Path, process::{Command, Stdio}};

use loft_native::Loaded;
use loft_passes::{program::Program, source::Graph};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

#[path = "../../../tools/gates/ordered_cull.rs"]
mod ordered_cull;

fn compile(root: &Path, file: &str) -> Result<Program, Box<dyn Error>> {
    let graph = Graph::file(&root.join(file), "")?;
    let (mut program, _, _) = loft_passes::compile(&graph, &[]).map_err(|d| d.render(&graph))?;
    ordered_cull::order_cull(&mut program)?;
    Ok(program)
}

fn score(
    gpu: &loft_native::Gpu, scorer: &Path, file: &str, main: &Program, candidate: &Program,
    sizes: &[((u32, u32), Vec<f32>)], work: &Path,
) -> Result<Value, Box<dyn Error>> {
    for (a, b) in main.kinds.iter().zip(&candidate.kinds) {
        if a.name != b.name || a.stride != b.stride || a.n_rows != b.n_rows ||
            a.rows.len() != b.rows.len() || a.rows.iter().zip(&b.rows).any(|(x, y)| x.to_bits() != y.to_bits())
        {
            return Err(format!("{file}: packed rows differ for {}", a.name).into());
        }
    }
    if main.kinds.len() != candidate.kinds.len() {
        return Err(format!("{file}: kind count changed").into());
    }

    let baseline = work.join("baseline");
    let revision = work.join("candidate");
    fs::create_dir(&baseline)?;
    fs::create_dir(&revision)?;
    let mut requests = serde_json::Map::new();
    for &(size, ref times) in sizes {
        let a = Loaded::new(gpu, main, size, None)?;
        let b = Loaded::new(gpu, candidate, size, None)?;
        for time in times {
            let stem = format!("{}x{}_t{time}", size.0, size.1);
            let mut baseline_hash = String::new();
            for (loaded, out) in [(&a, &baseline), (&b, &revision)] {
                loaded.frame(gpu, *time);
                let rgba = loaded.pixels(gpu)?;
                if out == &baseline {
                    baseline_hash = format!("{:x}", Sha256::digest(&rgba));
                }
                let mut png = png::Encoder::new(
                    fs::File::create(out.join(format!("{stem}.png")))?, size.0, size.1,
                );
                png.set_color(png::ColorType::Rgba);
                png.set_depth(png::BitDepth::Eight);
                png.write_header()?.write_image_data(&rgba)?;
                let (id, depth) = loaded.picture_planes(gpu)?;
                fs::write(out.join(format!("{stem}.id.u32")), id)?;
                fs::write(out.join(format!("{stem}.depth.f32")), depth)?;
            }
            requests.insert(stem.clone(), json!({"deterministic": {
                "size": size, "approved_hash": baseline_hash,
                "approved": baseline.join(format!("{stem}.png")),
                "baseline": baseline, "candidate": revision, "stem": stem,
            }}));
        }
    }
    let mut child = Command::new(scorer).args(["gate", "--score-pictures"])
        .stdin(Stdio::piped()).stdout(Stdio::piped()).spawn()?;
    child.stdin.take().unwrap().write_all(&serde_json::to_vec(&requests)?)?;
    let output = child.wait_with_output()?;
    if !output.status.success() {
        return Err(format!("{file}: picture scorer returned {}", output.status).into());
    }
    Ok(serde_json::from_slice(&output.stdout)?)
}

fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    let [main, candidate, scorer, out] = args.as_slice() else {
        return Err("usage: corpus_pictures BASELINE_ROOT CANDIDATE_ROOT LOFT EVIDENCE_DIR < scene-paths.txt".into());
    };
    let main = Path::new(main);
    let candidate = Path::new(candidate);
    let scorer = Path::new(scorer);
    let out = Path::new(out).canonicalize()?;
    let manifest: Value = serde_json::from_slice(&fs::read(candidate.join("corpus/sources.json"))?)?;
    let frames = &manifest["frames"];
    let gpu = loft_native::gpu()?;
    eprintln!("adapter {}", gpu.adapter);
    let mut held = true;
    for file in std::io::stdin().lines() {
        let file = file?;
        let work = tempfile::tempdir_in(&out)?;
        let result = (|| -> Result<Value, Box<dyn Error>> {
            let before = compile(main, &file)?;
            let after = compile(candidate, &file)?;
            let mut times: Vec<f32> = serde_json::from_value(frames["times"].clone())?;
            if file.ends_with("crab-actions-v1__v1__train-16__2.loft") {
                times.extend([0.125, 0.84, 1.1, 2.1]);
            }
            let sizes = [
                (serde_json::from_value(frames["size"].clone())?, times),
                (serde_json::from_value(frames["detail"]["size"].clone())?,
                 vec![serde_json::from_value(frames["detail"]["time"].clone())?]),
            ];
            score(&gpu, scorer, &file, &before, &after, &sizes, work.path())
        })();
        match result {
            Ok(pictures) => {
                let matches = pictures.as_object().is_some_and(|frames|
                    frames.values().all(|frame| frame["held"] == true));
                held &= matches;
                let evidence = if matches { None } else { Some(work.keep()) };
                println!("{}", json!({"scene": file, "pictures": pictures, "evidence": evidence}));
            }
            Err(error) => {
                held = false;
                println!("{}", json!({"scene": file, "error": error.to_string(), "evidence": work.keep()}));
            }
        }
    }
    if !held {
        return Err("one or more corpus picture cases failed".into());
    }
    Ok(())
}
