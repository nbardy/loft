use loft_check::{Prog, source::{Graph, Provider}};
use loft_ir::Code;
use std::{io, path::{Path, PathBuf}};

struct Library;

impl Provider for Library {
    fn read(&self, path: &Path) -> io::Result<(PathBuf, String)> {
        let text = match path.to_str() {
            Some("/workspace/scene.loft") => "schedule Main { render_bundle(Vis64) }",
            Some("/workspace/stdlib/core.loft") => "pub const TAU = 6.2831853",
            Some("/workspace/stdlib/render/left.loft") => {
                "fn hidden() -> f = TAU\npub fn sample() -> f = hidden()\npub fn gain() -> f = 2.0"
            }
            Some("/workspace/stdlib/render/right.loft") => "pub fn sample() -> f = TAU",
            _ => return Err(io::ErrorKind::NotFound.into()),
        };
        Ok((path.into(), text.into()))
    }
}

#[test]
fn stdlib_imports_discover_modules_and_link_duplicate_names_lexically() {
    let graph = Graph::load(
        Path::new("/workspace"),
        Path::new("scene.loft"),
        &Library,
        "use stdlib::render::left\nuse stdlib::render::right\nfn result() -> f = left::sample() + right::sample() + gain() + TAU",
    );
    let (modules, _) = graph.parsed().unwrap_or_else(|d| panic!("{}", d.render(&graph)));
    let prog = Prog::index(&modules).unwrap();
    assert_eq!(prog.fns.keys().filter(|name| name.ends_with("_sample")).count(), 2);
    assert!(prog.fns.contains_key("gain"));
    loft_check::check::check(&prog, &mut Vec::new()).unwrap_or_else(|d| panic!("{}", d.render(&graph)));
}

#[test]
fn stdlib_discovery_keeps_private_names_local() {
    for call in ["left::hidden()", "hidden()"] {
        let graph = Graph::load(
            Path::new("/workspace"),
            Path::new("scene.loft"),
            &Library,
            &format!("use stdlib::render::left\nfn result() -> f = {call}"),
        );
        assert_eq!(graph.validate_imports().unwrap_err().code, Code::E011);
    }
}
