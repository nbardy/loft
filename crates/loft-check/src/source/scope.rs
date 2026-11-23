use super::Graph;
use loft_ir::{Code, Diag, Res, Span};
use loft_parse::ast::*;
use std::collections::{BTreeMap, HashMap, HashSet};

#[derive(Clone)]
struct Decl {
    key: String,
    span: Span,
    public: bool,
}

#[derive(Default)]
struct Names {
    values: BTreeMap<String, Decl>,
    spaces: BTreeMap<String, Decl>,
}

impl Names {
    fn insert(&mut self, space: bool, name: &str, decl: Decl, site: Span) -> Res<()> {
        let table = if space {
            &mut self.spaces
        } else {
            &mut self.values
        };
        if let Some(previous) = table.get(name) {
            // Two imports can name the same declaration through a diamond.
            if previous.span == decl.span {
                return Ok(());
            }
            return Err(Diag::new(
                Code::E011,
                site,
                format!("`{name}` is bound twice"),
                "select distinct names or use qualified imports",
            )
            .note(previous.span, "first declaration here")
            .note(decl.span, "other declaration here"));
        }
        table.insert(name.into(), decl);
        Ok(())
    }
}

fn error(span: Span, name: &str) -> Diag {
    Diag::new(
        Code::E011,
        span,
        format!("unresolved name `{name}`"),
        "import a public declaration or use a local name",
    )
}

fn stock(path: &str) -> bool {
    matches!(
        path,
        "stdlib/core.loft"
            | "stdlib/engines/act.loft"
            | "stdlib/engines/roam.loft"
            | "stdlib/engines/reference.loft"
            | "stdlib/engines/realtime.loft"
            | "stdlib/engines/woodblock.loft"
            | "stdlib/engines/spectral_caustic.loft"
            | "stdlib/geometry/analytic.loft"
            | "stdlib/geometry/layout.loft"
            | "stdlib/geometry/mesh.loft"
            | "stdlib/frames/transport.loft"
            | "stdlib/sdf/solid.loft"
            | "stdlib/noise/value.loft"
            | "stdlib/motion/solve.loft"
            | "stdlib/materials/bsdf.loft"
            | "stdlib/sky/environment.loft"
            | "stdlib/lighting/world.loft"
            | "stdlib/lighting/emitters.loft"
            | "stdlib/integrators/direct.loft"
            | "stdlib/integrators/restir.loft"
            | "stdlib/styles/woodblock.loft"
            | "stdlib/post/display.loft"
            | "stdlib/migration/bead_observation.loft"
            | "stdlib/migration/trace_coverage.loft"
    )
}

fn shared_space(n: &str) -> bool {
    matches!(n, "World" | "Local")
}

fn key(path: &str, name: &str) -> String {
    // Encode the portable lexical path, independently of DFS order or source text.
    let path = path
        .as_bytes()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<String>();
    format!("loft_m_{path}_{name}")
}

struct Linker<'a> {
    graph: &'a Graph,
    names: Vec<Names>,
    bare: HashSet<String>,
    bare_spaces: HashSet<String>,
    /// Public values of the stock library, which keep their bare names as keys and so are unique.
    stock_public: HashMap<String, Decl>,
}

impl<'a> Linker<'a> {
    fn new(graph: &'a Graph) -> Res<Self> {
        let mut names = Vec::with_capacity(graph.nodes.len());
        let stock_names = graph
            .nodes
            .iter()
            .enumerate()
            .filter(|(id, _)| graph.stock(*id as u16).is_some_and(stock))
            .flat_map(|(_, node)| node.module.items.iter().flat_map(Item::declarations))
            .map(|n| n.name.as_str())
            .collect::<HashSet<_>>();
        let mut bare = HashSet::new();
        let mut bare_spaces = HashSet::new();
        for (id, node) in graph.nodes.iter().enumerate() {
            let mut scope = Names::default();
            let path = &graph.sources.files[id].0;
            let stock = stock(path);
            for item in &node.module.items {
                if matches!(item, Item::Camera(_))
                    || matches!(item, Item::Schedule(_)) && id == usize::from(graph.entry)
                {
                    continue;
                }
                let space = matches!(item, Item::Space(_));
                for n in item.declarations() {
                    if space && shared_space(&n.name) {
                        continue;
                    }
                    let internal = n.name.starts_with('#');
                    let root = id == usize::from(graph.entry);
                    let keep = stock || internal || root && !stock_names.contains(n.name.as_str());
                    let key = if keep {
                        n.name.clone()
                    } else {
                        key(path, &n.name)
                    };
                    if keep {
                        (if space { &mut bare_spaces } else { &mut bare }).insert(n.name.clone());
                    }
                    scope.insert(
                        space,
                        &n.name,
                        Decl {
                            key,
                            span: n.span,
                            public: node.module.public.contains(&n.span),
                        },
                        n.span,
                    )?;
                }
            }
            names.push(scope);
        }
        let stock_public = graph.nodes.iter().enumerate()
            .filter(|(id, _)| graph.stock(*id as u16).is_some_and(stock))
            .flat_map(|(id, _)| names[id].values.iter().filter(|(_, d)| d.public).map(|(n, d)| (n.clone(), d.clone())))
            .collect();
        Ok(Self {
            graph,
            names,
            bare,
            bare_spaces,
            stock_public,
        })
    }

    fn scope(&self, id: u16) -> Res<Scope<'_>> {
        let own = &self.names[usize::from(id)];
        let mut scope = Scope {
            own,
            bare: &self.bare,
            bare_spaces: &self.bare_spaces,
            stock_public: &self.stock_public,
            imported: Names::default(),
            modules: HashMap::new(),
            intrinsic_modules: HashMap::new(),
            derivatives: HashMap::new(),
        };
        if id != self.graph.core {
            let core = &self.names[usize::from(self.graph.core)];
            for (name, d) in &core.values {
                if d.public {
                    scope.imported.insert(false, name, d.clone(), d.span)?;
                }
            }
        }
        let node = &self.graph.nodes[usize::from(id)];
        for (import, ids) in &node.imports {
            let dependencies = ids
                .iter()
                .map(|&id| &self.names[usize::from(id)])
                .collect::<Vec<_>>();
            let last = import.path.last().expect("parsed nonempty import");
            match &import.binding {
                ImportBinding::Module(alias) => {
                    let name = alias.as_ref().unwrap_or(last);
                    if let Some((_, first)) =
                        scope.modules.insert(&name.name, (dependencies, name.span))
                    {
                        return Err(Diag::new(
                            Code::E011,
                            name.span,
                            format!("module alias `{}` is bound twice", name.name),
                            "choose distinct module aliases",
                        )
                        .note(first, "first import here"));
                    }
                    if import.path[0].name == "std" && dependencies_are_intrinsic(import) {
                        scope.intrinsic_modules.insert(name.name.as_str(), import);
                    }
                    // Old engine selections exposed their authored renderer callbacks.
                    if import.path[0].name == "engines" {
                        for &dep in &scope.modules[name.name.as_str()].0 {
                            for (name, d) in &dep.values {
                                if d.public && !name.starts_with(char::is_uppercase) {
                                    scope.imported.insert(false, name, d.clone(), import.span)?;
                                }
                            }
                        }
                    }
                }
                ImportBinding::Selected(selected) => {
                    for n in selected {
                        let mut found = false;
                        for dep in &dependencies {
                            for (space, table) in [(false, &dep.values), (true, &dep.spaces)] {
                                if let Some(d) = table.get(&n.name) {
                                    if !d.public {
                                        return Err(private(n.span, &n.name, d.span));
                                    }
                                    scope.imported.insert(space, &n.name, d.clone(), n.span)?;
                                    found = true;
                                }
                            }
                        }
                        if found || intrinsic_alias(import, &n.name) {
                            continue;
                        }
                        // Imported from the wrong stock module: the name is unique, so bind the one that exists.
                        if let Some(decl) = self.stock_public.get(&n.name) {
                            scope.imported.insert(false, &n.name, decl.clone(), n.span)?;
                            continue;
                        }
                        // Models import builtins from the catalog module they resemble (`mix_bsdf` from bsdf).
                        if crate::check::builtin_names().any(|b| b == n.name) {
                            return Err(Diag::new(
                                Code::E011,
                                n.span,
                                format!("`{}` is a builtin, not a member of this module", n.name),
                                "builtins are always in scope; remove it from the import list",
                            ));
                        }
                        // Name the module that does export it: models file helpers under the catalog
                        // heading they resemble (`capsule_chain` under sdf::solid, 3 of 45 fresh crabs).
                        if let Some(module) = exporter(&self.graph.root, &n.name) {
                            return Err(Diag::new(
                                Code::E011,
                                n.span,
                                format!("`{}` is not exported here; `{module}` exports it", n.name),
                                format!("import it with `use {module}::{{{}}}`", n.name),
                            ));
                        }
                        return Err(error(n.span, &n.name));
                    }
                }
                ImportBinding::Wildcard => {
                    for dep in dependencies {
                        for (space, table) in [(false, &dep.values), (true, &dep.spaces)] {
                            for (name, d) in table {
                                if d.public {
                                    scope.imported.insert(space, name, d.clone(), import.span)?;
                                }
                            }
                        }
                    }
                }
            }
        }
        for (space, local, imported) in [
            (false, &own.values, &scope.imported.values),
            (true, &own.spaces, &scope.imported.spaces),
        ] {
            for (name, d) in local {
                if let Some(previous) = imported.get(name) {
                    let namespace = if space { "space" } else { "name" };
                    return Err(Diag::new(
                        Code::E011,
                        d.span,
                        format!("{namespace} `{name}` also imported"),
                        "use a qualified import or rename the declaration",
                    )
                    .note(previous.span, "imported declaration here"));
                }
            }
        }
        Ok(scope)
    }
}

fn dependencies_are_intrinsic(import: &Import) -> bool {
    matches!(import.path[1].name.as_str(), "math" | "color" | "quant")
}

fn intrinsic_alias(import: &Import, name: &str) -> bool {
    import.path[0].name == "std"
        && match import.path[1].name.as_str() {
            "quant" => crate::quant::Quant::resolve(name).is_some(),
            "math" => name == "TAU",
            _ => false,
        }
}

fn private(site: Span, name: &str, declaration: Span) -> Diag {
    Diag::new(
        Code::E011,
        site,
        format!("`{name}` is private"),
        "import a public declaration",
    )
    .note(declaration, "private declaration here")
}

struct Scope<'a> {
    own: &'a Names,
    bare: &'a HashSet<String>,
    bare_spaces: &'a HashSet<String>,
    stock_public: &'a HashMap<String, Decl>,
    imported: Names,
    modules: HashMap<&'a str, (Vec<&'a Names>, Span)>,
    intrinsic_modules: HashMap<&'a str, &'a Import>,
    derivatives: HashMap<String, String>,
}

impl Scope<'_> {
    fn resolve(&self, name: &str, span: Span, space: bool) -> Res<Option<String>> {
        if let Some(derived) = self.derivatives.get(name) {
            return Ok(Some(derived.clone()));
        }
        if let Some((alias, member)) = name.split_once("::") {
            let (modules, _) = self.modules.get(alias).ok_or_else(|| error(span, alias))?;
            for module in modules {
                let table = if space {
                    &module.spaces
                } else {
                    &module.values
                };
                if let Some(decl) = table.get(member) {
                    if !decl.public {
                        return Err(private(span, member, decl.span));
                    }
                    return Ok(Some(decl.key.clone()));
                }
            }
            if self
                .intrinsic_modules
                .get(alias)
                .is_some_and(|import| intrinsic_alias(import, member))
            {
                return Ok(Some(member.to_owned()));
            }
            return Err(error(span, name));
        }
        let (own, imported, bare) = if space {
            (&self.own.spaces, &self.imported.spaces, self.bare_spaces)
        } else {
            (&self.own.values, &self.imported.values, self.bare)
        };
        if let Some(decl) = own.get(name).or_else(|| imported.get(name)) {
            return Ok(Some(decl.key.clone()));
        }
        if let Some((kind, helper)) = name.split_once('#')
            && let Some(decl) = self.own.values.get(kind)
        {
            return Ok(Some(format!("{}#{helper}", decl.key)));
        }
        // A stock catalog name resolves without an import: its bare name is its key and is unique. A missing
        // or misplaced `use` (vnoise, smin, ggx, capsule_chain) was the commonest fresh-crab error left.
        if !space && let Some(decl) = self.stock_public.get(name) {
            return Ok(Some(decl.key.clone()));
        }
        if bare.contains(name) {
            return Err(error(span, name));
        }
        Ok(None)
    }

    fn ident(&self, n: &mut Ident) -> Res<()> {
        if let Some(key) = self.resolve(&n.name, n.span, false)? {
            n.name = key;
        }
        Ok(())
    }

    fn space(&self, n: &mut SpaceRef, locals: &[String]) -> Res<()> {
        if !n.var
            && !locals.contains(&n.name)
            && let Some(key) = self.resolve(&n.name, n.span, true)?
        {
            n.name = key;
        }
        Ok(())
    }

    fn ty(&self, ty: &mut TyExpr, locals: &[String]) -> Res<()> {
        match ty {
            // Renderer recipes bind this ABI type when the complete program is linked.
            TyExpr::Name(n) if n.name == "Radiance" => Ok(()),
            TyExpr::Name(n) | TyExpr::Table(n) => self.ident(n),
            TyExpr::Generic(n, args) => {
                self.ident(n)?;
                for ty in args {
                    self.ty(ty, locals)?;
                }
                Ok(())
            }
            TyExpr::Array(ty, _, _) => self.ty(ty, locals),
            TyExpr::Geo(_, n) | TyExpr::Field(n) | TyExpr::Frame(n) => self.space(n, locals),
            TyExpr::Xform(a, b) => {
                self.space(a, locals)?;
                self.space(b, locals)
            }
        }
    }

    fn expr(&self, e: &mut Expr, locals: &[String]) -> Res<()> {
        match &mut e.kind {
            ExprKind::Name(n) if !locals.contains(n) => {
                if let Some(key) = self.resolve(n, e.span, false)? {
                    *n = key;
                }
            }
            ExprKind::Name(_) | ExprKind::Num { .. } | ExprKind::Bool(_) | ExprKind::String(_) => {}
            ExprKind::Call { callee, args } => {
                self.ident(callee)?;
                self.args(args, locals)?;
            }
            ExprKind::Ctor { ty, args } => {
                self.ty(ty, locals)?;
                self.args(args, locals)?;
            }
            ExprKind::FiniteDiff { function, args, .. } => {
                self.ident(function)?;
                self.args(args, locals)?;
            }
            ExprKind::Member(base, _) | ExprKind::Unary(_, base) => self.expr(base, locals)?,
            ExprKind::Binary(_, a, b) | ExprKind::Index(a, b) => {
                self.expr(a, locals)?;
                self.expr(b, locals)?;
            }
            ExprKind::If {
                condition,
                then,
                otherwise,
            } => {
                self.expr(condition, locals)?;
                self.arm(then, locals)?;
                self.arm(otherwise, locals)?;
            }
            ExprKind::Unproved { step_scale, body } => {
                self.expr(step_scale, locals)?;
                self.arm(body, locals)?;
            }
        }
        Ok(())
    }

    fn arm(&self, arm: &mut ExprBlock, locals: &[String]) -> Res<()> {
        let mut locals = locals.to_vec();
        self.block(&mut arm.body, &mut locals)?;
        self.expr(&mut arm.value, &locals)
    }

    fn args(&self, args: &mut [Arg], locals: &[String]) -> Res<()> {
        for arg in args {
            self.expr(&mut arg.value, locals)?;
        }
        Ok(())
    }

    fn fields(&self, fields: &mut [(Ident, Expr)], locals: &[String]) -> Res<()> {
        for (_, expr) in fields {
            self.expr(expr, locals)?;
        }
        Ok(())
    }

    fn block(&self, block: &mut Block, locals: &mut Vec<String>) -> Res<()> {
        for stmt in &mut block.stmts {
            self.stmt(stmt, locals)?;
        }
        Ok(())
    }

    fn stmt(&self, stmt: &mut Stmt, locals: &mut Vec<String>) -> Res<()> {
        match stmt {
            Stmt::Let {
                name, ty, value, ..
            } => {
                if let Some(ty) = ty {
                    self.ty(ty, locals)?;
                }
                self.expr(value, locals)?;
                locals.push(name.name.clone());
            }
            Stmt::Assign { target, value, .. } => {
                self.expr(target, locals)?;
                self.expr(value, locals)?;
            }
            Stmt::Map { name, head, stages } => {
                self.expr(head, locals)?;
                for (_, args) in stages {
                    for e in args {
                        self.expr(e, locals)?;
                    }
                }
                locals.push(name.name.clone());
            }
            Stmt::Return(value, _) => {
                if let Some(e) = value {
                    self.expr(e, locals)?;
                }
            }
            Stmt::If {
                cond,
                then,
                otherwise,
            } => {
                self.expr(cond, locals)?;
                self.block(then, &mut locals.clone())?;
                if let Some(other) = otherwise {
                    self.block(other, &mut locals.clone())?;
                }
            }
            Stmt::Each {
                var, values, body, ..
            } => {
                for e in values {
                    self.expr(e, locals)?;
                }
                let mut inside = locals.clone();
                inside.push(var.name.clone());
                self.block(body, &mut inside)?;
            }
            Stmt::For {
                var, lo, hi, body, ..
            } => {
                self.expr(lo, locals)?;
                self.expr(hi, locals)?;
                let mut inside = locals.clone();
                inside.push(var.name.clone());
                self.block(body, &mut inside)?;
            }
            Stmt::Emit {
                kind, fields, bind, ..
            } => {
                self.ident(kind)?;
                self.fields(fields, locals)?;
                if let Some(n) = bind {
                    locals.push(n.name.clone());
                }
            }
            Stmt::ImportTable { kind, index, .. } => {
                self.ident(kind)?;
                self.expr(index, locals)?;
            }
            Stmt::Expr(e) => self.expr(e, locals)?,
        }
        Ok(())
    }

    fn range(&self, range: &mut RowRange) -> Res<()> {
        self.expr(&mut range.lo, &[])?;
        if let Some(hi) = &mut range.hi {
            self.expr(hi, &[])?;
        }
        Ok(())
    }

    fn columns(&self, fields: &mut [RowField]) -> Res<()> {
        for field in fields {
            self.ty(&mut field.ty, &[])?;
            if let Some(range) = &mut field.range {
                self.range(range)?;
            }
            if let Some(value) = &mut field.default {
                self.expr(value, &[])?;
            }
        }
        Ok(())
    }

    fn function(&self, f: &mut FnDecl) -> Res<()> {
        let mut locals = f
            .params
            .iter()
            .map(|p| p.name.name.clone())
            .collect::<Vec<_>>();
        for p in &mut f.params {
            if let Some(ty) = &mut p.ty {
                self.ty(ty, &[])?;
            }
        }
        if let Some(ty) = &mut f.ret {
            self.ty(ty, &[])?;
        }
        for attr in &mut f.attrs {
            for expr in &mut attr.args {
                self.expr(expr, &locals)?;
            }
        }
        self.block(&mut f.body, &mut locals)?;
        if let Some(raw) = &mut f.raw {
            let mut resolved = String::with_capacity(raw.body.len());
            for token in raw
                .body
                .split_inclusive(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
            {
                let word =
                    token.trim_end_matches(|c: char| !(c.is_ascii_alphanumeric() || c == '_'));
                if let Some(name) = word.strip_prefix("loft_user_") {
                    let key = self
                        .resolve(name, f.name.span, false)?
                        .ok_or_else(|| error(f.name.span, name))?;
                    resolved.push_str(&crate::references::fn_symbol(&key));
                    raw.references.push(Ident {
                        name: key,
                        span: f.name.span,
                    });
                    resolved.push_str(&token[word.len()..]);
                } else {
                    resolved.push_str(token);
                }
            }
            raw.body = resolved;
        }
        Ok(())
    }

    fn body_fields(&self, fields: &mut BodyFields, locals: &[String]) -> Res<()> {
        for (_, value) in fields {
            match value {
                BodyValue::Value(e) => self.expr(e, locals)?,
                BodyValue::Lengths(es) => {
                    for e in es {
                        self.expr(e, locals)?;
                    }
                }
                BodyValue::Range(a, b) => {
                    self.expr(a, locals)?;
                    self.expr(b, locals)?;
                }
                BodyValue::Record(fields) => self.body_fields(fields, locals)?,
            }
        }
        Ok(())
    }

    fn body(&self, items: &mut [BodyItem], locals: &mut Vec<String>) -> Res<()> {
        for item in items {
            match item {
                BodyItem::Trunk(_, fields) | BodyItem::Segment(_, fields) => {
                    self.body_fields(fields, locals)?
                }
                BodyItem::Chain(_, index, _, fields) => {
                    if let Some(e) = index {
                        self.expr(e, locals)?;
                    }
                    self.body_fields(fields, locals)?;
                }
                BodyItem::For(var, lo, hi, body) => {
                    self.expr(lo, locals)?;
                    self.expr(hi, locals)?;
                    let mut inside = locals.clone();
                    inside.push(var.name.clone());
                    self.body(body, &mut inside)?;
                }
                BodyItem::Let(name, expr) => {
                    self.expr(expr, locals)?;
                    locals.push(name.name.clone());
                }
                BodyItem::Waive(..) => {}
            }
        }
        Ok(())
    }

    fn item(&self, item: &mut Item) -> Res<()> {
        match item {
            Item::Use(_) | Item::Space(_) | Item::Unit(_) => {}
            Item::Fn(f) | Item::Check(f) | Item::Build(f) => self.function(f)?,
            Item::Law(l) => self.function(&mut l.f)?,
            Item::Kernel(k) => self.function(&mut k.f)?,
            Item::Bake(b) => {
                self.function(&mut b.texel)?;
                if let BakeClock::Dirty(names) = &mut b.clock {
                    for n in names {
                        self.ident(n)?;
                    }
                }
            }
            Item::TypeAlias(_, ty) => self.ty(ty, &[])?,
            Item::Struct(_, fields) => self.columns(fields)?,
            Item::State(state) => {
                if let Some(home) = &mut state.home {
                    self.ident(home)?;
                }
                self.columns(&mut state.fields)?;
            }
            Item::Body(_, body) => self.body(body, &mut vec![])?,
            Item::World(_, kinds) => {
                for kind in kinds {
                    self.ident(kind)?;
                }
            }
            Item::Camera(c) => self.fields(&mut c.fields, &[])?,
            Item::Const(c) => {
                if let Some(ty) = &mut c.ty {
                    self.ty(ty, &[])?;
                }
                if let Some(range) = &mut c.range {
                    self.range(range)?;
                }
                self.expr(&mut c.value, &[])?;
            }
            Item::Kind(k) => {
                for p in &mut k.params {
                    self.ty(&mut p.ty, &[])?;
                    if let Some((lo, hi)) = &mut p.range {
                        self.expr(lo, &[])?;
                        self.expr(hi, &[])?;
                    }
                }
                self.columns(&mut k.row)?;
                let custom = &self.own.values[&k.name.name].key;
                for f in &mut k.fns {
                    self.function(f)?;
                    if let Some((_, tail)) = f.name.name.split_once('#') {
                        f.name.name = format!("{custom}#{tail}");
                    }
                }
                for (_, f) in &mut k.queries {
                    self.function(f)?;
                }
            }
            Item::Scan(s) => {
                self.ty(&mut s.var_ty, &[])?;
                if let Some(ty) = &mut s.elem {
                    self.ty(ty, &[])?;
                }
                for e in [&mut s.lo, &mut s.hi, &mut s.steps, &mut s.init] {
                    self.expr(e, &[])?;
                }
                self.expr(
                    &mut s.step,
                    &[s.prev.name.clone(), s.at.name.clone(), s.var.name.clone()],
                )?;
            }
            Item::Plan(p) => {
                self.columns(&mut p.params)?;
                self.ty(&mut p.ret, &[])?;
                let locals = p
                    .params
                    .iter()
                    .map(|p| p.name.name.clone())
                    .collect::<Vec<_>>();
                if let Some((_, dt)) = &mut p.horizon {
                    self.expr(dt, &locals)?;
                }
                self.expr(&mut p.objective, &locals)?;
                for e in &mut p.holds {
                    self.expr(e, &locals)?;
                }
            }
            Item::Solver(s) => {
                if let Some(head) = s.table.first_mut() {
                    self.ident(head)?;
                }
                self.fields(&mut s.config, &[])?;
                let mut locals = s.writes.iter().map(|(n, _)| n.name.clone()).collect();
                for step in &mut s.steps {
                    match step {
                        SolverStep::Statement(s) => self.stmt(s, &mut locals)?,
                        SolverStep::Constraint {
                            a,
                            b,
                            lo,
                            hi,
                            block,
                            ..
                        } => {
                            self.expr(lo, &locals)?;
                            self.expr(hi, &locals)?;
                            let mut inside = locals.clone();
                            inside.extend([a.name.clone(), b.name.clone()]);
                            self.block(block, &mut inside)?;
                        }
                    }
                }
            }
            Item::Retarget {
                source,
                target,
                fields,
                ..
            } => {
                self.ident(source)?;
                self.ident(target)?;
                self.fields(fields, &[])?;
            }
            Item::Schedule(s) => {
                if let Some((_, ty)) = &mut s.radiance {
                    self.ty(ty, &[])?;
                }
                for pass in &mut s.passes {
                    self.ident(&mut pass.verb)?;
                    self.args(&mut pass.args, &[])?;
                    for (name, args) in &mut pass.body {
                        self.ident(name)?;
                        self.args(args, &[])?;
                    }
                    if let Some(resources) = &mut pass.resources {
                        for texture in &mut resources.reads {
                            self.ident(texture)?;
                        }
                        self.block(&mut resources.body, &mut vec![])?;
                    }
                }
            }
        }
        self.rename(item);
        Ok(())
    }

    fn rename(&self, item: &mut Item) {
        let name = match item {
            Item::Use(_) | Item::Build(_) => return,
            Item::Space(names) => {
                for n in names {
                    if let Some(d) = self.own.spaces.get(&n.name) {
                        n.name.clone_from(&d.key);
                    }
                }
                return;
            }
            Item::Struct(n, _)
            | Item::TypeAlias(n, _)
            | Item::Unit(n)
            | Item::Body(n, _)
            | Item::World(n, _)
            | Item::Retarget { name: n, .. } => n,
            Item::Const(x) => &mut x.name,
            Item::Fn(x) | Item::Check(x) => &mut x.name,
            Item::Kind(x) => &mut x.name,
            Item::Scan(x) => &mut x.name,
            Item::Schedule(x) => &mut x.name,
            Item::Law(x) => &mut x.f.name,
            Item::State(x) => &mut x.name,
            Item::Solver(x) => &mut x.name,
            Item::Plan(x) => &mut x.name,
            Item::Kernel(x) => &mut x.f.name,
            Item::Camera(x) => &mut x.name,
            Item::Bake(x) => &mut x.texel.name,
        };
        if let Some(d) = self.own.values.get(&name.name) {
            name.name.clone_from(&d.key);
        }
    }

    fn rewrite(&mut self, module: &mut Module) -> Res<()> {
        for (f, x) in &mut module.diffs {
            let authored = format!("{}__d_{}", f.name, x.name);
            self.ident(f)?;
            self.derivatives
                .insert(authored, format!("{}__d_{}", f.name, x.name));
        }
        for (name, decl) in self.own.values.iter().chain(&self.imported.values) {
            if !name.starts_with('#') {
                module.links.push((name.clone(), decl.key.clone()));
            }
        }
        for item in &mut module.items {
            self.item(item)?;
        }
        Ok(())
    }
}

pub(super) fn edited(graph: &Graph, id: u16, mut module: Module) -> Res<Module> {
    Linker::new(graph)?.scope(id)?.rewrite(&mut module)?;
    Ok(module)
}

pub(super) fn link(graph: &Graph) -> Res<(Vec<Module>, u32)> {
    let linker = Linker::new(graph)?;
    let mut recipe = None;
    let mut modules = Vec::with_capacity(graph.nodes.len());
    for &id in &graph.order {
        let node = &graph.nodes[usize::from(id)];
        let mut module = node.module.clone();
        linker.scope(id)?.rewrite(&mut module)?;
        if id == graph.entry {
            for item in &mut module.items {
                if let Item::Schedule(s) = item
                    && module.public.contains(&s.name.span)
                {
                    s.implicit = true;
                }
            }
        } else {
            module.items.retain_mut(|item| match item {
                Item::Build(_) | Item::Camera(_) => false,
                Item::Schedule(s) => {
                    if graph.recipe.is_some_and(|(r, _)| r == id) {
                        recipe = Some(s.clone());
                    }
                    false
                }
                Item::Space(names) => {
                    names.retain(|n| !shared_space(&n.name));
                    !names.is_empty()
                }
                _ => true,
            });
        }
        // Stable ABI order is separate from the traversal and diagnostic file IDs.
        let rank = match graph.stock(id) {
            Some("stdlib/core.loft") => 0,
            Some("stdlib/geometry/mesh.loft") => 1,
            Some("stdlib/lighting/emitters.loft") => 2,
            Some("stdlib/lighting/world.loft") => 3,
            _ if id == graph.entry => 5,
            _ => 4,
        };
        modules.push((rank, module));
    }
    modules.sort_by_key(|(rank, _)| *rank);
    let mut modules = modules.into_iter().map(|(_, module)| module).collect();
    let mut next = graph.next;
    crate::defaults::schedule(&mut modules, &mut next, recipe)?;
    Ok((modules, next))
}

/// The stdlib module that publicly declares `name`, read from disk on the error path only.
fn exporter(root: &std::path::Path, name: &str) -> Option<String> {
    fn walk(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
        let Ok(entries) = std::fs::read_dir(dir) else { return };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                walk(&path, out);
            } else if path.extension().is_some_and(|ext| ext == "loft") {
                out.push(path);
            }
        }
    }
    let stdlib = root.join("stdlib");
    let mut files = vec![];
    walk(&stdlib, &mut files);
    files.sort();
    files.into_iter().find_map(|path| {
        let text = std::fs::read_to_string(&path).ok()?;
        let declares = text.lines().any(|line| {
            let mut words = line.split_whitespace().skip_while(|w| *w != "pub").skip(1)
                .skip_while(|w| *w == "build");
            words.next().is_some_and(|kind| matches!(kind, "fn" | "lobe" | "kind" | "const" | "type" | "schedule"))
                && words.next().is_some_and(|w| w.split(|c: char| !(c.is_alphanumeric() || c == '_')).next() == Some(name))
        });
        let module = path.strip_prefix(&stdlib).ok()?.with_extension("");
        declares.then(|| format!("stdlib::{}", module.components()
            .map(|c| c.as_os_str().to_string_lossy().into_owned()).collect::<Vec<_>>().join("::")))
    })
}
