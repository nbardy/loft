use loft_ir::Sources;
use std::path::Path;
#[test]
fn hermit() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut src = Sources::default();
    let mut files: Vec<_> = std::fs::read_dir(root.join("lib")).unwrap().map(|e| e.unwrap().path()).collect();
    files.sort();
    files.push(root.join(std::env::var("SCENE").unwrap_or("engines/hermit.loft".into())));
    for f in files { src.add(f.display().to_string(), std::fs::read_to_string(&f).unwrap()); }
    let n: f64 = std::env::var("COUNT").ok().and_then(|x| x.parse().ok()).unwrap_or(1.0);
    match loft_passes::compile(&src, &[("COUNT".into(), n)]) {
        Ok((p, r, c)) => {
            for i in 0..12 { let u = 0.80 + 0.005 * i as f32; let q = c.cpu_query(1, 0, [u, 0.25, 0.0], [0.0, 0.0, 1.0]).unwrap(); eprintln!("u {u:.3} p {:?} n {:?}", q.surf_p, q.surf_n); }
            for l in &c.luts { for i in [0usize, 1, 64, 128, 200, 255] { eprintln!("{} {i} {:?}", l.name, l.frames[i].map(|x| (x * 1000.0).round() / 1000.0)); } }
            for k in &r.kinds { eprintln!("{} {:?} rows {} shapes {} L {}..{} evals {}", k.name, k.draw, k.rows, k.shapes, k.l_min, k.l_max, k.evals); }
            eprintln!("modules {} ms {:?}", p.modules.len(), r.ms);
        }
        Err(d) => panic!("{}", d.render(&src)),
    }
}
