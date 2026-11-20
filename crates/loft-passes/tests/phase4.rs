use loft_ir::{Code, Span};
use loft_parse::ast::{Determinism, GeometryProof, UnprovedReason};
use loft_passes::{Prog, front, world::{LightSelection, WorldCall, WorldQuery}};
use std::path::Path;

fn sources(scene: &str) -> loft_passes::source::Graph {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let text = if scene.lines().any(|line| line.starts_with("use stdlib::")) {
        scene.to_owned()
    } else {
        format!("{scene}\nuse stdlib::lighting::world::*\nuse stdlib::materials::bsdf::*")
    };
    loft_passes::source::Graph::text(&root, Path::new("probe.loft"), &text)
}

#[test]
fn checked_contract_reaches_lowering() {
    let src = sources(include_str!("phase4/accepted.loft"));
    let (modules, _) = front(&src).unwrap_or_else(|d| panic!("{}", d.render(&src)));
    let p = Prog::index(&modules).unwrap();
    let k = p.kernels["scatter"];
    assert_eq!(k.writes.len(), 2);
    assert_eq!(k.writes["fixed"], Determinism::NonDet);
    assert_eq!(k.writes["floating"], Determinism::NonDet);
    for name in ["Opted", "Ragged"] {
        assert_eq!(p.kinds[name].proof, GeometryProof::Unproved(UnprovedReason::AuthorOptOut));
    }
    assert!(p.kinds["Ragged"].fns.iter().all(|f| f.name.name != "surface__jvp"));

}

#[test]
fn shared_function_still_requires_proof_at_an_unwrapped_call() {
    let kind = "space World\nfn raw(x: f) -> f = wgsl(x: f) -> f { return x; }\n\
        kind K @World () { trace(p) = unproved(0.5) { raw(p.x) }; material(p,n) = coat(1,1) }";
    let src = sources(kind);
    front(&src).unwrap_or_else(|d| panic!("{}", d.render(&src)));
    let src = sources(&kind.replace("{ raw(p.x) }", "{ raw(p.x) } + raw(p.x)"));
    assert_eq!(front(&src).unwrap_err().code, Code::E211);
}

#[test]
fn radiance_defaults_to_rgb() {
    let extinction = "fn extinction(p: p3@World) -> rgb = medium(p).t";
    let src = sources(&format!("fn radiance(x: Radiance) -> rgb = x\n{extinction}"));
    front(&src).unwrap_or_else(|d| panic!("{}", d.render(&src)));
    let src = sources("type Radiance = rgb\nschedule Main { type Radiance = v4 }\nfn energy(x: Radiance) -> v4 = x");
    front(&src).unwrap_or_else(|d| panic!("{}", d.render(&src)));
    // The built-in field sampler has RGB emission. A schedule with custom
    // Radiance must supply its own coefficient sampling.
    let src = sources(&format!("schedule Main {{ type Radiance = v4 }}\n{extinction}"));
    let err = front(&src).unwrap_err();
    assert_eq!(err.code, Code::E405, "{}", err.render(&src));
    assert_eq!(src.locate(err.span).3, extinction);
}

#[test]
fn world_operands_are_ordered_and_light_sampling_shares_a_hook() {
    use WorldQuery::*;
    for (name, args, world, query) in [
        ("trace", vec![5, 1], Some(5), Trace { ray: 1 }),
        ("visible", vec![1, 2], None, Visible { a: 1, b: 2 }),
        ("material", vec![1], None, Material { hit: 1 }),
        ("light", vec![5, 0, 1, 2], Some(5), Light { selection: LightSelection::Indexed(0), point: 1, rng: 2 }),
        ("pick_light", vec![1, 2], None, Light { selection: LightSelection::Pick, point: 1, rng: 2 }),
        ("sky", vec![1], None, Sky { direction: 1 }),
        ("medium", vec![1], None, Medium { point: 1 }),
    ] {
        assert_eq!(WorldCall::resolve(name, &args, Span::NONE).unwrap(), Some(WorldCall { world, query }));
    }
    assert_eq!(WorldCall::<u32>::resolve("light", &[], Span::NONE).unwrap_err().code, Code::E003);
}

#[test]
fn negative_fixtures_fail_at_the_author_site() {
    use Code::*;
    for (file, code) in [
        ("kernel-capability.invalid", E001), ("kernel-type.loft", E003),
        ("unproved-context.loft", E405), ("unproved-scale.loft", E003), ("unproved-scope.loft", E211),
        ("kernel-atomics.loft", E504), ("kernel-workgroup.loft", E504), ("kernel-subgroups.loft", E504),
        ("kernel-loops.loft", E504), ("kernel-atomic-destination.loft", E003), ("kernel-units.loft", E110),
        ("mix-types.loft", E003), ("radiance-type.loft", E003), ("world-space.loft", E104),
    ] {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/phase4").join(file);
        let src = sources(&std::fs::read_to_string(path).unwrap());
        let d = front(&src).expect_err(file);
        assert_eq!((d.code, d.span.file), (code, src.entry()), "{file}: {}", d.render(&src));
    }
}
