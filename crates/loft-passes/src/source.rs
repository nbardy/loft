//! Resolve authored modules before inference. Source IDs address diagnostics;
//! paths and ordered dependency edges are the input to the module cache.
use loft_ir::{Code, Diag, Res, Sources, Span};
use loft_parse::ast::{Import, Item, Module, ScheduleDecl};
use std::{
    collections::HashMap,
    fs, io,
    ops::Deref,
    path::{Path, PathBuf},
};

mod scope;

const CORE: &str = "stdlib/core.loft";
const REALTIME: &str = "stdlib/engines/realtime.loft";
const WOODBLOCK: &str = "stdlib/engines/woodblock.loft";

/// A source provider returns canonical identity and the bytes at the requested
/// path. An editor can overlay the bytes without changing the file's identity.
pub trait Provider {
    fn read(&self, path: &Path) -> io::Result<(PathBuf, String)>;
}

struct Disk;
impl Provider for Disk {
    fn read(&self, path: &Path) -> io::Result<(PathBuf, String)> {
        let canonical = path.canonicalize()?;
        let text = fs::read_to_string(&canonical)?;
        Ok((canonical, text))
    }
}

#[derive(Clone)]
struct Node {
    path: PathBuf,
    module: Module,
    imports: Vec<(Import, Vec<u16>)>,
}

pub struct Graph {
    sources: Sources,
    nodes: Vec<Node>,
    order: Vec<u16>,
    root: PathBuf,
    entry: u16,
    core: u16,
    recipe: Option<(u16, Span)>,
    next: u32,
    error: Option<Diag>,
}

impl Deref for Graph {
    type Target = Sources;
    fn deref(&self) -> &Sources {
        &self.sources
    }
}

impl Graph {
    /// Load a scene from its nearest workspace. The suffix is source owned by
    /// a client, such as an observation bake appended to the entry scene.
    pub fn file(path: &Path, suffix: &str) -> Result<Self, String> {
        let entry = path
            .canonicalize()
            .map_err(|e| format!("{}: {e}", path.display()))?;
        let root = entry
            .ancestors()
            .find(|p| p.join("Cargo.toml").is_file() && p.join("stdlib").is_dir())
            .ok_or_else(|| {
                format!(
                    "{}: no ancestor holds stdlib/ and Cargo.toml",
                    path.display()
                )
            })?;
        Ok(Self::load(root, &entry, &Disk, suffix))
    }

    /// Compile generated or edited scene text with the same dependency
    /// resolution as a file entry. The entry path anchors relative imports.
    pub fn text(root: &Path, entry: &Path, text: &str) -> Self {
        struct Overlay<'a> {
            entry: &'a Path,
            text: &'a str,
        }
        impl Provider for Overlay<'_> {
            fn read(&self, path: &Path) -> io::Result<(PathBuf, String)> {
                if path == self.entry {
                    return Ok((path.into(), self.text.into()));
                }
                Disk.read(path)
            }
        }
        let canonical = root.canonicalize();
        let root = canonical.as_deref().unwrap_or(root);
        let entry = root.join(entry);
        Self::load(
            root,
            &entry,
            &Overlay {
                entry: &entry,
                text,
            },
            "",
        )
    }

    pub fn load(root: &Path, entry: &Path, provider: &dyn Provider, suffix: &str) -> Self {
        let entry = root.join(entry);
        let mut loader = Loader {
            graph: Self {
                sources: Sources::default(),
                nodes: vec![],
                order: vec![],
                root: root.into(),
                entry: 0,
                core: 0,
                recipe: None,
                next: 0,
                error: None,
            },
            provider,
            interned: HashMap::new(),
            active: vec![],
        };
        if let Err(error) = loader.load(&entry, suffix) {
            loader.graph.error = Some(error);
        }
        loader.graph
    }

    pub fn entry(&self) -> u16 {
        self.entry
    }

    fn stock(&self, id: u16) -> Option<&str> {
        Some(&self.sources.files[usize::from(id)].0)
    }

    pub(crate) fn edited_module(&self, id: u16, next: &mut u32) -> Res<Module> {
        if let Some(error) = &self.error {
            return Err(error.clone());
        }
        let module = loft_parse::parse(id, &self.sources.files[usize::from(id)].1, next)?;
        scope::edited(self, id, module)
    }
}

struct Loader<'a> {
    graph: Graph,
    provider: &'a dyn Provider,
    interned: HashMap<PathBuf, u16>,
    active: Vec<PathBuf>,
}

fn schedule(module: &Module) -> Option<&ScheduleDecl> {
    module.items.iter().find_map(|item| match item {
        Item::Schedule(s) => Some(s),
        _ => None,
    })
}

fn complete(s: &ScheduleDecl) -> bool {
    s.passes.iter().any(|p| p.verb.name == "render_bundle")
}

impl Loader<'_> {
    fn load(&mut self, entry: &Path, suffix: &str) -> Res<()> {
        self.graph.core = self.visit(&self.graph.root.join(CORE), Span::NONE, "")?;
        // The entry's parse can fail before visit returns its ID. Its source
        // map slot still belongs to the entry and must survive that error.
        self.graph.entry = u16::try_from(self.graph.sources.files.len()).expect("source IDs");
        self.graph.entry = self.visit(entry, Span::NONE, suffix)?;
        let root = &self.graph.nodes[usize::from(self.graph.entry)];
        let authored = schedule(&root.module);
        if authored.is_some_and(complete) {
            return Ok(());
        }
        let mut candidates = Vec::new();
        for (import, ids) in &root.imports {
            for id in ids {
                let node = &self.graph.nodes[usize::from(*id)];
                let mut exported = node.module.items.iter().filter_map(|item| match item {
                    Item::Schedule(s) if node.module.public.contains(&s.name.span) => Some(s),
                    _ => None,
                });
                if let Some(first) = exported.next()
                    && let Some(second) = exported.next()
                {
                    return Err(Diag::new(
                        Code::E520,
                        second.name.span,
                        "multiple entry schedules in one module",
                        "export a single schedule",
                    )
                    .note(first.name.span, "first exported schedule"));
                }
                if schedule(&node.module).is_some_and(|s| node.module.public.contains(&s.name.span))
                    && !candidates.iter().any(|(seen, _)| seen == id)
                {
                    candidates.push((*id, import.span));
                }
            }
        }
        match candidates.as_slice() {
            [] => {
                let path = if authored
                    .is_some_and(|s| s.passes.iter().any(|p| p.verb.name == "woodblock"))
                {
                    WOODBLOCK
                } else {
                    REALTIME
                };
                let span = authored.map_or(Span::NONE, |s| s.name.span);
                let recipe = self.visit(&self.graph.root.join(path), span, "")?;
                self.graph.recipe = Some((recipe, span));
            }
            [candidate] => self.graph.recipe = Some(*candidate),
            [first, second, ..] => {
                return Err(Diag::new(
                    Code::E520,
                    second.1,
                    "multiple engine schedules are selected",
                    "import one engine or write a complete scene schedule",
                )
                .note(first.1, "other engine selected here"));
            }
        }
        Ok(())
    }

    fn visit(&mut self, path: &Path, span: Span, suffix: &str) -> Res<u16> {
        let (canonical, mut text) = self.provider.read(path).map_err(|e| {
            Diag::new(
                Code::E011,
                span,
                format!("cannot read module {}: {e}", path.display()),
                "import an existing .loft file",
            )
        })?;
        if let Some(start) = self.active.iter().position(|p| p == &canonical) {
            let chain = self.active[start..]
                .iter()
                .chain([&canonical])
                .map(|p| {
                    p.strip_prefix(&self.graph.root)
                        .unwrap_or(p)
                        .display()
                        .to_string()
                })
                .collect::<Vec<_>>()
                .join(" -> ");
            return Err(Diag::new(
                Code::E011,
                span,
                format!("module cycle: {chain}"),
                "remove one dependency",
            ));
        }
        if let Some(&id) = self.interned.get(&canonical) {
            return Ok(id);
        }
        if !suffix.is_empty() {
            text.push('\n');
            text.push_str(suffix);
        }
        let shown = canonical
            .strip_prefix(&self.graph.root)
            .or_else(|_| path.strip_prefix(&self.graph.root))
            .unwrap_or(&canonical);
        let id = self
            .graph
            .sources
            .add(shown.to_string_lossy().into_owned(), text);
        let module = loft_parse::parse(
            id,
            &self.graph.sources.files[usize::from(id)].1,
            &mut self.graph.next,
        )?;
        self.graph.nodes.push(Node {
            path: canonical.clone(),
            module,
            imports: vec![],
        });
        self.interned.insert(canonical.clone(), id);
        self.active.push(canonical.clone());
        let uses = self.graph.nodes[usize::from(id)]
            .module
            .items
            .iter()
            .filter_map(|item| match item {
                Item::Use(i) => Some(i.clone()),
                _ => None,
            })
            .collect::<Vec<_>>();
        let mut imports = Vec::with_capacity(uses.len());
        for import in uses {
            let parts = import
                .path
                .iter()
                .map(|n| n.name.as_str())
                .collect::<Vec<_>>();
            let targets = self.targets(&parts, &canonical, import.span)?;
            let mut ids = Vec::with_capacity(targets.len());
            for path in targets {
                ids.push(self.visit(&path, import.span, "")?);
            }
            imports.push((import, ids));
        }
        self.active.pop();
        self.graph.nodes[usize::from(id)].imports = imports;
        self.graph.order.push(id);
        Ok(id)
    }

    fn targets(&self, parts: &[&str], importing: &Path, span: Span) -> Res<Vec<PathBuf>> {
        let root = &self.graph.root;
        let under = |mut path: PathBuf, components: &[&str]| {
            path.extend(components);
            path.with_extension("loft")
        };
        let aliases: &[&str] = match parts {
            ["stdlib", first, rest @ ..] => {
                return Ok(vec![under(root.join("stdlib").join(first), rest)]);
            }
            ["self", first, rest @ ..] => {
                return Ok(vec![under(importing.with_file_name(first), rest)]);
            }
            ["engines", "woodblock"] => {
                return Ok(vec![
                    root.join(WOODBLOCK),
                    root.join("stdlib/styles/woodblock.loft"),
                    root.join("stdlib/post/display.loft"),
                ]);
            }
            [
                "engines",
                name @ ("realtime" | "reference" | "spectral_caustic"),
            ] => return Ok(vec![under(root.join("stdlib/engines").join(name), &[])]),
            ["std", "geom"] => &[
                "geometry/analytic",
                "geometry/mesh",
                "geometry/layout",
                "sdf/solid",
                "frames/transport",
            ],
            ["std", "light"] => &[
                "lighting/world",
                "lighting/emitters",
                "sky/environment",
                "integrators/direct",
                "materials/bsdf",
            ],
            ["std", "noise"] => &["noise/value"],
            ["std", "math" | "color" | "quant"] => &[],
            _ => {
                return Err(Diag::new(
                    Code::E011,
                    span,
                    format!("unsupported import {}", parts.join("::")),
                    "start a module path with stdlib:: or self::",
                ));
            }
        };
        Ok(aliases
            .iter()
            .map(|part| root.join(format!("stdlib/{part}.loft")))
            .collect())
    }
}

impl Graph {
    pub(crate) fn source_path(&self, id: u16) -> &Path {
        &self.nodes[usize::from(id)].path
    }

    pub(crate) fn parsed(&self) -> Res<(Vec<Module>, u32)> {
        if let Some(error) = &self.error {
            return Err(error.clone());
        }
        scope::link(self)
    }
}
