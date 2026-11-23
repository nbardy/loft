//! The authoring guide plus the public surface of the installed source library.
use crate::budget::Failure;
use loft_ir::Sources;
use loft_parse::{
    ast::{Item, Module},
    cst::{Cst, Kind, Node, Token, Trivia},
};
use std::{fmt::Write as _, fs, path::{Path, PathBuf}};

const GUIDE: &str = include_str!("../../../docs/context.md");
const USAGE: &str = "usage: loft context [--section core|engines|motion|styles|passes|all]";

enum Section {
    All,
    Core,
    Engines,
    Motion,
    Styles,
    Passes,
}

impl Section {
    fn parse(args: &[String]) -> Result<Self, Failure> {
        Ok(match args {
            [] => Self::All,
            [flag, value] if flag == "--section" => match value.as_str() {
                "all" => Self::All,
                "core" => Self::Core,
                "engines" => Self::Engines,
                "motion" => Self::Motion,
                "styles" => Self::Styles,
                "passes" => Self::Passes,
                _ => return Err(Failure::Usage(USAGE.into())),
            },
            _ => return Err(Failure::Usage(USAGE.into())),
        })
    }

    fn includes(&self, path: &Path, item: &Item) -> bool {
        let group = |dir: &str| path.starts_with(dir);
        match self {
            Self::All => true,
            Self::Core => path == Path::new("core.loft"),
            Self::Engines => group("engines") || group("schedules") || group("integrators"),
            Self::Motion => group("motion") || path == Path::new("engines/act.loft")
                || path == Path::new("engines/roam.loft"),
            Self::Styles => group("styles") || group("materials") || group("post"),
            Self::Passes => matches!(item, Item::Schedule(_) | Item::Kernel(_)),
        }
    }
}

pub(crate) fn run(args: &[String]) -> Result<(), Failure> {
    let section = Section::parse(args)?;
    let cwd = std::env::current_dir().map_err(|e| Failure::Usage(e.to_string()))?;
    let root = loft_passes::source::workspace_root(&cwd)
        .ok_or_else(|| Failure::Usage("cannot find the Loft workspace and its stdlib/".into()))?;
    let mut out = String::new();
    if matches!(section, Section::All | Section::Core) {
        writeln!(out, "{}\n", GUIDE.trim_end()).unwrap();
        core(&mut out);
    }
    if matches!(section, Section::All | Section::Passes) {
        writeln!(out, "Raster passes: {}\n", loft_passes::GPU_ORDER.join(" ")).unwrap();
    }
    out.push_str("## Public signatures\n\n");
    catalog(&mut out, &root.join("stdlib"), &section)?;
    print!("{out}");
    Ok(())
}

fn core(out: &mut String) {
    use loft_passes::check::{MAP_OPS, builtin_names, builtin_types};
    let names = builtin_names().filter(|name| !name.starts_with("loft_")).collect::<Vec<_>>();
    let aliases = loft_parse::TYPE_ALIASES.iter().map(|&(short, _)| short);
    let types = builtin_types().chain(aliases).collect::<Vec<_>>();
    out.push_str("## Compiler vocabulary\n\n");
    // Builtins are listed as bare names; say they need no import. Without this, a model filed `mix_bsdf`
    // under the catalog heading it belongs to (`use stdlib::materials::bsdf::{mix_bsdf}`), an E011.
    writeln!(out, "Builtins (always in scope, never imported): {}", names.join(" ")).unwrap();
    writeln!(out, "Types: {}", types.join(" ")).unwrap();
    writeln!(out, "Units: {}", loft_parse::UNIT_SYMBOLS.join(" ")).unwrap();
    writeln!(out, "Map stages: {}\n", MAP_OPS.join(" ")).unwrap();
}

fn catalog(out: &mut String, library: &Path, section: &Section) -> Result<(), Failure> {
    let mut paths = Vec::new();
    modules(library, &mut paths).map_err(|e| Failure::Usage(format!("{}: {e}", library.display())))?;
    paths.sort();
    for path in paths {
        let relative = path.strip_prefix(library).expect("walked under stdlib");
        let name = relative.with_extension("").components()
            .map(|part| part.as_os_str().to_string_lossy().into_owned())
            .collect::<Vec<_>>().join("::");
        let source = fs::read_to_string(&path)
            .map_err(|e| Failure::Usage(format!("{}: {e}", path.display())))?;
        let mut sources = Sources::default();
        let file = sources.add(format!("stdlib::{name}"), source);
        let source = &sources.files[usize::from(file)].1;
        let report = |d: loft_ir::Diag| Failure::Compile(d.render(&sources));
        let ast = loft_parse::parse(file, source, &mut 0).map_err(report)?;
        let items = ast.items.iter()
            .filter(|item| section.includes(relative, item) && public(&ast, item))
            .collect::<Vec<_>>();
        if items.is_empty() {
            continue;
        }
        let cst = Cst::parse(file, source).map_err(report)?;
        let tokens = cst.tokens();
        writeln!(out, "stdlib::{name}").unwrap();
        for item in items {
            signature(out, item, source, &cst.roots, &tokens);
            out.push('\n');
        }
        out.push('\n');
    }
    Ok(())
}

fn modules(dir: &Path, paths: &mut Vec<PathBuf>) -> std::io::Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let kind = entry.file_type()?;
        let path = entry.path();
        if kind.is_dir() {
            modules(&path, paths)?;
        } else if kind.is_file() && path.extension().is_some_and(|ext| ext == "loft") {
            paths.push(path);
        }
    }
    Ok(())
}

fn public(ast: &Module, item: &Item) -> bool {
    item.declarations().first().is_some_and(|name| ast.public.contains(&name.span))
}

fn signature(out: &mut String, item: &Item, source: &str, roots: &[Node], tokens: &[&Token]) {
    // The AST selects exports. CST spans preserve authored spelling, including
    // unit expressions and generic spaces that the AST represents canonically.
    let names = item.declarations();
    let name = roots.iter().position(|node| first(node).span == names[0].span)
        .expect("public declaration has a name token");
    let start = roots[..name].iter().rposition(|node| first(node).text(source) == "pub")
        .expect("public declaration has a pub token");
    if let Some(contract) = first(&roots[start]).leading.iter().rev().find_map(|(kind, span)| {
        if *kind != Trivia::Comment {
            return None;
        }
        source[span.lo as usize..span.hi as usize].strip_prefix("# context ")
            .filter(|text| (1..=12).contains(&text.split_whitespace().count()))
    }) {
        writeln!(out, "# {contract}").unwrap();
    }

    if let Item::Schedule(schedule) = item {
        // Print each pass as called, with its arguments. Bare verbs hid required arguments: models wrote
        // `find_shadow_map` without `size:` (E520) because the catalog showed only `find_shadow_map`.
        let Node::Group { children, .. } = &roots[body_index(roots, name, source)] else {
            unreachable!("schedule body is a group");
        };
        write!(out, "pub schedule {} {{", schedule.name.name).unwrap();
        let mut statements: Vec<(u32, u32)> = Vec::new();
        for child in children {
            let (lo, hi) = (first(child).span.lo, last(child).span.hi);
            match statements.last_mut() {
                Some(open) if !first(child).leading.iter().any(|(kind, _)| *kind == Trivia::Newline) => open.1 = hi,
                _ => statements.push((lo, hi)),
            }
        }
        for (i, &(lo, hi)) in statements.iter().enumerate() {
            if i > 0 {
                out.push_str("; ");
            }
            compact(out, source, tokens, lo, hi);
        }
        out.push('}');
        return;
    }

    if let Item::Fn(f) = item
        && f.attrs.iter().any(|a| a.name.name == "lobe")
        && let Some(light) = f.params.len().checked_sub(3).map(|i| &f.params[i])
    {
        // Print a lobe as a material calls it. The renderer appends the last three parameters (n, v, l);
        // printing the declared arity taught models to pass them (crab64: 9 of 20 Generate programs).
        compact(out, source, tokens, first(&roots[start]).span.lo, light.name.span.lo);
        if out.ends_with(',') {
            out.pop();
        }
        out.push(')');
        return;
    }

    let end = match item {
        Item::Space(_) => names.last().unwrap().span.hi,
        Item::Const(c) => c.range.as_ref()
            .map_or(&c.value, |range| range.hi.as_ref().unwrap_or(&range.lo)).span.hi,
        Item::TypeAlias(..) => type_end(roots, name + 2, source),
        Item::Struct(..) | Item::World(..) => {
            let body = body_index(roots, name, source);
            last(&roots[body]).span.hi
        }
        Item::Fn(_) | Item::Kind(_) | Item::Scan(_) | Item::Law(_)
        | Item::Kernel(_) | Item::Check(_)
        | Item::Camera(_) | Item::Bake(_) => {
            let body = body_index(roots, name, source);
            last(&roots[body - 1]).span.hi
        }
        Item::Unit(_) => names[0].span.hi,
        Item::Schedule(_) | Item::Use(_) | Item::Build(_) => unreachable!("handled or unnamed"),
    };
    compact(out, source, tokens, first(&roots[start]).span.lo, end);
}

fn type_end(nodes: &[Node], first_type: usize, source: &str) -> u32 {
    let mut end = first_type;
    while let Some(node) = nodes.get(end + 1) {
        let token = first(node);
        if token.text(source).is_empty() || token.text(source) == ";"
            || token.leading.iter().any(|(kind, _)| *kind == Trivia::Newline)
        {
            break;
        }
        end += 1;
    }
    last(&nodes[end]).span.hi
}

fn compact(out: &mut String, source: &str, tokens: &[&Token], start: u32, end: u32) {
    let from = tokens.partition_point(|t| t.span.lo < start);
    let to = tokens.partition_point(|t| t.span.lo < end);
    let mut previous = None::<&Token>;
    for &token in &tokens[from..to] {
        if let Some(prev) = previous
            && matches!(token.kind, Kind::Word | Kind::Number | Kind::Grid | Kind::String)
            && (matches!(prev.kind, Kind::Word | Kind::Number | Kind::Grid | Kind::String)
                || matches!(prev.text(source), ")" | "]" | "}"))
        {
            out.push(' ');
        }
        out.push_str(token.text(source));
        previous = Some(token);
    }
}

fn body_index(roots: &[Node], name: usize, source: &str) -> usize {
    (name + 1..roots.len()).find(|&i| {
        match first(&roots[i]).text(source) {
            "=" => true,
            "{" => first(&roots[i - 1]).text(source) != ".", // law writes h.{...}
            _ => false,
        }
    }).expect("declaration has a body")
}

fn first(node: &Node) -> &Token {
    match node {
        Node::Leaf(token) | Node::Group { open: token, .. } => token,
    }
}

fn last(node: &Node) -> &Token {
    match node {
        Node::Leaf(token) | Node::Group { close: token, .. } => token,
    }
}
