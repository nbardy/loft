//! End to end, for the rules added for the Auncel findings: compile a trace that uses them, then sample secants of
//! the CPU twin inside the derived AABB against the derived L (the method of `lipschitz_rotations.rs`). A secant
//! above L is a march step that can pass through the surface.

use loft_passes::source::Graph;
use std::path::Path;

fn measure(trace: &str) -> (f64, f64) {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let src = Graph::text(&root, Path::new("inverse_trig.loft"), &format!(r#"
use stdlib::materials::bsdf::{{emit_lobe}}
space World
kind Probe @World () {{
  trace(p) {{
    {trace}
  }}
  material(p, n) = emit_lobe([0.5, 0.5, 0.5])
}}
build {{ emit Probe() }}
camera Main {{ at: [0, 0, 0]@World, radius: 3, fov: 38deg }}
"#));
    let (program, report, twin) = loft_passes::compile(&src, &[]).unwrap_or_else(|d| panic!("{}", d.render(&src)));
    let bound = report.kinds[0].l_max;
    let shape = &program.instances[0];
    let field = |p| twin.cpu_query(0, 0, p, [0., 1., 0.]).unwrap().trace;
    let mut seed = 0x9e37_79b9_7f4a_7c15u64;
    let mut uniform = || {
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
        (seed >> 40) as f32 / (1u32 << 24) as f32
    };
    let mut point = || std::array::from_fn::<_, 3, _>(|i| shape.lo[i] + uniform() * (shape.hi[i] - shape.lo[i]));
    let mut max_secant = 0.0f64;
    for i in 0..4_000 {
        let (a, mut b) = (point(), point());
        if i % 2 == 0 {
            b = std::array::from_fn(|j| a[j] + 0.02 * (b[j] - a[j]));
        }
        let distance = (0..3).map(|j| (f64::from(a[j]) - f64::from(b[j])).powi(2)).sum::<f64>().sqrt();
        if distance < 1e-4 {
            continue;
        }
        let secant = (f64::from(field(a)) - f64::from(field(b))).abs() / distance;
        assert!(secant <= bound, "secant {secant} > L {bound}, {a:?} to {b:?}");
        max_secant = max_secant.max(secant);
    }
    (bound, max_secant)
}

#[test]
fn inverse_trig_squares_and_time_branches_bound_the_twin() {
    for (name, trace) in [
        ("acos asin atan", "let c = clamp(p.y, -0.5, 0.5)\n    length(coords(p)) - 0.7 + 0.1 * (acos(c) + asin(c) + atan(3.0 * p.x))"),
        // The log's argument is clamped above 0. `log(1.5 + p.x)` is NaN for x <= -1.5, so the bound search could
        // not exclude that half of the search region and refused the kind (E212), which is the rule working.
        ("tan log", "length(coords(p)) - 0.7 + 0.1 * (tan(clamp(p.y, -1.2, 1.2)) + log(2.0 + clamp(p.x, -1.0, 1.0)))"),
        ("atan2 off the axis", "let q = coords(p) - [2.0, 0.0, 0.0]\n    length(q) - 0.7 + 0.1 * sin(6.0 * atan2(q.z, q.x + 2.0))"),
        ("squares", "let q = coords(p)\n    length(q) - 0.7 + 0.1 / (1.0 + 8.0 * min(q.x * q.x, 0.35)) + 0.05 / (1.0 + dot(q, q))"),
        ("select on time", "select(length(coords(p)) - 0.7, length(coords(p)) - 0.5, time > 3.0)"),
        ("if on time", "if time > 3.0 {\n      return length(coords(p)) - 0.5\n    }\n    length(coords(p)) - 0.7"),
    ] {
        let (bound, max_secant) = measure(trace);
        eprintln!("{name:20} L {bound:.4}  max secant {max_secant:.4}");
        assert!(max_secant > 0.5, "{name}: the sample never crossed the field");
    }
}
