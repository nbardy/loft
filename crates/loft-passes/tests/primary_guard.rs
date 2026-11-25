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
