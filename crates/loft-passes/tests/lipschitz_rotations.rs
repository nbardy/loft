use loft_passes::source::Graph;
use std::path::Path;

fn source(helpers: &str, trace: &str) -> Graph {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    Graph::text(&root, Path::new("rotations.loft"), &format!(r#"
use stdlib::materials::bsdf::{{emit_lobe}}
use stdlib::sdf::solid::*
space World
{helpers}
kind Probe @World () {{
  trace(p) {{
    {trace}
  }}
  material(p, n) = emit_lobe([0.5, 0.5, 0.5])
}}
build {{ emit Probe() }}
camera Main {{ at: [0, 0, 0]@World, radius: 3, fov: 38deg }}
"#))
}

// Sample both ordinary and local secants inside each derived AABB. The minimum
// distance keeps floating-point noise in the CPU twin below the signal.
fn measure(helpers: &str, trace: &str) -> (f64, f64) {
    let src = source(helpers, trace);
    let (program, report, twin) = loft_passes::compile(&src, &[])
        .unwrap_or_else(|d| panic!("{}", d.render(&src)));
    let bound = report.kinds[0].l_max;
    let shape = &program.instances[0];
    let field = |p| twin.cpu_query(0, 0, p, [0., 1., 0.]).unwrap().trace;
    let mut seed = 0x2dae_3202_9469_aad3u64;
    let mut uniform = || {
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
        (seed >> 40) as f32 / (1u32 << 24) as f32
    };
    let mut point = || std::array::from_fn::<_, 3, _>(|i| shape.lo[i] + uniform() * (shape.hi[i] - shape.lo[i]));
    let mut max_secant = 0.0f64;
    for i in 0..10_000 {
        let (a, mut b) = (point(), point());
        if i % 2 == 0 {
            b = std::array::from_fn(|j| a[j] + 0.08 * (b[j] - a[j]));
        }
        let distance = (0..3).map(|j| (f64::from(a[j]) - f64::from(b[j])).powi(2)).sum::<f64>().sqrt();
        let secant = (f64::from(field(a)) - f64::from(field(b))).abs() / distance;
        assert!(secant <= bound, "secant {secant} > L {bound}, {a:?} to {b:?}");
        max_secant = max_secant.max(secant);
    }
    eprintln!("L {bound:.9}, max secant {max_secant:.9}");
    (bound, max_secant)
}

#[test]
fn four_fold_ifs_keeps_unit_bound() {
    let (bound, max_secant) = measure(r#"
fn fold(p: p3@World) -> p3@World {
  let x = abs(p.x) * 1.45 - 0.42
  let y = abs(p.y) * 1.45 - 0.28
  let z = abs(p.z) * 1.45 - 0.35
  [0.8525245 * x + 0.5226872 * z, y, 0.8525245 * z - 0.5226872 * x]@World
}
"#, r#"
    let q = fold(fold(fold(fold(p))))
    let origin = [0, 0, 0]@World
    max(round_box(q - origin, [0.32, 0.45, 0.32], 0.04) / 4.420506,
        box_sdf(p - origin, [0.82, 0.90, 0.82]))
"#);
    assert!(bound <= 1.000_001, "four folds derive L {bound}");
    assert!(max_secant > 0.9);
}

#[test]
fn rotations_projections_and_scales_compose() {
    let (bound, _) = measure("", r#"
    let q = [2 * abs(p.x), -3 * p.y, 0.5 * abs(p.z)]@World
    map x = q |> rot_x(0.55)
    map y = x.p |> rot_y(-0.91)
    map z = y.p |> rot_z(1.12)
    let permuted = [z.p.z, z.p.y, z.p.x]
    length(permuted) - 0.7
"#);
    assert_eq!(bound, 3.0);

    let (bound, max_secant) = measure("", r#"
    let q = [abs(p.x) * 1.45 - 0.42, abs(p.y) * 1.45 - 0.28, abs(p.z) * 1.45 - 0.35]
    let x = q.x
    let z = q.z
    let c = 0.6
    let s = 0.80001
    let rotated = [c * x + s * z, q.y, c * z - s * x]
    length(rotated) - 0.7
"#);
    let gain = 1.45f32 as f64 * (0.6f32 as f64).hypot(0.80001f32 as f64);
    assert!((gain..gain + f64::from(f32::EPSILON)).contains(&bound));
    assert!(max_secant > 1.4);
}

#[test]
fn coupled_rows_and_distinct_folds_keep_sound_bounds() {
    for (field, min_secant, min_bound) in [
        ("length([p.x + p.z, p.x + p.z, p.z]) - 0.65", 1.9, 2.1358),
        ("sin(p.x * p.z) + 0.04", 0.8, 1.0),
        ("length(p.xxz) - 0.65", 1.35, 2.0f64.sqrt()),
        ("length([0.6 * abs(p.x) + 0.8 * p.z, p.y, 0.6 * p.z - 0.8 * abs(p.x - 0.5)]) - 0.65", 1.15, 1.4),
    ] {
        let (bound, max_secant) = measure("", &format!(r#"
    max({field}, box_sdf(p - [0, 0, 0]@World, [0.82, 0.90, 0.82]))
"#));
        assert!(bound >= min_bound, "{field}: derived L {bound}");
        assert!(max_secant >= min_secant, "{field}: max secant {max_secant}");
    }
}
