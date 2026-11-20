use loft_ir::{PassId, PassRun, TableId};
use loft_passes::source::Graph;
use loft_passes::{compile_aov, emit::Aov};
use std::path::Path;

fn sources(scene: &str, engine: Option<&str>) -> Graph {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let suffix = engine.map_or_else(String::new, |name| format!("use stdlib::engines::{name}"));
    Graph::file(&root.join("scenes").join(scene), &suffix).unwrap()
}

fn compile(src: &Graph, aov: Aov) -> (loft_passes::program::Program, loft_passes::Compiled) {
    let (program, _, compiled) = compile_aov(src, &[], aov).unwrap_or_else(|d| panic!("{}", d.render(src)));
    for module in &program.modules {
        naga::valid::Validator::new(naga::valid::ValidationFlags::all(), naga::valid::Capabilities::default())
            .validate(&module.naga).unwrap_or_else(|e| panic!("{}: {}", module.name, e.emit_to_string(&module.wgsl)));
    }
    (program, compiled)
}

#[test]
fn all_aovs_and_engines_validate() {
    let source = sources("lipschitz_bound_mutant.loft", None);
    for aov in [Aov::Beauty, Aov::Albedo, Aov::Normal, Aov::Depth, Aov::Id, Aov::SdfSteps, Aov::LipschitzRatio, Aov::Overdraw] {
        let (program, _) = compile(&source, aov);
        assert_eq!(program.kinds.len(), 1);
    }
    for name in ["reference", "woodblock", "spectral_caustic"] {
        let (program, _) = compile(&sources("lipschitz_bound_mutant.loft", Some(name)), Aov::Beauty);
        let shader = &program.modules[program.present].naga;
        assert!(shader.entry_points.iter().any(|e| e.name == "fs_present"));
        assert_eq!(shader.entry_points.iter().any(|e| e.name == "photons"), name == "spectral_caustic");
    }
    // Exercise the packing path on the marched form as well as the analytic fixture.
    let original = sources("lipschitz_bound_mutant.loft", None);
    let text = original.files[usize::from(original.entry())].1.lines()
        .filter(|line| !line.trim_start().starts_with("visible(")).collect::<Vec<_>>().join("\n");
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let traced = Graph::text(&root, Path::new("scenes/lipschitz_bound_mutant.loft"), &text);
    let (program, _) = compile(&traced, Aov::SdfSteps);
    assert!(program.modules[0].wgsl.contains("0xffffff00u"));
}

#[test]
fn bad_bound_fixture_has_numeric_violation() {
    let source = sources("lipschitz_bound_mutant.loft", None);
    let (base, _) = compile(&source, Aov::LipschitzRatio);
    let (changed, _, twin) = compile_aov(&source, &[("mutant_scale".into(), 2.0)], Aov::LipschitzRatio)
        .unwrap_or_else(|d| panic!("{}", d.render(&source)));
    let s = 0.0005;
    let sample = |p| twin.cpu_query(0, 0, [0.0, p, 0.0], [0., 1., 0.]).unwrap().trace;
    let dt = (sample(0.8+s) - sample(0.8-s))/(2.0*s);
    let ratio = dt / base.kinds[0].lipschitz[0];
    assert!(ratio > 1.0 && (dt - 2.0).abs() < 1e-3, "gradient={dt} bound={} ratio={ratio}", base.kinds[0].lipschitz[0]);
    assert!(changed.kinds[0].lipschitz[0] > base.kinds[0].lipschitz[0]);
}

#[test]
fn bake_declarations_lower_to_dispatch_then_mips() {
    use loft_passes::bake_tex::{Clock, TexDim};
    let (p, _) = compile(&sources("bake_load_fixture.loft", None), Aov::Beauty);
    assert_eq!(p.bakes.len(), 3);
    assert_eq!(p.bakes[0].dim, TexDim::D2(2048, 2048));
    assert_eq!(p.bakes[1].dim, TexDim::Cube(128));
    assert_eq!(p.bakes[2].dim, TexDim::D3(16, 8, 4));
    assert_eq!(p.bakes.iter().take(2).map(|t| t.mip_count()).sum::<u32>(), 20);
    assert!(matches!(p.bakes[0].clock, Clock::Load));
    assert!(matches!(p.bakes[1].clock, Clock::Load));
    assert!(matches!(p.bakes[2].clock, Clock::Dirty(ref params) if params == &[12]));
    for (id, bake) in p.bakes.iter().enumerate() {
        let table = TableId(id as u32);
        let plan = bake.passes(table, PassId(id as u32 * 2));
        assert!(matches!(plan[1].run, PassRun::GenerateMips { texture } if texture == table));
        assert!(plan[0].wave < plan[1].wave);
    }
}

#[test]
fn spectral_study_lowers_the_blend_and_nested_boundaries() {
    use loft_passes::emit::paths::Integrator;
    let (p, twin) = compile(&sources("spectral_caustic_study.loft", None), Aov::Beauty);
    let path = p.path.as_ref().unwrap();
    assert!(matches!(path.integrator, Integrator::Spectral));
    assert_eq!(path.lens, [0.014, 3.49]);
    let glass = p.kinds.iter().position(|k| k.name == "StudyOptic").unwrap();
    let bead = p.kinds.iter().position(|k| k.name == "StudyBead").unwrap();
    let trace = |kind, row, point| twin.cpu_query(kind, row, point, [0.0, 1.0, 0.0]).unwrap().trace;
    let bubble = [0.18, 0.56, 0.09];
    assert!(trace(glass, 0, bubble) < -0.18);
    assert!(trace(glass, 1, bubble) < -0.13);
    assert!((trace(bead, 0, bubble) + 0.052).abs() < 1e-6);
    assert_eq!(p.bakes[0].name, "StudyInk");
    assert!(p.modules[p.present].wgsl.contains("mix(m0.c,m1.c,clamp(b.mix0,0.0,1.0))"));
    assert!(p.modules[p.present].wgsl.contains("atomicAdd(&loft_local_bins"));
}
