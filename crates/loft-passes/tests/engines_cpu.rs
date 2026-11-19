use loft_ir::{PassId, PassRun, Sources, TableId};
use loft_passes::{compile_aov, emit::Aov};
use std::{fs, path::Path};

fn sources(scene: &str, engine: Option<&str>) -> Sources {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut prelude: Vec<_> = fs::read_dir(root.join("lib")).unwrap().map(|x| x.unwrap().path())
        .filter(|p| p.extension().is_some_and(|x| x == "loft")).collect();
    prelude.sort();
    let mut src = Sources::default();
    for file in prelude.into_iter().chain([root.join("scenes").join(scene)]) {
        src.add(file.display().to_string(), fs::read_to_string(&file).unwrap());
    }
    if let Some(engine) = engine { src.add("selection", format!("use engines::{engine}")); }
    src
}

fn compile(src: &Sources, aov: Aov) -> (loft_passes::program::Program, loft_passes::Compiled) {
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
    let mut traced = sources("lipschitz_bound_mutant.loft", None);
    traced.files.last_mut().unwrap().1 = traced.files.last().unwrap().1.lines()
        .filter(|line| !line.trim_start().starts_with("visible(")).collect::<Vec<_>>().join("\n");
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
