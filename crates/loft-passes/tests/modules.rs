use loft_ir::Code;
use loft_passes::{
    Prog,
    eval::{Eval, Val},
    front,
    source::{Graph, Provider},
};
use std::{
    io,
    path::{Path, PathBuf},
};

struct Files<'a>(&'a [(&'a str, &'a str)]);

impl Provider for Files<'_> {
    fn read(&self, path: &Path) -> io::Result<(PathBuf, String)> {
        let name = path.strip_prefix("/workspace").unwrap().to_str().unwrap();
        let (_, text) = self
            .0
            .iter()
            .find(|(key, _)| *key == name)
            .ok_or_else(|| io::Error::from(io::ErrorKind::NotFound))?;
        Ok((path.into(), (*text).into()))
    }
}

fn graph(entry: &str, files: &[(&str, &str)]) -> Graph {
    Graph::load(
        Path::new("/workspace"),
        Path::new("scene.loft"),
        &Files(files),
        entry,
    )
}

const CORE: &str = "pub const TAU = 6.2831853";
const ROOT: &str = "schedule Main { render_bundle(Vis64) }";
const LEFT: &str =
    "use self::leaf::{value}\nfn shade() -> f = value() + 1.0\npub fn sample() -> f = shade()";
const RIGHT: &str =
    "use self::leaf as base\nfn shade() -> f = base::value() + 2.0\npub fn sample() -> f = shade()";

#[test]
fn one_leaf_and_two_lexical_helpers() {
    let text = format!(
        "{ROOT}\nuse self::left as ink\nuse self::right as light\nfn result() -> f = ink::sample() + light::sample()"
    );
    let graph = graph(
        &text,
        &[
            ("scene.loft", ""),
            ("stdlib/core.loft", CORE),
            ("left.loft", LEFT),
            ("right.loft", RIGHT),
            ("leaf.loft", "pub fn value() -> f = 5.0"),
        ],
    );
    assert_eq!(graph.files.len(), 5);
    assert_eq!(
        graph
            .files
            .iter()
            .filter(|(file, _)| file == "leaf.loft")
            .count(),
        1
    );
    let (modules, types) = front(&graph).unwrap_or_else(|d| panic!("{}", d.render(&graph)));
    let p = Prog::index(&modules).unwrap();
    assert_eq!(
        p.fns
            .values()
            .filter(|f| f.name.name.ends_with("_shade"))
            .count(),
        2
    );
    let f = p.fns["result"];
    let mut eval = Eval::<f32>::new(&p, &types, vec![], &[]);
    assert!(matches!(
        eval.call_fn(f, vec![], f.name.span),
        Ok(Val::S(13.0))
    ));
}

#[test]
fn import_failures_retain_the_offending_source() {
    let fixtures = [
        ("scene.loft", ""),
        ("stdlib/core.loft", CORE),
        ("left.loft", LEFT),
        ("right.loft", RIGHT),
        ("leaf.loft", "pub fn value() -> f = 5.0"),
        ("cycle_a.loft", "use self::cycle_b"),
        ("cycle_b.loft", "use self::cycle_a"),
    ];
    for (import, message, file) in [
        (
            "use self::left::*\nuse self::right::*",
            "bound twice",
            "scene.loft",
        ),
        ("use self::left::{shade}", "private", "scene.loft"),
        (
            "use self::left\nfn bad() -> f = left::shade()",
            "private",
            "scene.loft",
        ),
        ("use self::left::{absent}", "unresolved name", "scene.loft"),
        ("use self::absent", "cannot read module", "scene.loft"),
        (
            "use self::cycle_a",
            "cycle_a.loft -> cycle_b.loft -> cycle_a.loft",
            "cycle_b.loft",
        ),
    ] {
        let graph = graph(&format!("{ROOT}\n{import}"), &fixtures);
        let error = front(&graph).unwrap_err();
        assert_eq!(error.code, Code::E011);
        assert!(error.msg.contains(message), "{}", error.render(&graph));
        assert_eq!(graph.locate(error.span).0, file);
        if message == "bound twice" {
            assert_eq!(error.notes.len(), 2);
        }
    }
}

#[test]
fn stage_typed_fragments_keep_derivatives_in_fragments() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let source = r#"
use stdlib::lighting::surface::{SurfaceHit, LightSummary}
use stdlib::integrators::direct as transport
use stdlib::schedules::forward
use stdlib::post::display
use self::buoys::{Buoy}

fn shade_style(hit: SurfaceHit, light: LightSummary) -> rgb =
  wgsl(hit: SurfaceHit, light: LightSummary) -> rgb {
    return vec3f(dpdx(hit.p.x));
  }

pub schedule Test {
  orbit(at: [0, 0, 0], radius: 4, fov: 35deg)
  forward::Forward(transport::estimate, shade_style)
  display::PrintPost()
}
build { emit Buoy(pose: place([0]@World), radius: 0.5m, pigment: [1, 0.5, 0]) }
"#;
    let path = Path::new("scenes/showcase/std1d/derivative_probe.loft");
    let graph = Graph::text(&root, path, source);
    let (program, _, _) = loft_passes::compile(&graph, &[])
        .unwrap_or_else(|d| panic!("{}", d.render(&graph)));
    assert!(program.passes.iter().any(|pass| matches!(pass, loft_passes::program::Pass::Forward(_))));
    let tiled = source.replace("::forward", "::vis_buffer").replace("forward::Forward", "vis_buffer::VisBuffer");
    let graph = Graph::text(&root, path, &tiled);
    let Err(diag) = loft_passes::compile(&graph, &[]) else { panic!("tile shader accepted a derivative") };
    assert_eq!(diag.code, Code::E701);
    assert!(diag.msg.contains("forbidden") || diag.msg.contains("stage"), "{}", diag.render(&graph));
}

#[test]
fn spectral_rgb_cannot_bind_a_light_summary_estimator() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let graph = Graph::text(&root, Path::new("scenes/showcase/std1d/contract_probe.loft"), r#"
use stdlib::engines::spectral_caustic::{spectral_sample}
use stdlib::integrators::sampling::{primary_jitter}
use stdlib::lighting::print::{shade_style}
use stdlib::schedules::path
use stdlib::post::display
schedule Test {
  orbit(at: [0, 0, 0], radius: 4, fov: 35deg)
  path::Path(spectral_sample, shade_style, primary_jitter)
  display::PrintPost()
}
"#);
    let Err(diag) = front(&graph) else { panic!("spectral RGB accepted as LightSummary") };
    assert_eq!(diag.code, Code::E003);
    assert!(diag.msg.contains("callback `estimate`"));
    assert_eq!(graph.locate(diag.span).0, "scenes/showcase/std1d/contract_probe.loft");
}

#[test]
fn physical_surface_hit_drives_path_and_cpu_material() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let scene = Path::new("scenes/showcase/std1d/area_control.loft");
    let source = std::fs::read_to_string(root.join(scene)).unwrap()
        .replace("lambert([1, 1, 1])", "lambert([hit.n.y * hit.n.y, 1, 1])");
    let graph = Graph::text(&root, scene, &source);
    let (program, _, compiled) = loft_passes::compile(&graph, &[])
        .unwrap_or_else(|diag| panic!("{}", diag.render(&graph)));
    let receiver = compiled.drawn.iter().position(|(_, name)| name == "Receiver").unwrap();
    let a = compiled.cpu_query(receiver, 0, [0.0; 3], [0.0, 1.0, 0.0]).unwrap().material;
    let b = compiled.cpu_query(receiver, 0, [0.0; 3], [1.0, 0.0, 0.0]).unwrap().material;
    assert!(a[0] > 0.0);
    assert_eq!(b[0], 0.0);
    assert!(program.passes.iter().any(|pass| matches!(pass, loft_passes::program::Pass::PathPrimary(_))));
}
