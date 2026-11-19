//! These probes distinguish a correct image from two plausible mirrored-coordinate mistakes.
use loft_ir::{Sources, Span};
use loft_parse::ast::QueryKind;
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
        .filter(|p| p.extension().is_some_and(|ext| ext == "loft"))
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
                let loft_passes::eval::Layer::Lobe(_, args) = &lobes[0] else { panic!("expected GGX lobe") };
                let Val::V(color, _) = &args[0] else {
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
    let g = loft_cli::native::gpu().expect("motion proof needs a GPU");
    let src = scene("heron");
    let mut yaw = vec![];
    for side in [1., -1.] {
        let (p, _, _) = compile(&src, &[("side".into(), side)]).unwrap();
        let read = |t| loft_cli::native::sample_motion(&g, &p, t).unwrap().remove(0);
        let verdicts = |s: &loft_passes::program::MotionSample| {
            assert_eq!(s.checks.len(), 2);
            ["lock", "posture"].map(|name| {
                let records = &s.checks.iter().find(|(n, _)| n == name).unwrap().1;
                assert_eq!(records.len(), 1);
                records[0]
            })
        };
        let field = |s: &loft_passes::program::MotionSample, name| {
            s.fields.iter().find(|(n, _)| n == name).unwrap().1[0]
        };
        let mut airborne_miss = false;
        let mut landed_lock = false;
        for tick in 40..=132 {
            let sample = read(tick as f32 / 120.);
            let [lock, posture] = verdicts(&sample);
            let lift = field(&sample, "lift");
            assert_eq!(posture[0], 1.0, "posture failed at {tick} on side {side}");
            if lock[0] == 2.0 && lock[1] > 0.001 && lift > 0.01 {
                airborne_miss = true;
            }
            if airborne_miss && lift.abs() < 1e-5 && lock[0] == 1.0 {
                landed_lock = true;
            }
        }
        assert!(airborne_miss && landed_lock, "missing airborne loss then landing and lock on side {side}");
        let last = read(1.1);
        assert!(verdicts(&last).into_iter().all(|v| v[0] == 1.0));
        yaw.push(field(&last, "yaw"));
    }
    assert!(yaw[0] > 0.3 && yaw[1] < -0.3 && (yaw[0] + yaw[1]).abs() < 2e-5);
    println!(
        "132 GPU ticks, signed yaw {:.8} and {:.8} rad",
        yaw[0], yaw[1]
    );
}

#[test]
fn heron_checks_fail_for_reversed_pitch_and_a_displaced_landmark() {
    let g = loft_cli::native::gpu().unwrap();
    for (from, to, check) in [
        ("h.pitch = atan2", "h.pitch = -atan2", "lock"),
        ("const HIPS_Y = 0.65m", "const HIPS_Y = 1.2m", "posture"),
    ] {
        let mut src = scene("heron");
        let text = &mut src.files.last_mut().unwrap().1;
        assert!(text.contains(from));
        *text = text.replace(from, to);
        let (p, _, _) = compile(&src, &[]).unwrap();
        let samples = loft_cli::native::sample_motion(&g, &p, 1.1).unwrap();
        let v = samples[0].checks.iter().find(|(name, _)| name == check).unwrap().1[0];
        assert_eq!(v[0], 2., "changing {from} must fail {check}");
    }
}

#[test]
fn sexp_rebuilds_the_crab_with_its_shader_origins_and_pixels() {
    let source = scene("crab");
    let (program, _, _) = compile(&source, &[]).unwrap();
    let text = loft_passes::sexp::write(&program).unwrap();
    let restored = loft_passes::sexp::read(&text).unwrap();
    assert_eq!(loft_passes::sexp::write(&restored).unwrap(), text);
    assert!(
        restored
            .modules
            .iter()
            .flat_map(|m| &m.origins)
            .any(|o| o.span.0.ends_with("crab.loft") && o.span.1 > 1 && o.span.2 > 1)
    );
    let device = loft_cli::native::gpu().unwrap();
    let pixels = |p| {
        let loaded = loft_cli::native::Loaded::new(&device, p, (160, 100), None).unwrap();
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

fn source(text: &str) -> Sources {
    let mut src = scene("heron");
    src.files.pop();
    src.add("review.loft", text);
    src
}

#[test]
fn map_y_rotation_keeps_cpu_gpu_and_culling_in_the_same_space() {
    let g = loft_cli::native::gpu().unwrap();
    for (angle, x) in [(90, -1.), (-90, 1.)] {
        let src = source(&format!("space World
kind Orb @World () {{
  map turn(q) = q |> rot_y({angle}deg)
  trace(q) = sphere(turn.p - turn.in(turn.out(turn.pt(0,0,1))), 0.1)
  material(q,n) = emit_lobe(rgb(distance(q, turn.out(turn.in(q)))))
}}
build {{ emit Orb() }}"));
        let (p, _, c) = compile(&src, &[]).unwrap();
        let center = [x, 0., 0.];
        let i = &p.instances[0];
        assert!((0..3).all(|j| i.lo[j] < center[j] && center[j] < i.hi[j]));
        let l = loft_cli::native::Loaded::new(&g, &p, (96, 64), None).unwrap();
        l.frame(&g, 0.);
        for (pos, expected) in [(center, -0.1), ([-x, 0., 0.], 1.9)] {
            let cpu = c.cpu_query(0, 0, pos, [0., 0., 1.]).unwrap();
            let gpu = l.twin(&g, 0, &[pos[0], pos[1], pos[2], 0., 0., 0., 1., 0.]).unwrap();
            assert!((cpu.trace - expected).abs() < 1e-5);
            assert!((gpu[0] - expected).abs() < 1e-5);
            assert!(cpu.material.iter().all(|v| v.abs() < 1e-5));
            assert!(gpu[4..7].iter().all(|v| v.abs() < 1e-5));
        }
    }
}

#[test]
fn loop_binder_shadows_only_inside_the_gpu_and_cpu_loop() {
    let src = source("space World
kind Orb @World (scale: f = 2) {
  trace(q) {
    var sum: f = 0
    for scale in 0..2 { sum = sum + f32(scale) }
    sphere(q - [0,0,0]@World, (sum + scale) / 10)
  }
  material(q,n) = ggx(rgb(0.5), 0.5, 0)
}
build { emit Orb() }");
    let (p, _, c) = compile(&src, &[]).unwrap();
    let g = loft_cli::native::gpu().unwrap();
    let l = loft_cli::native::Loaded::new(&g, &p, (80, 60), None).unwrap();
    l.frame(&g, 0.);
    assert!((c.cpu_query(0, 0, [0.; 3], [0., 0., 1.]).unwrap().trace + 0.3).abs() < 1e-5);
    assert!((l.twin(&g, 0, &[0., 0., 0., 0., 0., 0., 1., 0.]).unwrap()[0] + 0.3).abs() < 1e-5);
}

#[test]
fn overrides_change_data_and_pixels_without_changing_shader_text() {
    let g = loft_cli::native::gpu().unwrap();
    let mut src = scene("heron");
    let text = &mut src.files.last_mut().unwrap().1;
    *text = text.replace("at: [0, 0.65m, 0]@World, radius: 3m", "at: [0, 0.2m, 0]@World, radius: 5m");
    let a = compile(&src, &[("side".into(), 1.)]).unwrap().0;
    let b = compile(&src, &[("side".into(), -1.)]).unwrap().0;
    assert!(a.modules.iter().map(|m| &m.wgsl).eq(b.modules.iter().map(|m| &m.wgsl)));
    assert_ne!(a.params, b.params);
    let pixels = |p| {
        let l = loft_cli::native::Loaded::new(&g, p, (128, 80), None).unwrap();
        l.frame(&g, 0.45);
        l.pixels(&g).unwrap()
    };
    assert!(pixels(&a) != pixels(&b), "the override must affect rendered pixels");

    let body = "space World
kind Orb @World () {
  trace(q) = sphere(q - [0,0,0]@World, 0.5)
  material(q,n) = ggx(rgb(0.5), 0.5, 0)
}
build { emit Orb() }
";
    let a = compile(&source(body), &[]).unwrap().0;
    let b = compile(&source(&format!("{body}build {{ emit Sky(zenith: [1,0,0], horizon: [0,1,0]) }}")), &[]).unwrap().0;
    assert!(a.modules.iter().map(|m| &m.wgsl).eq(b.modules.iter().map(|m| &m.wgsl)));
    assert_ne!(a.params, b.params);
    assert!(pixels(&a) != pixels(&b), "the override must affect rendered pixels");
}

#[test]
fn scheduled_checks_cover_rows_that_have_no_draw() {
    let src = source("space World
kind Ledger @World (value: f = 0) {}
law advance_row(h: Ledger, dt) writes h.{value} { h.value = h.value + 1 }
check data_only(h: Ledger) -> f { if h.value == 12 { held(1) } else { missed(1) } }
build { emit Ledger(); emit Ledger() }
schedule Main { run_laws(advance_row); check(data_only) }");
    let (p, _, _) = compile(&src, &[]).unwrap();
    assert!(p.kinds.is_empty());
    let g = loft_cli::native::gpu().unwrap();
    let result = loft_cli::native::sample_motion(&g, &p, 0.1).unwrap();
    assert_eq!(result[0].fields[0].1, [12., 12.]);
    assert_eq!(result[0].checks[0].1, [[1., 1.], [1., 1.]]);
}

#[test]
fn fixed_motion_math_covers_direct_and_transitive_calls() {
    let src = source("space World
kind Data @World (x: f = 0) {}
fn wrapped(x: f) -> f = acos(x) + log(x)
law apply(h: Data, dt) writes h.{x} { h.x = exp(dt) + wrapped(0.5) }
build { emit Data() }
schedule Main { run_laws(apply) }");
    let (p, _, _) = compile(&src, &[]).unwrap();
    let wgsl = &p.modules[p.dispatches[0].module].wgsl;
    let law = wgsl.split("fn law_apply").nth(1).unwrap().split("@compute").next().unwrap();
    let helper = wgsl.split("fn motion_loft_user_wrapped").nth(1).unwrap().split("fn law_apply").next().unwrap();
    assert!(law.contains("det_exp(loft_local_dt)") && !law.contains("= exp("));
    assert!(helper.contains("det_acos(") && helper.contains("det_log("));
    let g = loft_cli::native::gpu().unwrap();
    let samples = loft_cli::native::sample_motion(&g, &p, 1. / 120.).unwrap();
    let want = (1.0f32 / 120.).exp() + 0.5f32.acos() + 0.5f32.ln();
    assert!((samples[0].fields[0].1[0] - want).abs() < 2e-6);
}

#[test]
fn geometry_cache_follows_resolved_functions_and_only_used_scans() {
    let mut src = scene("crab");
    let cache = tempfile::TempDir::new().unwrap();
    let run = |src: &Sources| loft_passes::compile_cached(src, &[], cache.path()).unwrap().1;
    let get = |r: &loft_passes::Report, kind| {
        let kind = r.kinds.iter().find(|k| k.name == kind).unwrap();
        (kind.cache_hits, kind.shapes)
    };
    let first = run(&src);
    assert_eq!(get(&first, "Coil"), (0, 1));
    let sdf = src.files.iter().position(|(name, _)| name.ends_with("/sdf.loft")).unwrap();
    src.files[sdf].1.insert_str(0, "# Formatting does not change an analytic certificate.\n");
    let text = &mut src.files.last_mut().unwrap().1;
    let unused = text[text.find("scan coil").unwrap()..text.find("fn roughness").unwrap()]
        .replace("scan coil", "scan unused")
        .replace("by 192", "by 12");
    text.push_str(&unused);
    let second = run(&src);
    assert!(second.kinds.iter().all(|k| k.cache_hits == k.shapes));
    let text = &mut src.files.last_mut().unwrap().1;
    *text = text.replace("by 192", "by 193");
    let third = run(&src);
    assert_eq!(get(&third, "Coil"), (0, 1));
    assert_eq!(get(&third, "Pincer"), get(&second, "Pincer"));

    src.files[sdf].1 = src.files[sdf].1.replace("length(v) - r", "length(v) - (r + 0.002)");
    let fourth = run(&src);
    assert_eq!(get(&fourth, "Eyes").0, 0, "a changed sphere must rederive the eye bound");
    assert_eq!(get(&fourth, "Coil"), (1, 1), "the coil does not call sphere");
}
