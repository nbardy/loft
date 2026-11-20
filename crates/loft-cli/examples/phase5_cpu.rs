use loft_passes::source::Graph;
use std::{collections::BTreeMap, error::Error, fs, path::{Path, PathBuf}};

fn main() -> Result<(), Box<dyn Error>> {
    let mut args = std::env::args_os().skip(1);
    let mut probes = BTreeMap::<String, Vec<(String, usize, [f32; 3])>>::new();
    for path in std::env::var_os("LOFT_SEED_PROBES").into_iter().flat_map(|p| std::env::split_paths(&p).collect::<Vec<_>>()) {
        for line in fs::read_to_string(path)?.lines() {
            let fields = line.split('\t').collect::<Vec<_>>();
            let &[file, kind, row, x, y, z] = fields.as_slice() else { return Err(format!("invalid probe: {line}").into()); };
            probes.entry(file.to_owned()).or_default().push((kind.to_owned(), row.parse()?, [x.parse()?, y.parse()?, z.parse()?]));
        }
    }
    let manifest = args.next().ok_or("missing scene inventory")?;
    let scenes = fs::read_to_string(manifest)?;
    let mut failures = 0;
    for scene in scenes.lines().map(Path::new).map(PathBuf::from) {
        let result = (|| -> Result<_, Box<dyn Error>> {
            let src = Graph::file(&scene, "")?;
            let (program, _, twin) = loft_passes::compile(&src, &[]).map_err(|e| e.render(&src))?;
            let (mut samples, mut error) = (0, 0_f32);
            if let Some(points) = probes.get(scene.file_stem().and_then(|n| n.to_str()).ok_or("invalid scene name")?) {
                for (kind, row, expected) in points {
                    let k = twin.drawn.iter().position(|(_, n)| n == kind).ok_or("unknown probe kind")?;
                    let q = twin.cpu_query(k, *row, [0.5, 0.23125, 0.], [0., 0., 1.]).map_err(|e| e.render(&src))?;
                    if !q.surf_p.into_iter().chain(q.surf_n).chain(q.material).all(f32::is_finite) {
                        return Err(format!("nonfinite query for {kind} row {row}").into());
                    }
                    error = error.max(q.surf_p.into_iter().zip(expected).map(|(a, b)| (a - b).powi(2)).sum::<f32>().sqrt());
                    samples += 1;
                }
                if error > 1e-5 { return Err(format!("pinned CPU geometry differs by {error:e} m").into()); }
            }
            for (kind, table) in program.kinds.iter().enumerate() {
                use loft_passes::program::Draw;
                for i in program.instances.iter().filter(|i| i.kind as usize == kind).take(3) {
                    let p = std::array::from_fn(|axis| 0.5 * (i.lo[axis] + i.hi[axis]));
                    let pos = if matches!(table.draw, Draw::Surface { .. }) { [0.372, 0.213, 0.] } else { p };
                    let q = twin.cpu_query(kind, i.row as usize, pos, [0., 0., -1.]).map_err(|e| e.render(&src))?;
                    if !q.material.into_iter().chain([q.trace, q.visible]).all(f32::is_finite)
                        || matches!(table.draw, Draw::Surface { .. })
                           && (0..3).any(|a| !(i.lo[a]..=i.hi[a]).contains(&q.surf_p[a]))
                    {
                        return Err(format!("query or bound failed on {} row {}", table.name, i.row).into());
                    }
                }
            }
            Ok(serde_json::json!({"pinned_cpu_samples": samples, "max_error_m": error,
                "kinds": program.kinds.len(), "authored_checks": program.dispatches.iter().map(|d| d.checks.len()).sum::<usize>()}))
        })();
        match result {
            Ok(report) => println!("{}", serde_json::json!({"scene": scene, "cpu": report})),
            Err(error) => {
                failures += 1;
                println!("{}", serde_json::json!({"scene": scene, "error": error.to_string()}));
            }
        }
    }
    if failures > 0 { return Err(format!("{failures} corpus candidates failed CPU validation").into()); }
    Ok(())
}
