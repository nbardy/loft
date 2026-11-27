use loft_eval::eval::{Eval, RowVal, Val};
use loft_eval::lip::Iv;
use loft_ir::Code;
use loft_passes::{
    Prog, front,
    source::{Graph, Provider},
};
use loft_parse::ast::{ExprKind, Item, Ty};
use std::{
    collections::BTreeSet,
    io,
    path::{Path, PathBuf},
};

struct Files<'a>(&'a [(&'a str, &'a str)]);

impl Provider for Files<'_> {
    fn read(&self, path: &Path) -> io::Result<(PathBuf, String)> {
        let path_name = path.strip_prefix("/workspace").unwrap().to_str().unwrap();
        let (_, text) = self.0.iter().find(|(name, _)| *name == path_name)
            .ok_or_else(|| io::Error::from(io::ErrorKind::NotFound))?;
        Ok((path.to_owned(), (*text).to_owned()))
    }
}

fn graph(scene: &str, files: &[(&str, &str)]) -> Graph {
    let mut all = vec![
        ("scene.loft", ""),
        ("stdlib/core.loft", ""),
        ("stdlib/engines/realtime.loft", DEFAULT_RECIPE),
    ];
    all.extend_from_slice(files);
    Graph::load(Path::new("/workspace"), Path::new("scene.loft"), &Files(&all), scene)
}

const DEFAULT_RECIPE: &str = r#"
pub schedule Realtime {
    pass cull writes [IndirectBuf] { dispatch("cull.wgsl", "main", 1) }
    render_bundle(Vis64) { draw_indirect(Drawables) }
}
"#;

const EXPRESSIONS: &str = r#"
use self::parts as p
const PICKED: f = 0.0 + (match 1 { 1 => 4, _ => 3 })
kind Piece @World (at: p3@World, offset: v3@World) {}

fn choose_signed(key: i) -> f = match key {
    -2i => 7,
    3 => PICKED,
    _ => 9,
}
fn choose_unsigned(key: u) -> i = match key { 0xffffffff => 7, _ => 9 }
fn choose_float(key: f) -> f = match key { 0 => 1, 2 => 5, _ => 8 }
fn lazy(key: i, zero: i) -> i = match key { 1 => 7, _ => 1 / zero }
fn point(raw: rgb) -> p3@p::Room = p3(raw)@p::Room

build fn pieces(show: bool) {
    if show {
        let base = p3([1, 2, 3])@World
        emit Piece(at: base + [1, 0, 0], offset: v3(1, 2, 3)@World)
    } else {
        emit Piece(at: p3(0)@World, offset: v3(rgb(4))@World)
    }
}
build fn maybe(show: bool) = if show { emit Piece(at: p3(0)@World, offset: v3(0)@World) }

build {
    pieces(true)
    pieces(false)
    maybe(false)
    if false {
        emit Piece(at: [9, 9, 9]@World, offset: v3(9)@World)
    }
    if true {
        emit Piece(at: [0, 1, 0]@World, offset: v3(0)@World)
    }
}
schedule Main { render_bundle(Vis64) }
"#;

const SPACES: &str = r#"pub space Room"#;

#[test]
fn constructors_and_build_conditions_share_the_existing_row_evaluator() {
    let src = graph(EXPRESSIONS, &[("parts.loft", SPACES)]);
    let (modules, types) = front(&src).unwrap_or_else(|e| panic!("{}", e.render(&src)));
    let p = Prog::index(&modules).unwrap();
    let mut eval = Eval::<f32>::new(&p, &types, vec![vec![]], &[]);
    let build = modules.iter().flat_map(|m| &m.items).find_map(|item| {
        if let Item::Build(f) = item { Some(f) } else { None }
    }).unwrap();
    eval.call_fn(build, vec![], build.name.span).unwrap();
    assert_eq!(eval.tables[0], [
        vec![RowVal::V(vec![2., 2., 3.]), RowVal::V(vec![1., 2., 3.])],
        vec![RowVal::V(vec![0., 0., 0.]), RowVal::V(vec![4., 4., 4.])],
        vec![RowVal::V(vec![0., 1., 0.]), RowVal::V(vec![0., 0., 0.])],
    ]);
}

#[test]
fn matches_are_lazy_and_only_point_dependent_forks_lose_the_lipschitz_bound() {
    let src = graph(EXPRESSIONS, &[("parts.loft", SPACES)]);
    let (modules, types) = front(&src).unwrap_or_else(|e| panic!("{}", e.render(&src)));
    let p = Prog::index(&modules).unwrap();
    let mut eval = Eval::<f32>::new(&p, &types, vec![], &[]);
    let mut call = |name: &str, args: Vec<Val<f32>>| {
        let f = p.fns[name];
        eval.call_fn(f, args, f.name.span).unwrap()
    };
    assert!(matches!(call("choose_signed", vec![Val::I(-2)]), Val::S(7.)));
    assert!(matches!(call("choose_signed", vec![Val::I(3)]), Val::S(4.)));
    assert!(matches!(call("choose_unsigned", vec![Val::U(u32::MAX)]), Val::I(7)));
    assert!(matches!(call("choose_float", vec![Val::S(-0.0)]), Val::S(1.)));
    assert!(matches!(call("lazy", vec![Val::I(1), Val::I(0)]), Val::I(7)));

    let f = p.fns["choose_float"];
    for (selector, expected) in [
        (Iv { lo: 1., hi: 1.5, l: 1. }, Iv { lo: 8., hi: 8., l: 0. }),
        (Iv { lo: 0., hi: 1., l: 0. }, Iv { lo: 1., hi: 8., l: 0. }),
        (Iv { lo: 0., hi: 1., l: 1. }, Iv { lo: 1., hi: 8., l: f64::INFINITY }),
    ] {
        let mut eval = Eval::<Iv>::new(&p, &types, vec![], &[]);
        let result = eval.call_fn(f, vec![Val::S(selector)], f.name.span).unwrap();
        assert!(matches!(result, Val::S(actual) if actual == expected));
    }

    let mut emitter = loft_passes::emit::Emitter::new(&p, &types);
    emitter.constants(&BTreeSet::from(["PICKED".into()]));
    for name in ["choose_signed", "choose_unsigned", "choose_float", "lazy", "point"] {
        emitter.func(p.fns[name], name);
    }
    assert_eq!(emitter.out.matches("switch ").count(), 5);
    let mut clean = String::new();
    let mut rest = emitter.out.as_str();
    while let Some((head, tail)) = rest.split_once('\u{fdd0}') {
        clean.push_str(head);
        let (_, after) = tail.split_once('\u{fdd1}').expect("provenance marker");
        rest = after;
    }
    clean.push_str(rest);
    let module = naga::front::wgsl::parse_str(&clean).unwrap_or_else(|e| {
        panic!("{}", e.emit_to_string(&clean))
    });
    naga::valid::Validator::new(naga::valid::ValidationFlags::all(), naga::valid::Capabilities::all())
        .validate(&module).unwrap();
}

const BAD_SPACE: &str = r#"
space Room
fn retag(x: v3@World) -> v3@Room = v3(x)@Room
schedule Main { render_bundle(Vis64) }
"#;
const BAD_ARMS: &str = r#"
fn wrong(x: i) -> f = match x { 1 => 1.0, _ => false }
schedule Main { render_bundle(Vis64) }
"#;
const BAD_PATTERN: &str = r#"
fn duplicate(x: i) -> i = match x { 1 => 1, 0x1 => 2, _ => 0 }
schedule Main { render_bundle(Vis64) }
"#;
const NO_FALLBACK: &str = r#"
fn incomplete(x: i) -> i = match x { 1 => 1 }
schedule Main { render_bundle(Vis64) }
"#;

const NONSMOOTH: &str = r#"
fn f(x: f) -> f = match x { 1 => 2.0 * x, _ => 3.0 * x }
fn df(x: f) -> f = diff(f, x)(x)
schedule Main { render_bundle(Vis64) }
"#;

#[test]
fn rejects_retagging_and_invalid_matches() {
    for (scene, code) in [(BAD_SPACE, Code::E104), (BAD_ARMS, Code::E003), (BAD_PATTERN, Code::E003), (NO_FALLBACK, Code::E001), (NONSMOOTH, Code::E307)] {
        let src = graph(scene, &[]);
        assert_eq!(front(&src).unwrap_err().code, code);
    }
}

const SCHEDULE_SCENE: &str = r#"
use self::parts as p
target Color: rgba16float
camera Main { at: [0, 0, 0], radius: 4 }
schedule local(output) { p::render(output) }
schedule Main { local(Color) }
"#;
const SCHEDULE_LIBRARY: &str = r#"
pub schedule render(output, groups = 2, ..extra) {
    pass cull reads [extra] writes [IndirectBuf] { dispatch("cull.wgsl", "main", groups) }
    pass paint reads [IndirectBuf] writes [output] { draw("paint.wgsl", "vs", "fs") }
    pass finish reads [output] writes [Swapchain] { draw("present.wgsl", "vs", "fs") }
}
"#;

#[test]
fn imported_schedule_is_a_single_graph_with_call_site_and_template_provenance() {
    let src = graph(SCHEDULE_SCENE, &[("parts.loft", SCHEDULE_LIBRARY)]);
    let (modules, _) = src.parsed().unwrap_or_else(|e| panic!("{}", e.render(&src)));
    let p = Prog::index(&modules).unwrap();
    let schedule = p.schedule.unwrap();
    assert!(!schedule.implicit);
    assert_eq!(schedule.passes.iter().map(|pass| pass.label.name.as_str()).collect::<Vec<_>>(),
        ["use_camera", "cull", "paint", "finish"]);
    let [_, cull, paint, finish] = schedule.passes.as_slice() else { unreachable!() };
    for pass in [cull, paint, finish] {
        assert_eq!(pass.scheduled_from().file, src.entry());
        assert_ne!(pass.verb.span.file, src.entry());
    }
    let cull = cull.resources.as_ref().unwrap();
    assert!(cull.reads.is_empty());
    let loft_parse::ast::Stmt::Expr(dispatch) = &cull.body.stmts[0] else { panic!("expected dispatch") };
    let ExprKind::Call { args, .. } = &dispatch.kind else { panic!("expected dispatch call") };
    let mut types: Vec<Ty> = vec![];
    assert_eq!(loft_eval::stage0(&p, &mut types, &args[2].value, &[]).unwrap(), RowVal::I(2));
    assert_eq!(paint.resources.as_ref().unwrap().writes[0].name, "Color");
}
