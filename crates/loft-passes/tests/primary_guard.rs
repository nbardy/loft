//! Regression: treating the zero set as the hit set drops a positive plateau
//! which the original marcher accepts from a sufficiently distant camera.
use loft_passes::{compile_cached, source::Graph};
use std::path::Path;

#[test]
fn positive_hit_band_and_original_proxy_are_both_preserved() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let src = Graph::text(
        &root,
        Path::new("guard.loft"),
        r#"
use stdlib::materials::bsdf::{emit_lobe}
space World
kind Probe @World () {
  trace(p) {
    let d = length(p.xyz) - 0.2
    min(d, 0.00015)
  }
  material(p,n) = emit_lobe([1.0,1.0,1.0])
}
build { emit Probe() }
camera Main { at: [0,0,0]@World, radius: 3, fov: 40deg }
"#,
    );
    let cache = tempfile::tempdir().unwrap();
    let compile =
        || compile_cached(&src, &[], cache.path()).unwrap_or_else(|e| panic!("{}", e.render(&src)));
    let (p, _, _) = compile();
    let (cached, report, _) = compile();
    assert_eq!(report.kinds[0].evals, 0);
    assert_eq!(p.luts, cached.luts);
    let instance = p.instances[0];
    let kind = &p.kinds[0];
    assert_eq!(&kind.rows[..3], &instance.lo);
    assert_eq!(&kind.rows[3..6], &instance.hi);
    assert!(
        p.modules[kind.module]
            .wgsl
            .contains("Restrict only the exit")
    );
    let offset = p.luts[0].to_bits() as usize;
    for i in 0..3 {
        assert!(p.luts[offset + i] <= instance.lo[i]);
        assert!(p.luts[offset + 3 + i] >= instance.hi[i]);
    }
}

#[test]
fn cancelled_coordinate_keeps_gpu_hits_outside_the_real_field() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let src = Graph::text(&root, Path::new("cancel.loft"), r#"
use stdlib::materials::bsdf::{emit_lobe}
space World
kind Probe @World () {
  trace(p) = length([(p.x + 16777216.0) - 16777216.0, p.y, p.z]) - 0.005
  material(p,n) = emit_lobe([1.0,1.0,1.0])
}
build { emit Probe() }
camera Main { at: [0,0,0]@World, radius: 3, fov: 40deg }
"#);
    let cache = tempfile::tempdir().unwrap();
    let (p, _, _) = compile_cached(&src, &[], cache.path()).unwrap_or_else(|d| panic!("{}", d.render(&src)));
    let i = p.instances[0];
    // In real arithmetic the field is a 5 mm ball. In f32 the entire old
    // x interval at y=z=0 is inside, including its two extreme samples.
    let offset = p.luts[0].to_bits() as usize;
    assert!(p.luts[offset + 9] > 0.0);
    for x in [i.lo[0], i.hi[0]] {
        assert!(x.abs() > 0.03);
        assert_eq!((x + 16_777_216.0_f32) - 16_777_216.0_f32, 0.0);
        assert!(p.luts[offset] <= x && x <= p.luts[offset + 3]);
    }
}

#[test]
fn an_indexed_surface_with_a_closed_form_query_keeps_its_grid_draw() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let src = Graph::text(&root, Path::new("hybrid.loft"), r#"
use stdlib::geometry::analytic::{ray_capsule}
use stdlib::materials::bsdf::{emit_lobe}
space World
kind Hybrid @World (a: p3@World, b: p3@World) {
  surface(u,v) grid 8x8 = [u,v,0]@World
  visible(ro,rd) = ray_capsule(ro,rd,a,b,0.01)
  material(p,n,uv) = emit_lobe([1.0,1.0,1.0])
}
build {
  build for i in 0..8 { emit Hybrid(a: [0,0,0]@World, b: [0.05,0,0]@World) }
}
camera Main { at: [0,0,0]@World, radius: 3, fov: 40deg }
"#);
    let tmp = tempfile::tempdir().unwrap();
    let (p, _, _) = compile_cached(&src, &[], tmp.path()).unwrap_or_else(|e| panic!("{}", e.render(&src)));
    assert!(matches!(p.kinds[0].draw, loft_passes::program::Draw::Surface { .. }));
    assert_eq!(p.kinds[0].n_rows, 8);
    assert!(p.segments.is_empty());
}
