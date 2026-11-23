use loft_ir::{Code, Diag, Res, Span};
use loft_parse::ast::*;
use std::collections::{BTreeMap, HashSet};

/// The program's items by name. Built from parsed modules; cheap to rebuild.
pub struct Prog<'a> {
    pub dimensions: std::collections::HashMap<Span, Dimension>,
    links: BTreeMap<(u16, &'a str), &'a str>,
    pub fns: BTreeMap<&'a str, &'a FnDecl>,
    pub kinds: BTreeMap<&'a str, &'a KindDecl>,
    /// Declaration order; a kind's index here is its stage-0 table.
    pub kind_order: Vec<&'a str>,
    pub consts: BTreeMap<&'a str, &'a ConstDecl>,
    pub worlds: BTreeMap<&'a str, &'a [Ident]>,
    pub spaces: HashSet<&'a str>,
    pub structs: BTreeMap<&'a str, (&'a Ident, &'a [RowField])>,
    pub aliases: BTreeMap<&'a str, (&'a Ident, &'a TyExpr)>,
    pub units: HashSet<&'a str>,
    pub scans: BTreeMap<&'a str, &'a ScanDecl>,
    pub schedule: Option<&'a ScheduleDecl>,
    pub kernels: BTreeMap<&'a str, &'a KernelDecl>,
    pub bakes: BTreeMap<&'a str, &'a BakeDecl>,
    pub laws:BTreeMap<&'a str,&'a LawDecl>,
    pub checks:BTreeMap<&'a str,&'a FnDecl>,
}

impl<'a> Prog<'a> {
    pub fn index(modules: &'a [Module]) -> Res<Prog<'a>> {
        let mut p = Prog {
            dimensions: modules.iter().flat_map(|m| m.dimensions.iter().copied()).collect(),
            links: modules.iter().filter_map(|m| m.file.map(|file| (file, &m.links))).flat_map(|(file, links)| {
                links.iter().map(move |(name, key)| ((file, name.as_str()), key.as_str()))
            }).collect(),
            fns: BTreeMap::new(),
            kinds: BTreeMap::new(),
            kind_order: vec![],
            consts: BTreeMap::new(),
            worlds: BTreeMap::new(),
            spaces: HashSet::new(),
            structs: BTreeMap::new(),
            aliases: BTreeMap::new(),
            units: HashSet::new(),
            scans: BTreeMap::new(),
            kernels: BTreeMap::new(),
            bakes: BTreeMap::new(),
            schedule: None,laws:BTreeMap::new(),checks:BTreeMap::new(),
        };
        let mut seen: BTreeMap<&str, Span> = BTreeMap::new();
        let mut name = |n: &'a Ident| -> Res<()> {
            if let Some(prev) = seen.insert(&n.name, n.span) {
                return Err(Diag::new(Code::E011, n.span, format!("`{}` is declared twice", n.name), "rename one")
                    .note(prev, "first declared here"));
            }
            Ok(())
        };
        for item in modules.iter().flat_map(|m| &m.items) {
            match item {
                // Spaces are their own namespace: `space Blob;` beside `kind Blob` is the natural spelling.
                Item::Space(names) => {
                    for n in names {
                        if !p.spaces.insert(&n.name) {
                            return Err(Diag::new(Code::E011, n.span, format!("space `{}` is declared twice", n.name), "declare it once"));
                        }
                    }
                }
                Item::Use(_) => {}
                Item::Camera(_) => {}
                Item::World(n, kinds) => {
                    name(n)?;
                    let mut seen = HashSet::new();
                    for kind in kinds {
                        if !seen.insert(&kind.name) {
                            return Err(Diag::new(Code::E003, kind.span, "duplicate world kind", "list each world kind once"));
                        }
                    }
                    p.worlds.insert(&n.name, kinds.as_slice());
                }
                Item::Bake(b) => {
                    name(&b.texel.name)?;
                    p.fns.insert(&b.texel.name.name, &b.texel);
                    p.bakes.insert(&b.texel.name.name, b);
                }
                Item::Kernel(k) => {
                    name(&k.f.name)?;
                    p.kernels.insert(&k.f.name.name, k);
                }
                Item::Law(l)=>{name(&l.f.name)?;p.laws.insert(&l.f.name.name,l);}
                Item::Check(f)=>{name(&f.name)?;p.checks.insert(&f.name.name,f);}
                Item::Struct(n, fields) => {
                    name(n)?;
                    p.structs.insert(&n.name, (n, fields));
                }
                Item::TypeAlias(n, ty) => {
                    name(n)?;
                    p.aliases.insert(&n.name, (n, ty));
                }
                Item::Unit(n) => {
                    name(n)?;
                    p.units.insert(&n.name);
                }
                Item::Const(c) => {
                    name(&c.name)?;
                    p.consts.insert(&c.name.name, c);
                }
                Item::Build(f) | Item::Fn(f) => {
                    name(&f.name)?;
                    p.fns.insert(&f.name.name, f);
                }
                Item::Kind(k) => {
                    name(&k.name)?;
                    p.kinds.insert(&k.name.name, k);
                    p.kind_order.push(&k.name.name);
                }
                Item::Scan(sc) => {
                    name(&sc.name)?;
                    p.scans.insert(&sc.name.name, sc);
                }
                Item::Schedule(s) => {
                    if let Some(prev) = p.schedule {
                        return Err(Diag::new(Code::E520, s.name.span, "a program has one schedule", "remove one")
                            .note(prev.name.span, "first schedule here"));
                    }
                    p.schedule = Some(s);
                }
            }
        }
        for kinds in p.worlds.values() {
            for kind in *kinds {
                if !p.kinds.contains_key(kind.name.as_str()) {
                    return Err(Diag::new(Code::E011, kind.span, format!("unknown world kind {}", kind.name), "declare the kind before using it in the world"));
                }
            }
        }
        if let Some((n, target)) = p.schedule.and_then(|s| s.radiance.as_ref()) {
            p.aliases.insert(&n.name, (n, target));
        }
        Ok(p)
    }

    pub fn linked(&self, owner: Span, name: &str) -> Res<&'a str> {
        if let Some(&key) = self.links.get(&(owner.file, name)) {
            return Ok(key);
        }
        // A scene that writes its own schedule rarely imports the pass's helpers (`paper_grain` for
        // `outline_and_paper`, 3 of 19 first attempts). The prelude binds them; one program-wide binding is
        // unambiguous, so it is used. Two different ones still need the import that chooses.
        let keys: std::collections::BTreeSet<&'a str> =
            self.links.iter().filter(|((_, n), _)| *n == name).map(|(_, &key)| key).collect();
        match keys.into_iter().collect::<Vec<&'a str>>()[..] {
            [key] => Ok(key),
            _ => Err(Diag::new(Code::E011, owner, format!("missing renderer binding `{name}`"),
                format!("import it in this file, e.g. `use stdlib::post::display::{{{name}}}`"))),
        }
    }

    pub fn continuous_params(&self) -> impl Iterator<Item = (&str, &ConstDecl)> {
        self.consts.iter().filter(|(_, c)| c.comptime && matches!(&c.ty, Some(TyExpr::Name(n)) if n.name == "f32")).map(|(&name, &c)| (name, c))
    }

    pub fn param_slot(&self, name: &str) -> Option<usize> {
        self.continuous_params().position(|(key, _)| key == name).map(|i| 3 + i)
    }

    pub fn runtime_constant(&self, name: &str) -> bool {
        let mut pending = vec![name];
        let mut known = HashSet::new();
        while let Some(name) = pending.pop() {
            if !known.insert(name) { continue; }
            let mut refs = crate::references::References::default();
            refs.expr(&self.consts[name].value, &[]);
            if matches!(self.consts[name].value.kind, ExprKind::If { .. }) || !refs.calls.is_empty()
                || refs.names.iter().any(|(n, _)| *n == "time" || self.param_slot(n).is_some())
            {
                return true;
            }
            pending.extend(refs.names.into_iter().map(|(n, _)| n).filter(|n| self.consts.contains_key(n)));
        }
        false
    }

    pub fn param_word(&self, name: &str) -> Option<usize> {
        self.param_slot(name).map(|slot| slot * 4)
    }

    pub fn kind_index(&self, name: &str) -> usize {
        self.kind_order.iter().position(|k| *k == name).expect("checked kind")
    }
}

/// The space a kind's queries are asked in: the surface's point, or the first point parameter.
pub fn kind_space(k: &KindDecl) -> &str {
    let t = match k.queries.first() {
        Some((QueryKind::Surface, f)) => f.ret.as_ref(),
        Some((QueryKind::Visible | QueryKind::Trace | QueryKind::Material, f)) => f.params.get(1).and_then(|p| p.ty.as_ref()),
        None => None,
    };
    match t {
        Some(TyExpr::Geo(_, s)) => &s.name,
        _ => "World",
    }
}

/// The row field that places a kind asked in its own space (`Xform[S -> World]`), checked to be unique.
pub fn pose_field(k: &KindDecl) -> Option<usize> {
    let s = if k.name.name == "Mesh" && k.queries.is_empty() { "Local" } else { kind_space(k) };
    if s == "World" {
        return None;
    }
    k.row.iter().position(|f| matches!(&f.ty, TyExpr::Xform(a, b) if a.name == s && b.name == "World"))
}

/// The light direction the twin evaluates materials under (world, normalised in use).
pub const TWIN_LIGHT: [f32; 3] = [0.3, 0.8, 0.5];

pub fn unit(v: [f32; 3]) -> [f32; 3] {
    let l = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    v.map(|x| x / l)
}
/// Explicit finite differences inside an opted-out geometry expression.
pub fn finite_diff_parameter(f: &FnDecl, parameter: &Ident) -> usize {
    if parameter.name.is_empty() { return 0; }
    f.params.iter().position(|p| p.name.name == parameter.name).expect("checked parameter")
}

pub const FINITE_DIFF_STEP: f32 = 1e-3;
