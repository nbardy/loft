//! These probes distinguish a correct image from two plausible mirrored-coordinate mistakes.
use loft_ir::{Sources, Span, ast::QueryKind};
use loft_passes::{
    compile,
    eval::{Eval, Val},
};
use std::{fs, path::PathBuf};
fn scene(name: &str) -> Sources {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut paths: Vec<_> = fs::read_dir(root.join("lib"))
        .unwrap()
        .map(|p| p.unwrap().path())
        .collect();
    paths.sort();
    paths.push(root.join(format!("agent_notes/2026-11-20_meta/v1_syntax/{name}.loft")));
    let mut src = Sources::default();
    for p in paths {
        src.add(p.display().to_string(), fs::read_to_string(p).unwrap());
    }
    src
}
fn delta(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    std::array::from_fn(|i| a[i] - b[i])
}
fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a.into_iter().zip(b).map(|(x, y)| x * y).sum()
}
fn unit(a: [f32; 3]) -> [f32; 3] {
    let n = dot(a, a).sqrt();
    a.map(|x| x / n)
}
fn vec3(x: [f32; 3]) -> Val<f32> {
    Val::V(x.into(), 0.)
}

#[test]
fn both_pupils_use_their_own_world_origin() {
    let src = scene("crab");
    let (_, _, c) = compile(&src, &[]).unwrap();
    let p = c.prog();
    let k = p.kind_index("Eyes");
    let f = p.kinds["Eyes"].query(QueryKind::Material).unwrap();
    let mut ev = Eval::<f32>::new(&p, &c.types, c.tables.clone(), &[]);
    let mut distinguishes_shared = 0;
    for time in [0., 2.0f32] {
        ev.time = time;
        let target = [0.55 * (0.4 * time).sin(), 0.72, 1.4];
        for side in [-1., 1.] {
            let center = [side * 0.15, 0.63, 0.36];
            let right_center = [0.15, 0.63, 0.36];
            let aim = unit(delta(target, center));
            for angle in -65..=75 {
                let a = (angle as f32).to_radians();
                let ray = unit([a.sin(), aim[1], a.cos()]);
                let q = std::array::from_fn(|i| center[i] + 0.07 * ray[i] - [0., 0.26, 0.32][i]);
                let Val::Bsdf(lobes) = ev
                    .call_fn(f, vec![Val::Row(k, 0), vec3(q), vec3(ray)], Span::NONE)
                    .unwrap()
                else {
                    panic!("material did not construct a BSDF")
                };
                let Val::V(color, _) = &lobes[0].1[0] else {
                    panic!("missing GGX albedo")
                };
                let expected = dot(ray, aim) > 0.91;
                assert_eq!(
                    color[0] < 0.02,
                    expected,
                    "side {side}, t {time}, angle {angle}"
                );
                distinguishes_shared +=
                    usize::from(expected != (dot(ray, unit(delta(target, right_center))) > 0.91));
            }
        }
    }
    assert!(
        distinguishes_shared > 30,
        "sample set cannot detect a shared origin"
    );
    println!("564 pupil samples, {distinguishes_shared} distinguish a shared right-eye origin");
}

#[test]
fn mirrored_fish_reverse_the_compiled_gpu_laws() {
    let g = loft_runner::gpu().expect("motion proof needs a GPU");
    let src = scene("heron");
    let mut yaw = vec![];
    for side in [1., -1.] {
        let (p, _, _) = compile(&src, &[("side".into(), side)]).unwrap();
        let read = |t| loft_runner::sample_motion(&g, &p, t).unwrap().remove(0);
        let held = read(1.1);
        let field = |n: &str| held.fields.iter().find(|(f, _)| f == n).unwrap().1[0];
        yaw.push(field("yaw"));
        assert!(held.checks.iter().all(|(_, c)| c[0][0] == 1.));
        let mut missed = false;
        for tick in 40..105 {
            let hop = read(tick as f32 / 120.);
            let lock = &hop.checks.iter().find(|(n, _)| n == "lock").unwrap().1;
            if lock[0][0] == 0. {
                assert!(lock[0][1] > 0.001);
                missed = true;
                break;
            }
        }
        assert!(missed, "neck limit never reported missed");
    }
    assert!(yaw[0] > 0.3 && yaw[1] < -0.3 && (yaw[0] + yaw[1]).abs() < 2e-5);
    println!(
        "132 GPU ticks, signed yaw {:.8} and {:.8} rad",
        yaw[0], yaw[1]
    );
}

#[test]
fn sexp_rebuilds_the_crab_with_its_shader_origins_and_pixels() {
    let source = scene("crab");
    let (program, _, _) = compile(&source, &[]).unwrap();
    let text = loft_ir::sexp::write(&program).unwrap();
    let restored = loft_ir::sexp::read(&text).unwrap();
    assert_eq!(loft_ir::sexp::write(&restored).unwrap(), text);
    assert!(
        restored
            .modules
            .iter()
            .flat_map(|m| &m.origins)
            .any(|o| o.span.0.ends_with("crab.loft") && o.span.1 > 1 && o.span.2 > 1)
    );
    let device = loft_runner::gpu().unwrap();
    let pixels = |p| {
        let loaded = loft_runner::Loaded::new(&device, p, (160, 100), None).unwrap();
        loaded.frame(&device, 2.0);
        loaded.pixels(&device).unwrap()
    };
    let actual = pixels(&restored);
    let expected = pixels(&program);
    assert_eq!(actual.len(), expected.len());
    assert_eq!(
        actual
            .iter()
            .zip(expected)
            .filter(|(a, b)| **a != *b)
            .count(),
        0
    );
}
