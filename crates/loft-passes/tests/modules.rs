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
