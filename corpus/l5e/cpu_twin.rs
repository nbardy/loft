use loft_ir::Sources;
use std::{collections::BTreeMap, fs, path::Path};

#[test]
fn emitted_sweep_positions_and_materials() {
    let mut lib: Vec<_> = fs::read_dir("lib").unwrap().map(|e| e.unwrap().path()).collect();
    lib.retain(|p| p.extension().is_some_and(|e| e == "loft"));
    lib.sort();

    let probes = fs::read_to_string("out/meta/p5/l5e/cpu-probes.tsv").unwrap();
    let mut scenes = BTreeMap::<_, Vec<_>>::new();
    for line in probes.lines() {
        let fields: Vec<_> = line.split('\t').collect();
        let &[name, kind, row, x, y, z] = fields.as_slice() else { panic!("{line}") };
        let point: [f32; 3] = [x, y, z].map(|c| c.parse().unwrap());
        scenes.entry(name).or_default().push((kind, row.parse::<usize>().unwrap(), point));
    }
    assert_eq!(scenes.len(), 75);

    let mut max_error = 0.0_f32;
    let mut sample_count = 0;
    for (name, rows) in scenes {
        let mut src = Sources::default();
        let scene = Path::new("corpus/l5e").join(format!("{name}.loft"));
        for path in lib.iter().chain([&scene]) {
            src.add(path.display().to_string(), fs::read_to_string(path).unwrap());
        }
        let (_, _, twin) = loft_passes::compile(&src, &[]).unwrap_or_else(|d| panic!("{}", d.render(&src)));
        for (kind, row, expected) in rows {
            let k = twin.drawn.iter().position(|(_, n)| n == kind).unwrap();
            let q = twin.cpu_query(k, row, [0.5, 0.23125, 0.0], [0.0, 0.0, 1.0]).unwrap();
            assert!(q.surf_n.into_iter().chain(q.material).all(f32::is_finite), "{name} {kind} {row}");
            let error = q.surf_p.into_iter().zip(expected).map(|(a, b)| (a - b).powi(2)).sum::<f32>().sqrt();
            assert!(error <= 1e-5, "{name} {kind} {row}: {error} m");
            max_error = max_error.max(error);
            sample_count += 1;
        }
    }
    println!("{sample_count} strip samples, max position error {max_error:e} m");
}
