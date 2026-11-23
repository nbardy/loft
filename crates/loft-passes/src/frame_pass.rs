//! Frame passes a schedule writes: a WGSL template whose holes the schedule fills, bound to the frame's stores by name.
//!
//!   pass cull reads [CameraSlot, kinds] writes [IndirectBuf] {
//!     dispatch("core/cull.wgsl", "cull", Instances, draw: if drawn == 1 { "core/cull_single.wgsl" } else { "..." })
//!   }
//!
//! A named argument fills the template hole of the same name in capitals. Holes are evaluated after the frame's
//! compile-time facts are known (`drawn`, `edges`, `aov`, `paper`), which is when the compiler filled them itself.
use crate::{Prog, authored_compute::compute_extent, emit};
use crate::program::{AuthoredPass, AuthoredRun, Extent, FrameView, ShaderModule, ViewStore, abi};
use crate::source::Graph;
use emit::{Aov, shade_tiles::Visibility};
use loft_ir::{Code, Diag, Res};
use loft_parse::ast::*;
use std::path::{Path, PathBuf};

/// The stores a render frame owns, named as schedules name them.
const FRAME_STORES: [&str; 13] = ["FrameUniform", "CameraSlot", "IndirectBuf", "Vis64", "DepthBuf", "ShadowMap", "TileLists",
    "ColorHdr", "Swapchain", "RowTables", "LutTables", "PoseBuf", "VerdictBuf"];
const EDGES: [&str; 3] = ["none", "taps", "coverage"];
const AOVS: [&str; 8] = ["beauty", "albedo", "normal", "depth", "id", "sdf_steps", "lipschitz_ratio", "overdraw"];
/// Fragments may name holes of their own; this bounds a fragment that names itself.
const MAX_NESTING: u32 = 8;

pub(crate) struct FramePass<'a> {
    pub(crate) decl: &'a PassDecl,
    template: PathBuf,
    form: Form,
    /// Hole name in capitals, and its value.
    holes: Vec<(String, &'a Expr)>,
    pub(crate) reads: Vec<&'a str>,
    pub(crate) writes: Vec<&'a str>,
    /// The kinds the pass reads. A pass that reads kinds binds the instance table, which holds every drawn kind.
    pub(crate) kinds: Vec<&'a str>,
    /// Whether `reads` names kinds at all: `Drawables` in a scene that emits no drawable rows lists none.
    pub(crate) reads_kinds: bool,
}

enum Form {
    Compute { entry: String, extent: ExtentSpec },
    FullScreen { vertex: String, fragment: String },
}

enum ExtentSpec {
    Groups([u32; 3]),
    Instances,
    Pixels,
}

/// What the frame knows when its passes are lowered.
pub(crate) struct Facts<'p> {
    pub(crate) drawn: u32,
    pub(crate) edges: Visibility,
    pub(crate) aov: Aov,
    pub(crate) paper: Option<&'p str>,
}

enum Fact<'p> {
    Drawn(u32),
    Edges(&'static str),
    Aov(&'static str),
    Paper(Option<&'p str>),
}

enum Fragment<'p> {
    Text(String),
    Function(&'p str),
}

fn literal(e: &Expr) -> Option<Res<String>> {
    let ExprKind::String(text) = &e.kind else { return None };
    Some(serde_json::from_str::<String>(text).map_err(|err| Diag::new(Code::E003, e.span, format!("invalid string literal: {err}"), "write a quoted string")))
}

fn read(path: &Path, at: &Expr) -> Res<String> {
    std::fs::read_to_string(path).map_err(|err| Diag::new(Code::E011, at.span, format!("{}: {err}", path.display()), "name a WGSL file next to the schedule"))
}

impl<'a> FramePass<'a> {
    /// `None`: the pass names no frame store, so it is an authored compute pass over its own tables.
    pub(crate) fn classify(
        p: &Prog<'a>, types: &mut Vec<Ty>, src: &Graph, overrides: &[(String, f64)], decl: &'a PassDecl, resources: &'a PassResources,
    ) -> Res<Option<Self>> {
        let implicit = p.schedule.is_some_and(|s| s.implicit);
        let kinds_named = |n: &str| -> Option<Vec<&'a str>> {
            if implicit && matches!(n, "Drawables" | "Casters") {
                return Some(p.kind_order.iter().copied().filter(|k| {
                    let kind = p.kinds[k];
                    (!kind.queries.is_empty() || *k == "Mesh") && (n == "Drawables" || crate::pose_field(kind).is_some())
                }).collect());
            }
            p.kinds.get_key_value(n).map(|(&k, _)| vec![k])
        };
        let names = resources.reads.iter().chain(&resources.writes).collect::<Vec<&Ident>>();
        let frame = |n: &Ident| FRAME_STORES.contains(&n.name.as_str()) || kinds_named(n.name.as_str()).is_some();
        if !names.iter().any(|&n| frame(n)) {
            return Ok(None);
        }
        if let Some(n) = names.iter().find(|&&n| !frame(n)) {
            return Err(Diag::new(Code::E520, n.span, format!("`{}` is not a frame store", n.name),
                format!("a pass in a render frame names frame stores ({}) and kinds", FRAME_STORES.join(", "))));
        }
        let err = |message: String, fix: &str| Diag::new(Code::E520, decl.label.span, message, fix.to_owned());
        let mut reads = vec![];
        let mut kinds = vec![];
        let mut reads_kinds = false;
        for n in &resources.reads {
            match kinds_named(n.name.as_str()) {
                Some(found) => { kinds.extend(found); reads_kinds = true; }
                None => reads.push(n.name.as_str()),
            }
        }
        let writes = resources.writes.iter().map(|n| n.name.as_str()).collect::<Vec<_>>();
        if writes.iter().any(|&w| kinds_named(w).is_some()) {
            return Err(err("a frame pass cannot write a kind's rows".into(), "write rows with a law"));
        }
        let [Stmt::Expr(Expr { kind: ExprKind::Call { callee, args }, .. })] = resources.body.stmts.as_slice() else {
            return Err(err("a frame pass body is one dispatch(...) or draw(...) call".into(),
                "write dispatch(\"t.wgsl\", \"entry\", Pixels, hole: value) or draw(\"t.wgsl\", \"vs\", \"fs\", hole: value)"));
        };
        let positional = args.iter().filter(|a| a.name.is_none()).map(|a| &a.value).collect::<Vec<_>>();
        let text = |e: &Expr| literal(e).unwrap_or_else(|| Err(Diag::new(Code::E003, e.span, "expected a quoted string", "quote the file or entry name")));
        let (template, form) = match (callee.name.as_str(), positional.as_slice()) {
            ("dispatch", [file, entry, extent]) => {
                let word = if let ExprKind::Name(n) = &extent.kind { n.as_str() } else { "" };
                let extent = match word {
                    "Instances" => ExtentSpec::Instances,
                    "Pixels" => ExtentSpec::Pixels,
                    _ => ExtentSpec::Groups(compute_extent(p, types, overrides, extent)?),
                };
                (*file, Form::Compute { entry: text(*entry)?, extent })
            }
            ("draw", [file, vertex, fragment]) => {
                if !writes.contains(&"Swapchain") {
                    return Err(err("a full-screen draw writes the swapchain".into(), "add `writes [Swapchain]`"));
                }
                (*file, Form::FullScreen { vertex: text(*vertex)?, fragment: text(*fragment)? })
            }
            _ => return Err(err(format!("`{}` with {} positional arguments in a frame pass", callee.name, positional.len()),
                "write dispatch(template, entry, extent, ...) or draw(template, vertex, fragment, ...)")),
        };
        let path = src.source_path(template.span.file).with_file_name(text(template)?);
        let mut holes: Vec<(String, &'a Expr)> = vec![];
        for arg in args {
            let Some(name) = &arg.name else { continue };
            let key = name.name.to_ascii_uppercase();
            if holes.iter().any(|(k, _)| *k == key) {
                return Err(Diag::new(Code::E520, name.span, format!("hole `{}` is filled twice", name.name), "fill each hole once"));
            }
            holes.push((key, &arg.value));
        }
        Ok(Some(Self { decl, template: path, form, holes, reads, writes, kinds, reads_kinds }))
    }

    /// The function this pass's `sky` hole names, when it presents.
    pub(crate) fn sky(&self, p: &Prog<'a>) -> Option<&'a str> {
        if !matches!(self.form, Form::FullScreen { .. }) { return None; }
        let (_, e) = self.holes.iter().find(|(key, _)| key == "SKY")?;
        let ExprKind::Name(n) = &e.kind else { return None };
        p.fns.get_key_value(n.as_str()).map(|(&k, _)| k)
    }

    fn error(&self, message: String, fix: &str) -> Diag {
        Diag::new(Code::E520, self.decl.label.span, message, fix.to_owned())
    }

    /// The module text: the frame prelude, the Loft library the function holes reach, then the filled template.
    pub(crate) fn module<'p>(&self, p: &Prog<'p>, types: &[Ty], src: &Graph, facts: &Facts<'p>) -> Res<String> {
        let mut roots: Vec<&str> = vec![];
        let mut values = vec![];
        for (key, e) in &self.holes {
            let text = match self.value(p, src, e, facts)? {
                Fragment::Text(text) => text,
                Fragment::Function(f) => {
                    if !roots.contains(&f) { roots.push(f); }
                    emit::fn_symbol(f)
                }
            };
            values.push((key.as_str(), text));
        }
        let template = std::fs::read_to_string(&self.template).map_err(|err| self.error(format!("{}: {err}", self.template.display()),
            "name a WGSL template next to the schedule"))?;
        let body = self.fill(&template, &values, 0)?;
        Ok(emit::template_module(p, types, &roots, &body))
    }

    fn fill(&self, text: &str, values: &[(&str, String)], depth: u32) -> Res<String> {
        let mut out = String::with_capacity(text.len());
        let mut rest = text;
        while let Some((before, marker)) = rest.split_once("{{") {
            let Some((key, after)) = marker.split_once("}}") else {
                return Err(self.error("an unclosed `{{` in the template".into(), "close the hole with `}}`"));
            };
            let (_, value) = values.iter().find(|(k, _)| *k == key).ok_or_else(|| self.error(
                format!("the template has hole {{{{{key}}}}}, which `{}` does not fill", self.decl.label.name),
                &format!("add `{}: ...` to the call", key.to_ascii_lowercase())))?;
            if depth == MAX_NESTING {
                return Err(self.error(format!("hole {{{{{key}}}}} nests more than {MAX_NESTING} deep"), "a fragment must not name its own hole"));
            }
            out.push_str(before);
            out.push_str(&self.fill(value, values, depth + 1)?);
            rest = after;
        }
        out.push_str(rest);
        Ok(out)
    }

    fn value<'p>(&self, p: &Prog<'p>, src: &Graph, e: &Expr, facts: &Facts<'p>) -> Res<Fragment<'p>> {
        let bad = |message: String| Diag::new(Code::E520, e.span, message,
            "fill a hole with WGSL text, a `.wgsl` fragment, a number, a library fn, or `if` over drawn, edges, aov and paper");
        match &e.kind {
            ExprKind::String(_) => {
                let text = literal(e).expect("string")?;
                if text.ends_with(".wgsl") {
                    return Ok(Fragment::Text(read(&src.source_path(e.span.file).with_file_name(&text), e)?));
                }
                Ok(Fragment::Text(text))
            }
            ExprKind::Num { value, .. } if value.fract() == 0.0 => Ok(Fragment::Text(format!("{}", *value as i64))),
            ExprKind::Num { value, .. } => Ok(Fragment::Text(format!("{value:?}"))),
            ExprKind::Name(n) => match fact(n, facts) {
                Some(Fact::Paper(Some(f))) => Ok(Fragment::Function(f)),
                Some(Fact::Paper(None)) => Err(bad("`paper` is unset: the schedule has no outline_and_paper".into())),
                Some(Fact::Drawn(_) | Fact::Edges(_) | Fact::Aov(_)) => Err(bad(format!("`{n}` is a frame fact; compare it in an `if`"))),
                None => p.fns.get_key_value(n.as_str()).map(|(&k, _)| Fragment::Function(k)).ok_or_else(|| bad(format!("`{n}` is not a library fn"))),
            },
            ExprKind::If { condition, then, otherwise } => {
                let arm = if self.truth(condition, facts)? { then } else { otherwise };
                if !arm.body.stmts.is_empty() {
                    return Err(bad("a hole's `if` arm is one value".into()));
                }
                self.value(p, src, &arm.value, facts)
            }
            ExprKind::Unproved { .. } | ExprKind::FiniteDiff { .. } | ExprKind::Bool(_) | ExprKind::Member(..) | ExprKind::Index(..)
            | ExprKind::Call { .. } | ExprKind::Ctor { .. } | ExprKind::Binary(..) | ExprKind::Unary(..) => Err(bad("this is not a hole value".into())),
        }
    }

    fn truth(&self, e: &Expr, facts: &Facts<'_>) -> Res<bool> {
        let bad = || Diag::new(Code::E520, e.span, "a hole condition compares a frame fact",
            "write `drawn == 1`, `edges == taps`, `aov != beauty` or `paper`, joined by && and ||");
        match &e.kind {
            ExprKind::Binary(BinOp::And, a, b) => Ok(self.truth(a, facts)? & self.truth(b, facts)?),
            ExprKind::Binary(BinOp::Or, a, b) => Ok(self.truth(a, facts)? | self.truth(b, facts)?),
            ExprKind::Binary(op @ (BinOp::Eq | BinOp::Ne), a, b) => Ok(equal(a, b, facts)? == (*op == BinOp::Eq)),
            ExprKind::Unary(UnOp::Not, a) => Ok(!self.truth(a, facts)?),
            ExprKind::Bool(b) => Ok(*b),
            ExprKind::Name(n) => match fact(n, facts) {
                Some(Fact::Paper(f)) => Ok(f.is_some()),
                Some(Fact::Drawn(_) | Fact::Edges(_) | Fact::Aov(_)) | None => Err(bad()),
            },
            ExprKind::Unproved { .. } | ExprKind::FiniteDiff { .. } | ExprKind::If { .. } | ExprKind::Num { .. } | ExprKind::String(_)
            | ExprKind::Member(..) | ExprKind::Index(..) | ExprKind::Call { .. } | ExprKind::Ctor { .. } | ExprKind::Binary(..)
            | ExprKind::Unary(..) => Err(bad()),
        }
    }

    /// The lowered pass: bindings by global name onto frame views, each view's store listed by the pass.
    pub(crate) fn lowered(&self, module: &ShaderModule, index: usize) -> Res<AuthoredPass> {
        let label = &self.decl.label.name;
        let ir = &module.naga;
        let mut bindings = vec![];
        for (_, global) in ir.global_variables.iter() {
            let Some(binding) = &global.binding else { continue };
            if binding.group == 0 { continue; }
            let name = global.name.as_deref().unwrap_or_default();
            let (view, store, writes) = FrameView::named(name).ok_or_else(|| self.error(
                format!("`{label}` binds `{name}`, which is not a frame resource"),
                "bind vis_tex, hdr, hdr_tex, tiles, shade_args, draw_args, visible, insts or bases"))?;
            let listed = match store {
                ViewStore::Kinds => self.reads_kinds,
                ViewStore::Named(s) => self.writes.contains(&s) || !writes && self.reads.contains(&s),
            };
            if !listed {
                let (list, store) = match store {
                    ViewStore::Kinds => ("reads", "the drawn kinds"),
                    ViewStore::Named(s) => (if writes { "writes" } else { "reads" }, s),
                };
                return Err(self.error(format!("`{label}` binds `{name}` of {store}"), &format!("list {store} in `{list}`")));
            }
            bindings.push((binding.group, binding.binding, view));
        }
        let entry = |name: &str, stage: naga::ShaderStage| ir.entry_points.iter().find(|e| e.name == name && e.stage == stage)
            .ok_or_else(|| self.error(format!("the template has no {stage:?} entry `{name}`"), "name an entry the template declares"));
        let run = match &self.form {
            Form::Compute { entry: name, extent } => {
                let [x, y, _] = entry(name, naga::ShaderStage::Compute)?.workgroup_size;
                let extent = match extent {
                    ExtentSpec::Groups(groups) => Extent::Groups(*groups),
                    ExtentSpec::Instances => Extent::Instances { width: x },
                    ExtentSpec::Pixels => Extent::Pixels { tile: [x, y] },
                };
                AuthoredRun::Compute { entry: name.clone(), extent }
            }
            Form::FullScreen { vertex, fragment } => {
                entry(vertex, naga::ShaderStage::Vertex)?;
                entry(fragment, naga::ShaderStage::Fragment)?;
                AuthoredRun::FullScreen { vertex: vertex.clone(), fragment: fragment.clone() }
            }
        };
        // Shade modules decode a tile id as an 8x8 tile (`abi::TILE`), so the TileLists producer has that shape.
        let tiled = matches!(run, AuthoredRun::Compute { extent: Extent::Pixels { tile }, .. } if tile == [abi::TILE; 2]);
        if self.writes.contains(&"TileLists") && !tiled {
            return Err(self.error(format!("TileLists holds {0}x{0} tiles", abi::TILE),
                &format!("dispatch `{label}` over Pixels with @workgroup_size({0}, {0})", abi::TILE)));
        }
        Ok(AuthoredPass { label: label.clone(), module: index, run, bindings })
    }
}

fn fact<'p>(name: &str, facts: &Facts<'p>) -> Option<Fact<'p>> {
    Some(match name {
        "drawn" => Fact::Drawn(facts.drawn),
        "edges" => Fact::Edges(match facts.edges {
            Visibility::Center => "none",
            Visibility::SubpixelEdges => "taps",
            Visibility::Coverage => "coverage",
        }),
        "aov" => Fact::Aov(match facts.aov {
            Aov::Beauty => "beauty",
            Aov::Albedo => "albedo",
            Aov::Normal => "normal",
            Aov::Depth => "depth",
            Aov::Id => "id",
            Aov::SdfSteps => "sdf_steps",
            Aov::LipschitzRatio => "lipschitz_ratio",
            Aov::Overdraw => "overdraw",
        }),
        "paper" => Fact::Paper(facts.paper),
        _ => return None,
    })
}

fn equal(a: &Expr, b: &Expr, facts: &Facts<'_>) -> Res<bool> {
    let name = |e: &Expr| if let ExprKind::Name(n) = &e.kind { Some(n.clone()) } else { None };
    let (subject, other) = match (name(a).and_then(|n| fact(&n, facts)), name(b).and_then(|n| fact(&n, facts))) {
        (Some(f), None) => (f, b),
        (None, Some(f)) => (f, a),
        (Some(_), Some(_)) | (None, None) => return Err(Diag::new(Code::E520, a.span, "compare a frame fact with a value",
            "write `drawn == 1`, `edges == taps` or `aov != beauty`")),
    };
    let word = |allowed: &[&str], what: &str| match name(other) {
        Some(w) if allowed.contains(&w.as_str()) => Ok(w),
        Some(_) | None => Err(Diag::new(Code::E520, other.span, format!("`{what}` is one of {}", allowed.join(", ")), "compare with one of those words")),
    };
    match subject {
        Fact::Drawn(n) => {
            let ExprKind::Num { value, .. } = &other.kind else {
                return Err(Diag::new(Code::E520, other.span, "`drawn` is a count", "compare it with a number"));
            };
            Ok(f64::from(n) == *value)
        }
        Fact::Edges(v) => Ok(word(&EDGES, "edges")? == v),
        Fact::Aov(v) => Ok(word(&AOVS, "aov")? == v),
        Fact::Paper(_) => Err(Diag::new(Code::E520, other.span, "`paper` is tested, not compared", "write `paper` or `!paper`")),
    }
}
