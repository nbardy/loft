//! Layer 2 of the meta engine (plan 2026-11-20 §1): every pass from source to `Program`, one module per pass.
//!
//!   parse -> index -> check -> lower the schedule -> stage 0 (rows) -> derive (bounds, L) -> emit -> naga validate
//!
//! `compile` runs them all; `Compiled` keeps what the gates need afterwards (the CPU twin of every query).

pub mod ad;
pub mod check;
mod cache;
mod defaults;
mod refine;
mod provenance;
mod infer;
pub mod emit;
pub mod eval;
pub mod lip;
pub use loft_parse as parse;

use eval::{Eval, RowVal, Tables, Val};
use lip::Iv;
use loft_parse::ast::*;
use crate::program::{abi, Draw, Instance, KindTable, Orbit, Pass, Program, ShaderModule};
use loft_ir::{Code, Diag, Res, Sources, Span};
use std::collections::{BTreeMap, HashSet};
use std::time::Instant;

/// The program's items by name. Built from parsed modules; cheap to rebuild.
pub struct Prog<'a> {
    dimensions: std::collections::HashMap<Span, Dimension>,
    pub fns: BTreeMap<&'a str, &'a FnDecl>,
    pub kinds: BTreeMap<&'a str, &'a KindDecl>,
    /// Declaration order; a kind's index here is its stage-0 table.
    pub kind_order: Vec<&'a str>,
    pub consts: BTreeMap<&'a str, &'a ConstDecl>,
    pub spaces: HashSet<&'a str>,
    pub structs: BTreeMap<&'a str, (&'a Ident, &'a [RowField])>,
    pub aliases: BTreeMap<&'a str, (&'a Ident, &'a TyExpr)>,
    pub units: HashSet<&'a str>,
    pub scans: BTreeMap<&'a str, &'a ScanDecl>,
    pub schedule: Option<&'a ScheduleDecl>,
    pub kernels: BTreeMap<&'a str, &'a KernelDecl>,
    pub laws:BTreeMap<&'a str,&'a LawDecl>,
    pub checks:BTreeMap<&'a str,&'a FnDecl>,
}

impl<'a> Prog<'a> {
    pub fn index(modules: &'a [Module]) -> Res<Prog<'a>> {
        let mut p = Prog {
            dimensions: modules.iter().flat_map(|m| m.dimensions.iter().copied()).collect(),
            fns: BTreeMap::new(),
            kinds: BTreeMap::new(),
            kind_order: vec![],
            consts: BTreeMap::new(),
            spaces: HashSet::new(),
            structs: BTreeMap::new(),
            aliases: BTreeMap::new(),
            units: HashSet::new(),
            scans: BTreeMap::new(),
            kernels: BTreeMap::new(),
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
                Item::Use(path) => {
                    let parts: Vec<_> = path.iter().map(|n| n.name.as_str()).collect();
                    if !matches!(parts.as_slice(), ["std", "geom" | "light" | "math" | "color" | "noise"] | ["engines", "realtime"]) {
                        let span = path.first().unwrap().span.to(path.last().unwrap().span);
                        return Err(Diag::new(Code::E011, span, format!("unresolved import {}", parts.join("::")), "use a module supplied by the loaded prelude"));
                    }
                }
                Item::Camera(_) => {}
                Item::Retarget { name: n, .. } => name(n)?,
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
        if let Some((n, target)) = p.schedule.and_then(|s| s.radiance.as_ref()) {
            p.aliases.insert(&n.name, (n, target));
        }
        Ok(p)
    }

    fn continuous_params(&self) -> impl Iterator<Item = (&str, &ConstDecl)> {
        self.consts.iter().filter(|(_, c)| c.comptime && matches!(&c.ty, Some(TyExpr::Name(n)) if n.name == "f32")).map(|(&name, &c)| (name, c))
    }

    fn param_slot(&self, name: &str) -> Option<usize> {
        self.continuous_params().position(|(key, _)| key == name).map(|i| 3 + i)
    }

    fn runtime_constant(&self, name: &str) -> bool {
        let mut pending = vec![name];
        let mut known = HashSet::new();
        while let Some(name) = pending.pop() {
            if !known.insert(name) { continue; }
            let mut refs = emit::References::default();
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

    pub fn kind_index(&self, name: &str) -> usize {
        self.kind_order.iter().position(|k| *k == name).expect("checked kind")
    }
}

/// Parse every file, then generate the derivative fns `diff` asked for (one more module at the end).
pub fn parse_all(src: &Sources) -> Res<Vec<Module>> {
    let mut next = 0;
    let mut modules = src.files.iter().enumerate().map(|(i, (_, text))| parse::parse(i as u16, text, &mut next)).collect::<Res<Vec<_>>>()?;
    // Certify the normalized function and the actual resolved call closure. A
    // comment or a different function in the same file has no bearing on it.
    let certified = {
        let p = Prog::index(&modules)?;
        let mut certified = HashSet::new();
        for (name, hash) in [
            ("box_sdf", "103768bf7d29ffc6232109015b9f5dc3419951aeba89bac263e4b8bbf2c22035"),
            ("capsule", "d5969ede3cb6b897e19919fe25668130e063e5649a1287d6e2480e5e0113e261"),
            ("round_box", "f34d6300722e78c429d9810a41bb4b0f5c4ded1b2e05f5fbfab8acc926fa05c9"),
            ("smin", "90337285c51e676a04494e72cd5c0e7249e39f241ac12d77d4efa91c433117ca"),
            ("sphere", "f8de200152a8ad78550c80e959ea5d93e4bd9e0e4921063373e07fbaf5368add"),
            ("ssub", "0c3786ccda1d02f6cd30c6fca3cfb2645161b4365f2208a09f953edf00650697"),
        ] {
            if let Some(f) = p.fns.get(name) && cache::certificate(&p, f)? == hash {
                certified.insert(f.name.span);
            }
        }
        certified
    };
    for item in modules.iter_mut().flat_map(|m| &mut m.items) {
        let Item::Fn(f) = item else { continue };
        let sp = f.name.span;
        if !certified.contains(&sp) { continue; }
        let make = |name: &str, next: &mut u32| {
            let id = *next;
            *next += 1;
            Expr { id, span: sp, kind: ExprKind::Name(name.into()) }
        };
        let ax = match f.name.name.as_str() {
            "sphere" | "round_box" | "box_sdf" => make("v", &mut next),
            "capsule" => make("p", &mut next),
            "smin" | "ssub" => {
                let args = ["a", "b"].map(|n| Arg { name: None, value: make(n, &mut next) }).to_vec();
                let id = next;
                next += 1;
                Expr { id, span: sp, kind: ExprKind::Call { callee: Ident { name: "max".into(), span: sp }, args } }
            }
            _ => unreachable!("known certified function"),
        };
        f.attrs.push(Attr { name: Ident { name: "lipschitz".into(), span: sp }, args: vec![ax] });
    }
    defaults::schedule(&mut modules,&mut next)?;
    infer::run(&mut modules)?;
    ad::run(&mut modules, &mut next)?;
    Ok(modules)
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

/// The row fields a kind's geometry (trace, visible, surface) reads: the key that groups rows into shapes.
fn geometry_fields(k: &KindDecl) -> Vec<usize> {
    fn walk(e: &Expr, out: &mut Vec<String>) {
        match &e.kind {
            ExprKind::Unproved { step_scale, body } => {
                walk(step_scale, out);
                block(&body.body, out);
                walk(&body.value, out);
            }
            ExprKind::FiniteDiff { args, .. } => args.iter().for_each(|a| walk(&a.value, out)),
            ExprKind::If { condition, then, otherwise } => {
                walk(condition, out);
                for arm in [then, otherwise] { block(&arm.body, out); walk(&arm.value, out); }
            }
            ExprKind::Member(b, m) => {
                if matches!(&b.kind, ExprKind::Name(n) if n == "self") {
                    out.push(m.name.clone());
                } else {
                    walk(b, out);
                }
            }
            ExprKind::Call { args, .. } | ExprKind::Ctor { args, .. } => args.iter().for_each(|a| walk(&a.value, out)),
            ExprKind::Index(a, b) | ExprKind::Binary(_, a, b) => {
                walk(a, out);
                walk(b, out);
            }
            ExprKind::Unary(_, a) => walk(a, out),
            // Passing the entire row to a helper (including through a local)
            // lets its callee read any column. Include all words in this key.
            ExprKind::Name(n) if n == "self" => out.push(n.clone()),
            ExprKind::Num { .. } | ExprKind::Bool(_) | ExprKind::String(_) | ExprKind::Name(_) => {}
        }
    }
    fn block(b: &Block, out: &mut Vec<String>) {
        for s in &b.stmts {
            match s {
                Stmt::Let { value, .. } | Stmt::Assign { value, .. } => walk(value, out),
                Stmt::Map { head, stages, .. } => {
                    walk(head, out);
                    stages.iter().flat_map(|(_, a)| a).for_each(|a| walk(a, out));
                }
                Stmt::Return(v, _) => v.iter().for_each(|e| walk(e, out)),
                Stmt::If { cond, then, otherwise } => {
                    walk(cond, out);
                    block(then, out);
                    otherwise.iter().for_each(|o| block(o, out));
                }
                Stmt::Each{values,body,..}=>{for e in values{walk(e,out)}block(body,out);}
                Stmt::For { lo, hi, body, .. } => {
                    walk(lo, out);
                    walk(hi, out);
                    block(body, out);
                }
                Stmt::Expr(e) => walk(e, out),
                Stmt::Emit { .. } => {}
            }
        }
    }
    let mut names = vec![];
    for (q, f) in &k.queries {
        if *q != QueryKind::Material {
            block(&f.body, &mut names);
        }
    }
    // A placement applied by the draw is external to this proof. A query
    // explicitly reading its placement (for world-space wind, for example)
    // makes those words ordinary shape dependencies.
    (0..k.row.len()).filter(|i| names.iter().any(|name| name == "self" || name == &k.row[*i].name.name)).collect()
}

/// Per drawn kind: rows, distinct shapes (one bound derivation each) and the derived L range.
pub struct KindReport {
    pub name: String,
    pub rows: usize,
    pub shapes: usize,
    pub l_min: f64,
    pub l_max: f64,
    pub evals: u64,
    pub cache_hits: usize,
    pub draw: Draw,
}

pub struct Report {
    pub kinds: Vec<KindReport>,
    /// Wall milliseconds per pass, in order.
    pub ms: Vec<(&'static str, f64)>,
}

/// What survives compilation for the gates: the source, types, scans and stage-0 tables, so any query runs on the CPU.
pub struct Compiled {
    pub modules: Vec<Module>,
    pub types: Vec<Ty>,
    pub tables: Tables,
    pub luts: Vec<eval::Lut>,
    /// Drawn kinds by kind id: (stage-0 table index, name).
    pub drawn: Vec<(usize, String)>,
    meshes: Vec<gltf::Mesh>,
}

/// One kind's queries on the CPU in f32 (the twin). Traced and analytic kinds take a world point and direction;
/// surface kinds take (u, v) in `p[0..2]`. `material` is the Bsdf seen along its normal, lit 60 degrees off it (in
/// the plane of the normal and `TWIN_LIGHT`), plus its emission.
pub struct CpuQuery {
    pub trace: f32,
    pub visible: f32,
    pub material: [f32; 3],
    pub surf_p: [f32; 3],
    pub surf_n: [f32; 3],
}

/// The light direction the twin evaluates materials under (world, normalised in use).
pub const TWIN_LIGHT: [f32; 3] = [0.3, 0.8, 0.5];

pub(crate) fn unit(v: [f32; 3]) -> [f32; 3] {
    let l = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    v.map(|x| x / l)
}
fn rot(r: &eval::Rigid, v: [f32; 3]) -> [f32; 3] {
    [0, 1, 2].map(|i| r.r[i][0] * v[0] + r.r[i][1] * v[1] + r.r[i][2] * v[2])
}
fn v3<S: eval::Scalar>(x: [f32; 3]) -> Val<S> {
    Val::V(x.iter().map(|c| S::c(f64::from(*c))).collect(), 0.0)
}
fn of_v3(v: Val<f32>) -> [f32; 3] {
    match v {
        Val::V(c, _) => [c[0], c[1], c[2]],
        other @ Val::Void | other @ Val::S(_) | other @ Val::B(_) | other @ Val::U(_) | other @ Val::I(_) | other @ Val::X(_) | other @ Val::Row(..) | other @ Val::MapRef(..) | other @ Val::Bsdf(_) => unreachable!("a vec3, got {other:?}"),
    }
}

impl Compiled {
    pub fn prog(&self) -> Prog<'_> {
        Prog::index(&self.modules).expect("indexed once already")
    }

    pub fn mesh_triangles(&self) -> impl Iterator<Item = &[[[f32; 3]; 3]]> {
        self.meshes.iter().map(|mesh| mesh.triangles.as_slice())
    }

    /// The row's placement in World (identity for a kind asked in World).
    pub fn pose(&self, kind_id: usize, row: usize) -> eval::Rigid {
        let prog = self.prog();
        let (table, name) = &self.drawn[kind_id];
        match pose_field(prog.kinds[name.as_str()]) {
            Some(i) => match &self.tables[*table][row][i] {
                RowVal::X(x) => *x,
                other @ RowVal::F(_) | other @ RowVal::V(_) | other @ RowVal::U(_) | other @ RowVal::I(_) | other @ RowVal::B(_) => unreachable!("checked pose, got {other:?}"),
            },
            None => eval::Rigid::yaw([0.0; 3], 0.0),
        }
    }

    pub fn cpu_query(&self, kind_id: usize, row: usize, p: [f32; 3], rd: [f32; 3]) -> Res<CpuQuery> {
        let prog = self.prog();
        let (table, name) = &self.drawn[kind_id];
        let k = prog.kinds[name.as_str()];
        let mut ev: Eval<f32> = Eval::new(&prog, &self.types, self.tables.clone(), &[]);
        ev.luts = &self.luts;
        let pose = self.pose(kind_id, row);
        let to_local = pose.inv();
        let me = Val::Row(*table, row);
        let sp = Span::default();
        let f32_of = |v: Val<f32>| match v {
            Val::S(s) => s,
            other @ Val::Void | other @ Val::V(..) | other @ Val::B(_) | other @ Val::U(_) | other @ Val::I(_) | other @ Val::X(_) | other @ Val::Row(..) | other @ Val::MapRef(..) | other @ Val::Bsdf(_) => unreachable!("query returns f32, got {other:?}"),
        };
        let mut out = CpuQuery { trace: 0.0, visible: 0.0, material: [0.0; 3], surf_p: [0.0; 3], surf_n: [0.0; 3] };
        if name == "Mesh" && k.queries.is_empty() {
            let RowVal::U(asset) = self.tables[*table][row][1] else { unreachable!("checked asset") };
            let mesh = &self.meshes[asset as usize];
            let r = rot(&to_local, p);
            let origin = std::array::from_fn(|i| r[i] + to_local.t[i]);
            let n = unit(rd);
            out.visible = mesh.hit(origin, unit(rot(&to_local, rd)));
            let a = unit(TWIN_LIGHT);
            let t = unit([n[1] * a[2] - n[2] * a[1], n[2] * a[0] - n[0] * a[2], n[0] * a[1] - n[1] * a[0]]);
            let lt = std::array::from_fn(|i| n[i] * 0.5 + t[i] * 0.866_025_4);
            let b = Val::Bsdf(vec![("ggx".into(), vec![v3(mesh.color), Val::S(mesh.roughness), Val::S(mesh.metal)])]);
            out.material = of_v3(ev.eval_bsdf(&b, &v3(n), &v3(n), &v3(lt), sp)?);
            return Ok(out);
        }
        let (pl, nl, nw, uv) = match k.query(QueryKind::Surface) {
            Some(surf) => {
                let (u, v) = (Val::S(p[0]), Val::S(p[1]));
                let jvp = k.fns.iter().find(|f| f.name.name == "surface__jvp").expect("generated");
                let at = of_v3(ev.call_fn(surf, vec![me.clone(), u.clone(), v.clone()], sp)?);
                let du = of_v3(ev.call_fn(jvp, vec![me.clone(), u.clone(), v.clone(), Val::S(1.0), Val::S(0.0)], sp)?);
                let dv = of_v3(ev.call_fn(jvp, vec![me.clone(), u, v, Val::S(0.0), Val::S(1.0)], sp)?);
                let n = unit([du[1] * dv[2] - du[2] * dv[1], du[2] * dv[0] - du[0] * dv[2], du[0] * dv[1] - du[1] * dv[0]]);
                let pw = rot(&pose, at);
                out.surf_p = [0, 1, 2].map(|i| pw[i] + pose.t[i]);
                out.surf_n = rot(&pose, n);
                (at, n, out.surf_n, Some([p[0], p[1]]))
            }
            None => {
                let pl = { let r = rot(&to_local, p); [0, 1, 2].map(|i| r[i] + to_local.t[i]) };
                let dl = unit(rot(&to_local, rd));
                out.trace = f32_of(ev.call_fn(k.query(QueryKind::Trace).expect("checked"), vec![me.clone(), v3(pl)], sp)?);
                if let Some(f) = k.query(QueryKind::Visible) {
                    out.visible = f32_of(ev.call_fn(f, vec![me.clone(), v3(pl), v3(dl)], sp)?);
                }
                (pl, dl, unit(rd), None)
            }
        };
        let mut args = vec![me, v3(pl), v3(nl)];
        if let Some(uv) = uv {
            args.push(Val::V(vec![uv[0], uv[1]], 0.0));
        }
        let b = ev.call_fn(k.query(QueryKind::Material).expect("checked"), args, sp)?;
        let (a, k) = (unit(TWIN_LIGHT), nw);
        let t = unit([k[1] * a[2] - k[2] * a[1], k[2] * a[0] - k[0] * a[2], k[0] * a[1] - k[1] * a[0]]);
        let lt = [0, 1, 2].map(|i| k[i] * 0.5 + t[i] * 0.866_025_4);
        let lit = of_v3(ev.eval_bsdf(&b, &v3(nw), &v3(nw), &v3(lt), sp)?);
        let glow = of_v3(ev.eval_bsdf(&b, &v3(nw), &v3(nw), &v3([0.0; 3]), sp)?);
        out.material = [0, 1, 2].map(|i| lit[i] + glow[i]);
        Ok(out)
    }
}

fn arg<'e>(pd: &'e PassDecl, name: &str) -> Res<&'e Expr> {
    pd.args.iter().find(|a| a.name.as_ref().is_some_and(|n| n.name == name)).map(|a| &a.value).ok_or_else(|| {
        Diag::new(Code::E520, pd.verb.span, format!("`{}` needs `{name}: ...`", pd.verb.name), format!("add `{name}: ...`"))
    })
}

/// The kind names among a pass's positional arguments.
fn kind_args<'p>(p: &Prog<'p>, pd: &PassDecl) -> Res<Vec<&'p str>> {
    let mut kinds = vec![];
    for a in pd.args.iter().filter(|a| a.name.is_none()) {
        let ExprKind::Name(n) = &a.value.kind else {
            return Err(Diag::new(Code::E011, a.value.span, "expected kind name", "name a declared drawable kind"));
        };
        if a.value.span.file == Span::NONE.file && matches!(n.as_str(), "Drawables" | "Casters") {
            kinds.extend(p.kind_order.iter().copied().filter(|k| {
                let kind = p.kinds[k];
                (!kind.queries.is_empty() || *k == "Mesh") && (n == "Drawables" || pose_field(kind).is_some())
            }));
        } else if let Some((&name, _)) = p.kinds.get_key_value(n.as_str()) {
            kinds.push(name);
        } else {
            return Err(Diag::new(Code::E011, a.value.span, format!("unknown kind {n}"), "name a declared drawable kind"));
        }
    }
    Ok(kinds)
}

fn fn_name_arg<'e>(p: &Prog, pd: &'e PassDecl, name: &str) -> Res<&'e str> {
    let e = arg(pd, name)?;
    match &e.kind {
        ExprKind::Name(n) if p.fns.contains_key(n.as_str()) => Ok(n),
        ExprKind::Unproved { .. } | ExprKind::FiniteDiff { .. } | ExprKind::If { .. } | ExprKind::Num { .. } | ExprKind::Bool(_) | ExprKind::String(_) | ExprKind::Name(_) | ExprKind::Member(..) | ExprKind::Index(..) | ExprKind::Call { .. } | ExprKind::Ctor { .. } | ExprKind::Binary(..) | ExprKind::Unary(..) => Err(Diag::new(Code::E011, e.span, format!("`{name}:` names a library fn"), "write the fn's name, e.g. `sky: sky_color`")),
    }
}

/// Stage-0 value of a schedule argument.
fn stage0(p: &Prog, types: &mut Vec<Ty>, e: &Expr, overrides: &[(String, f64)]) -> Res<RowVal> {
    check::check_stage0_expr(p, types, e)?;
    let mut ev: Eval<f32> = Eval::new(p, types, vec![], overrides);
    eval::concrete(&ev.expr(e)?, e.span)
}

fn as_f32(v: RowVal, span: Span) -> Res<f32> {
    match v {
        RowVal::F(x) => Ok(x),
        RowVal::U(u) => Ok(u as f32),
        RowVal::I(i) => Ok(i as f32),
        other @ RowVal::V(_) | other @ RowVal::B(_) | other @ RowVal::X(_) => Err(Diag::new(Code::E003, span, format!("expected a number, found {other:?}"), "pass an f32")),
    }
}
fn as_v3(v: RowVal, span: Span) -> Res<[f32; 3]> {
    match v {
        RowVal::V(c) if c.len() == 3 => Ok([c[0], c[1], c[2]]),
        other @ RowVal::F(_) | other @ RowVal::V(_) | other @ RowVal::U(_) | other @ RowVal::I(_) | other @ RowVal::B(_) | other @ RowVal::X(_) => Err(Diag::new(Code::E003, span, format!("expected a vec3, found {other:?}"), "pass vec3(x, y, z) or Pt[S](...)")),
    }
}

/// Words of one row in the GPU table: the author's fields in order (Xform: rotation column-major, then t).
fn pack(row: &[RowVal], out: &mut Vec<f32>) {
    for v in row {
        match v {
            RowVal::F(x) => out.push(*x),
            RowVal::V(c) => out.extend(c),
            RowVal::U(u) => out.push(f32::from_bits(*u)),
            RowVal::I(i) => out.push(f32::from_bits(*i as u32)),
            RowVal::B(b) => out.push(if *b { 1.0 } else { 0.0 }),
            RowVal::X(x) => {
                for j in 0..3 {
                    for i in 0..3 {
                        out.push(x.r[i][j]);
                    }
                }
                out.extend(x.t);
            }
        }
    }
}

fn naga_module(name:&str,text:String,src:&Sources)->Res<ShaderModule>{
    let map=provenance::Map::new(text);
    let diagnostic=|at:Option<Span>,message:String|Diag::new(Code::E701,at.unwrap_or(Span::NONE),format!("WGSL validation in {name}: {message}"),"correct the WGSL expression at this source location");
    let module=naga::front::wgsl::parse_str(&map.shader).map_err(|e|diagnostic(e.labels().find_map(|(s,_)|map.origin(s)),e.emit_to_string(&map.shader)))?;
    naga::valid::Validator::new(naga::valid::ValidationFlags::all(),naga::valid::Capabilities::default()).validate(&module).map_err(|e|diagnostic(e.spans().find_map(|(s,_)|map.origin(*s)),e.emit_to_string(&map.shader)))?;
    let origins=map.expressions(&module,src);
    Ok(ShaderModule{name:name.into(),wgsl:map.shader,naga:module,span:None,origins})
}

/// The GPU passes in dependency order; `find_shadow_map` is optional, between the vis bundle and classification.
pub const GPU_ORDER: [&str; 6] = ["cull_indirect", "render_bundle", "find_shadow_map", "tile_classify", "shade_tiles", "tonemap_aces"];

/// Front end only: parse, index, check. For `loft check` and the error tests.
pub fn front(src: &Sources) -> Res<(Vec<Module>, Vec<Ty>)> {
    let mut modules = parse_all(src)?;
    let mut types = vec![];
    check_modules(&mut modules, &mut types)?;
    refine::check(&Prog::index(&modules)?, &types, &[])?;
    Ok((modules, types))
}

fn check_modules(modules: &mut [Module], types: &mut Vec<Ty>) -> Res<Option<Span>> {
    let mut effects = check::check(&Prog::index(modules)?, types)?;
    for item in modules.iter_mut().flat_map(|m| &mut m.items) {
        if let Item::Kernel(k) = item {
            k.writes = effects.kernels.remove(&k.f.name.name).expect("checked kernel");
        }
    }
    Ok(effects.pending_lowering)
}

pub fn compile(src: &Sources, overrides: &[(String, f64)]) -> Res<(Program, Report, Compiled)> {
    compile_cached(src, overrides, std::path::Path::new("target/loft_cache"))
}

pub fn compile_cached(src: &Sources, overrides: &[(String, f64)], cache: &std::path::Path) -> Res<(Program, Report, Compiled)> {
    compile_impl(src, overrides, &[], cache)
}

/// Re-derive local bounds and pack the sampled pose before drawing a requested
/// still. The render path never culls an animated row against its time-zero box.
pub fn compile_snapshot(src:&Sources,overrides:&[(String,f64)],samples:&[crate::program::MotionSample])->Res<(Program,Report,Compiled)> {
    compile_impl(src, overrides, samples, std::path::Path::new("target/loft_cache"))
}

fn compile_impl(src: &Sources, overrides: &[(String, f64)], samples: &[crate::program::MotionSample], cache: &std::path::Path) -> Res<(Program, Report, Compiled)> {
    let mut ms = vec![];
    let mut clock = Instant::now();
    let mut lap = |name: &'static str, ms: &mut Vec<(&'static str, f64)>| {
        ms.push((name, clock.elapsed().as_secs_f64() * 1e3));
        clock = Instant::now();
    };
    let mut modules = parse_all(src)?;
    let meshes = gltf::lower(src, &mut modules)?;
    for c in modules.iter_mut().flat_map(|m|&mut m.items).filter_map(|it|if let Item::Const(c)=it{Some(c)}else{None}){
        if c.comptime && let Some((_,v))=overrides.iter().find(|(n,_)|n==&c.name.name){
            let integer=matches!(&c.ty,Some(TyExpr::Name(n)) if n.name=="u32" || n.name=="i32");
            if !v.is_finite() || integer && (v.fract()!=0. || matches!(&c.ty,Some(TyExpr::Name(n)) if n.name=="u32") && !(0.0..=u32::MAX as f64).contains(v) || matches!(&c.ty,Some(TyExpr::Name(n)) if n.name=="i32") && !(i32::MIN as f64..=i32::MAX as f64).contains(v)){
                return Err(Diag::new(Code::E602,c.name.span,"invalid parameter override","supply a finite value in the parameter type's domain"))
            }
            c.value.kind=ExprKind::Num{value:*v,float:!integer,text:v.to_string(),suffix:NumSuffix::None};
        }
    }

    lap("parse+ad", &mut ms);
    let mut types = vec![];
    let pending_lowering = check_modules(&mut modules, &mut types)?;
    let p = Prog::index(&modules)?;
    for (name, _) in overrides {
        match p.consts.get(name.as_str()) {
            Some(c) if c.comptime => {}
            _ => {
                return Err(Diag::new(Code::E011, Span::NONE, format!("`--set {name}`: no scene parameter with this name"), "override a declared `param`"));
            }
        }
    }
    for c in p.consts.values(){
        if let Some(range)=&c.range{
            let mut ev:Eval<f32>=Eval::new(&p,&types,vec![],overrides);
            let provided=overrides.iter().find(|(n,_)|n==&c.name.name).map(|(_,v)|*v as f32);
            let x=match provided{Some(v)=>v,None=>as_f32(eval::concrete(&ev.expr(&c.value)?,c.value.span)?,c.value.span)?};
            ev.validate_range(&c.name,range,x,c.value.span)?;
        }
    }
    refine::check(&p,&types,overrides)?;
    if let Some(span) = pending_lowering {
        return Err(Diag::new(Code::E405, span, "Phase 4 lowering is pending for this checked program",
            "use the typed front end to inspect the Phase 4 contract"));
    }
    lap("check", &mut ms);

    // The schedule: stage-0 passes, the camera, and the GPU passes in dependency order.
    let sched = p.schedule.ok_or_else(|| Diag::new(Code::E520, Span::NONE, "the program has no `schedule`", "add `schedule Main { ... }` to the scene"))?;
    let mut rows_fns: Vec<&FnDecl> = modules.iter().flat_map(|m| &m.items).filter_map(|i| if let Item::Build(f)=i {Some(f)}else{None}).collect();
    let (mut camera, mut sun, mut light, mut sky) = (None, None, None, None);
    let mut passes = vec![];
    let mut drawn: Vec<&str> = vec![];
    let mut culled: Vec<&str> = vec![];
    let mut casters: Vec<&str> = vec![];
    let mut shadow_size = 0u32;
    let has_shadow_pass = sched.passes.iter().any(|p| p.verb.name == "find_shadow_map");
    let mut emitted_passes = HashSet::new();
    let mut available: HashSet<&str> = ["FrameUniform", "InputsBuf", "ParamsBuf", "RowTables", "LutTables", "BodyTables", "StateBuf", "PoseBuf"].into_iter().collect();
    for pd in &sched.passes {
        if let Some(resources) = &pd.resources {
            if let Some(read) = resources.reads.iter().find(|r| !available.contains(r.name.as_str())) {
                return Err(Diag::new(Code::E505, read.span, format!("{} read before it was written", read.name), "schedule its producer before this pass"));
            }
            // Generic compute/raster entry and bind-group lowering are Step 0 work.
            return Err(Diag::new(Code::E405, pd.label.span, "custom entry is outside this phase’s renderer", "use a supported schedule operation"));
        }
        let verb = pd.verb.name.as_str();
        if GPU_ORDER.contains(&verb) && !emitted_passes.insert(verb) {
            return Err(Diag::new(Code::E505, pd.verb.span, format!("duplicate render pass {verb}"), "schedule one producer per renderer resource"));
        }
        match verb {
            v if p.fns.get(v).is_some_and(|f| f.comptime) => {
                if !pd.args.is_empty() || !p.fns[v].params.is_empty() {
                    return Err(Diag::new(Code::E405, pd.verb.span, "a rows pass calls a `comptime fn` with no arguments", "read stage-0 constants inside the fn"));
                }
                rows_fns.push(p.fns[v]);
            }
            "run_laws" | "check" => {}, // lowered together in schedule order by motion_module
            "orbit" => {
                let f = |n: &str, types: &mut Vec<Ty>| -> Res<f32> {
                    let e = arg(pd, n)?;
                    as_f32(stage0(&p, types, e, overrides)?, e.span)
                };
                let at = arg(pd, "at")?;
                camera = Some(Orbit {
                    at: as_v3(stage0(&p, &mut types, at, overrides)?, at.span)?,
                    radius: f("radius", &mut types)?,
                    height: if pd.args.iter().any(|a|a.name.as_ref().is_some_and(|n|n.name=="height")){f("height",&mut types)?}else{0.5},
                    period: if pd.args.iter().any(|a|a.name.as_ref().is_some_and(|n|n.name=="period")){f("period",&mut types)?}else{24.0},
                    fov: f("fov", &mut types)?,
                    fixed_eye: None,
                });
            }
            "use_camera" => {
                let [Arg { value: Expr { kind: ExprKind::Name(name), .. }, .. }] = pd.args.as_slice() else {
                    return Err(Diag::new(Code::E520, pd.verb.span, "use_camera needs a declared camera", "write use_camera(CameraName)"));
                };
                let fields = &modules.iter().flat_map(|m| &m.items).find_map(|item| match item {
                    Item::Camera(c) if c.name.name == *name => Some(c),
                    _ => None,
                }).ok_or_else(|| Diag::new(Code::E520, pd.verb.span, format!("unknown camera {name}"), "declare the camera at module scope"))?.fields;
                let field = |names: &[&str]| fields.iter()
                    .find(|(n, _)| names.contains(&n.name.as_str()))
                    .map(|(_, e)| e)
                    .ok_or_else(|| Diag::new(Code::E520, pd.verb.span, format!("camera needs {}", names[0]), "supply the camera field"));
                let f = |e: &Expr, types: &mut Vec<Ty>| as_f32(stage0(&p, types, e, overrides)?, e.span);
                let point = |e: &Expr, types: &mut Vec<Ty>| as_v3(stage0(&p, types, e, overrides)?, e.span);
                let at = point(field(&["at", "target"])?, &mut types)?;
                let fov = f(field(&["fov"])?, &mut types)?;
                camera = Some(match field(&["eye"]) {
                    Ok(e) => Orbit { at, fov, fixed_eye: Some(point(e, &mut types)?), radius: 0.0, height: 0.0, period: 1.0 },
                    Err(_) => Orbit {
                        at, fov, fixed_eye: None,
                        radius: f(field(&["radius"])?, &mut types)?,
                        height: field(&["height"]).map_or(Ok(0.5), |e| f(e, &mut types))?,
                        period: field(&["period"]).map_or(Ok(24.0), |e| f(e, &mut types))?,
                    },
                });
            }
            "cull_indirect" => {
                culled = kind_args(&p, pd)?;
                passes.push(Pass::Cull);
            }
            "render_bundle" => {
                if !matches!(pd.args.first().map(|a| &a.value.kind), Some(ExprKind::Name(n)) if n == "Vis64") {
                    return Err(Diag::new(Code::E405, pd.verb.span, "the bundle draws into `Vis64` only", "write `render_bundle(Vis64) { draw_indirect(K); }`"));
                }
                for (call, args) in &pd.body {
                    if call.name != "draw_indirect" {
                        return Err(Diag::new(Code::E520, call.span, format!("`{}` in a render bundle", call.name), "write `draw_indirect(Kind, ...)`"));
                    }
                    let fake = PassDecl { label: pd.label.clone(), verb: call.clone(), args: args.iter().map(|a| Arg { name: None, value: clone_name(&a.value) }).collect(), body: vec![], resources: None };
                    for k in kind_args(&p, &fake)? {
                        if let Some(prm) = p.kinds[k].params.first() {
                            return Err(Diag::new(Code::E405, prm.span, "kind variants are not lowered yet", "move the parameter into the row for now"));
                        }
                        drawn.push(k);
                    }
                }
                passes.push(Pass::Vis((0..drawn.len() as u32).collect()));
            }
            "find_shadow_map" => {
                casters = kind_args(&p, pd)?;
                let e = arg(pd, "size")?;
                shadow_size = as_f32(stage0(&p, &mut types, e, overrides)?, e.span)? as u32;
                if !(64..=8192).contains(&shadow_size) || (!sched.implicit && casters.is_empty()) {
                    return Err(Diag::new(Code::E520, pd.verb.span, "a shadow map needs casters and a size of 64 to 8192 texels", "write `find_shadow_map(Kind, ..., size: 1024u)`"));
                }
                passes.push(Pass::Shadow(vec![]));
            }
            "tile_classify" => {
                let tile = arg(pd, "tile")?;
                if as_f32(stage0(&p, &mut types, tile, overrides)?, tile.span)? != abi::TILE as f32 {
                    return Err(Diag::new(Code::E405, tile.span, "tiles are 8x8", "write `tile: 8u`"));
                }
                passes.push(Pass::Classify);
            }
            "shade_tiles" => {
                light = Some(fn_name_arg(&p, pd, "light")?);
                let s = arg(pd, "sun")?;
                sun = Some(as_v3(stage0(&p, &mut types, s, overrides)?, s.span)?);
                passes.push(Pass::Shade(vec![]));
            }
            "tonemap_aces" => {
                sky = Some(fn_name_arg(&p, pd, "sky")?);
                passes.push(Pass::Present);
            }
            other => {
                return Err(Diag::new(
                    Code::E520,
                    pd.verb.span,
                    format!("unknown pass `{other}`"),
                    format!("passes: a `comptime fn` (rows), orbit, {}", GPU_ORDER.join(", ")),
                ));
            }
        }
        let (reads, writes): (&[&str], &[&str]) = match verb {
            "orbit" | "use_camera" => (&["FrameUniform"], &["CameraSlot"]),
            "run_laws" => (&["StateBuf", "RowTables"], &["PoseBuf"]),
            "check" => (&["RowTables", "PoseBuf"], &["VerdictBuf"]),
            "cull_indirect" => (&["CameraSlot"], &["IndirectBuf"]),
            "render_bundle" => (&["IndirectBuf"], &["Vis64", "DepthBuf"]),
            "find_shadow_map" => (&["IndirectBuf"], &["ShadowMap"]),
            "tile_classify" => (&["Vis64"], &["TileLists"]),
            "shade_tiles" if has_shadow_pass => (&["Vis64", "DepthBuf", "TileLists", "ShadowMap"], &["ColorHdr"]),
            "shade_tiles" => (&["Vis64", "DepthBuf", "TileLists"], &["ColorHdr"]),
            "tonemap_aces" => (&["ColorHdr"], &["Swapchain"]),
            _ => (&[], &[]),
        };
        if let Some(read) = reads.iter().find(|r| !available.contains(**r)) {
            return Err(Diag::new(Code::E505, pd.verb.span, format!("{read} read before it was written"), "schedule its producer before this pass"));
        }
        available.extend(writes);
    }
    let camera = camera.ok_or_else(|| Diag::new(Code::E520, sched.name.span, "no camera", "add `pass camera: orbit(at: ..., radius: ..., height: ..., period: ..., fov: ...);`"))?;
    let (light, sky, sun) = (light.expect("checked order"), sky.expect("checked order"), sun.expect("checked order"));
    lap("schedule", &mut ms);

    // Scans, baked in declaration order (a scan may sample one declared before it).
    let mut luts: Vec<eval::Lut> = vec![];
    for item in modules.iter().flat_map(|m| &m.items) {
        let Item::Scan(sc) = item else { continue };
        let lut = {
            let mut ev: Eval<f32> = Eval::new(&p, &types, vec![], overrides);
            ev.luts = &luts;
            let lo = as_f32(eval::concrete(&ev.expr(&sc.lo)?, sc.lo.span)?, sc.lo.span)?;
            let hi = as_f32(eval::concrete(&ev.expr(&sc.hi)?, sc.hi.span)?, sc.hi.span)?;
            let n = as_f32(eval::concrete(&ev.expr(&sc.steps)?, sc.steps.span)?, sc.steps.span)? as usize;
            if n < 4 || n > 65_536 || hi <= lo {
                return Err(Diag::new(Code::E602, sc.name.span, "a scan needs 4 to 65,536 steps over a range lo < hi", "fix `steps` or the range"));
            }
            let frame = |v: Val<f32>, span: Span| -> Res<[f32; 12]> {
                match v {
                    Val::V(c, _) if c.len() == 12 && c.iter().all(|x| x.is_finite()) => Ok(std::array::from_fn(|i| c[i])),
                    other => Err(Diag::new(Code::E701, span, format!("the scan produced {other:?}"), "keep the step's frame finite (a zero tangent cannot be normalised)")),
                }
            };
            ev.bind(&sc.var.name, Val::S(lo));
            let mut frames = vec![frame(ev.expr(&sc.init)?, sc.init.span)?];
            ev.unbind();
            for i in 1..n {
                let t = lo + (hi - lo) * i as f32 / (n - 1) as f32;
                ev.bind(&sc.prev.name, Val::V(frames[i - 1].to_vec(), 0.0));
                ev.bind(&sc.at.name, Val::S(t));
                frames.push(frame(ev.expr(&sc.step)?, sc.step.span)?);
                ev.unbind();
                ev.unbind();
            }
            eval::Lut { name: sc.name.name.clone(), lo, hi, frames }
        };
        luts.push(lut);
    }
    lap("scans", &mut ms);

    // Stage 0.
    let mut ev: Eval<f32> = Eval::new(&p, &types, vec![vec![]; p.kind_order.len()], overrides);
    ev.luts = &luts;
    for f in &rows_fns {
        ev.call_fn(f, vec![], f.name.span)?;
    }
    for sample in samples{
        let k=p.kinds.get(sample.kind.as_str()).ok_or_else(||Diag::new(Code::E602,Span::NONE,"snapshot names an unknown kind","sample the same scene revision"))?;
        let t=p.kind_index(&sample.kind);
        for (field,values) in &sample.fields{
            let col=k.row.iter().position(|f|f.name.name==*field).ok_or_else(||Diag::new(Code::E602,k.name.span,"snapshot field is absent","sample the current row layout"))?;
            if ev.tables[t].len()!=values.len(){return Err(Diag::new(Code::E602,k.name.span,"motion row count differs from build","sample the same scene and parameter values"))}
            for (i,x) in values.iter().enumerate(){
                if !x.is_finite(){return Err(Diag::new(Code::E602,k.row[col].name.span,"nonfinite motion result","guard the law expression"))}
                if let Some(range)=&k.row[col].range{ev.validate_range(&k.row[col].name,range,*x,k.row[col].name.span)?;}
                ev.tables[t][i][col]=RowVal::F(*x);
            }
        }
    }
    let authored_params = p.continuous_params().map(|(_, c)| as_f32(eval::concrete(&ev.module_expr(&c.value)?, c.name.span)?, c.name.span)).collect::<Res<Vec<_>>>()?;
    let mut environment_defaults = [[0.; 3]; 3];
    for (color, name) in environment_defaults.iter_mut().zip(defaults::ENV_DEFAULTS) {
        let decl = p.consts.get(name).ok_or_else(|| Diag::new(Code::E011, Span::NONE, format!("missing environment default {name}"), "load the light prelude"))?;
        *color = as_v3(eval::concrete(&ev.module_expr(&decl.value)?, decl.name.span)?, decl.name.span)?;
    }
    let tables = ev.tables;
    let environment=|kind:&str|->Res<Option<[[f32;3];2]>>{
        let Some(k)=p.kinds.get(kind) else{return Ok(None)};
        match tables[p.kind_index(kind)].as_slice(){
            []=>Ok(None),
            [row] if row.len()==2=>Ok(Some([as_v3(row[0].clone(),k.name.span)?,as_v3(row[1].clone(),k.name.span)?])),
            _=>Err(Diag::new(Code::E602,k.name.span,format!("expected one {} environment row",kind),"emit one environment for this view")),
        }
    };
    let sky_colors=environment("Sky")?;
    let sun_row=environment("Sun")?;
    let [default_sun, default_zenith, default_horizon] = environment_defaults;
    let [sun, sun_color] = sun_row.unwrap_or([sun, default_sun]);
    let [zenith, horizon] = sky_colors.unwrap_or([default_zenith, default_horizon]);
    let mut params: Vec<f32> = [sun_color, zenith, horizon].into_iter().zip([0., f32::from(sky_colors.is_some()), 0.])
        .flat_map(|([r, g, b], flag)| [r, g, b, flag]).collect();
    params.extend(authored_params.into_iter().flat_map(|x| [x, 0., 0., 0.]));
    lap("stage0", &mut ms);

    if sched.implicit {
        let populated = |k: &&str| !tables[p.kind_index(k)].is_empty();
        culled.retain(populated);
        drawn.retain(populated);
        casters.retain(populated);
        if casters.is_empty() {
            shadow_size = 0;
            passes.retain(|p| !matches!(p, Pass::Shadow(_)));
        }
    }
    let mut a = culled.clone();
    let mut b = drawn.clone();
    a.sort_unstable();
    b.sort_unstable();
    if a != b || b.windows(2).any(|pair| pair[0] == pair[1]) || drawn.len() > abi::MAX_KINDS {
        return Err(Diag::new(Code::E520, sched.name.span, format!("cull lists {culled:?} but the bundle draws {drawn:?}"), "cull each drawable kind once (up to 32)"));
    }
    let caster_ids = casters.iter().map(|c| {
        drawn.iter().position(|d| d == c).map(|i| i as u32).ok_or_else(|| Diag::new(Code::E520, sched.name.span, format!("shadow caster `{c}` is not drawn"), "cast shadows from drawn kinds"))
    }).collect::<Res<Vec<_>>>()?;
    let ids: Vec<u32> = (0..drawn.len() as u32).collect();
    for pass in &mut passes {
        match pass {
            Pass::Vis(k) | Pass::Shade(k) => *k = ids.clone(),
            Pass::Shadow(k) => *k = caster_ids.clone(),
            Pass::Cull | Pass::Classify | Pass::Present => {}
        }
    }

    // Derived bounds and L, once per distinct local shape: rows that differ only in pose (or in fields the geometry
    // never reads) share one derivation, so the count follows shapes, not instances (Phase 0 derived every row).
    let mut kinds = vec![];
    let mut instances = vec![];
    let mut reports = vec![];
    for (id, name) in drawn.iter().enumerate() {
        let k = p.kinds[*name];
        let t = p.kind_index(name);
        let rows = &tables[t];
        if rows.is_empty() || rows.len() as u32 >= abi::MAX_ROWS {
            return Err(Diag::new(Code::E602, k.name.span, format!("kind `{name}` is drawn with {} rows", rows.len()), "emit 1 to 16,777,215 rows from the rows pass"));
        }
        let mesh_kind = *name == "Mesh" && k.queries.is_empty();
        let mesh_asset = |row: &[RowVal]| {
            match row {
                [RowVal::X(_), RowVal::U(id), RowVal::U(0)] if (*id as usize) < meshes.len() => Ok(&meshes[*id as usize]),
                _ => Err(Diag::new(Code::E602, k.name.span, "invalid Mesh asset or LOD", "emit an imported mesh with LOD zero")),
            }
        };
        let key_fields = if mesh_kind { vec![1] } else { geometry_fields(k) };
        let mut shapes: Vec<usize> = vec![];
        let mut shape_of = vec![0usize; rows.len()];
        let mut keys: Vec<Vec<&RowVal>> = vec![];
        for (r, row) in rows.iter().enumerate() {
            let key: Vec<&RowVal> = key_fields.iter().filter_map(|i| row.get(*i)).collect();
            shape_of[r] = match keys.iter().position(|q| *q == key) {
                Some(s) => s,
                None => {
                    keys.push(key);
                    shapes.push(r);
                    keys.len() - 1
                }
            };
        }
        let surface = k.query(QueryKind::Surface);
        let trace = k.query(QueryKind::Trace);
        let domain=cache::kind_key(&p,k,&luts)?;
        let threads = std::thread::available_parallelism().map_or(1, |n| n.get()).min(shapes.len());
        let chunk = shapes.len().div_ceil(threads);
        let derived: Vec<lip::Derived> = if mesh_kind {
            for row in rows { mesh_asset(row)?; }
            shapes.iter().map(|&row| {
                let mesh = mesh_asset(&rows[row])?;
                // A planar triangle still has a nonempty conservative box for the culler.
                Ok(lip::Derived { lo: mesh.lo.map(|x| x.next_down().into()),
                    hi: mesh.hi.map(|x| x.next_up().into()), l: 0., evals: 0 })
            }).collect::<Res<_>>()?
        } else { std::thread::scope(|sc| {
            let jobs: Vec<_> = shapes.chunks(chunk)
                .map(|group| {
                    let (p, types, tables, luts, keys, shape_of, domain) = (&p, &types, &tables, &luts, &keys, &shape_of, &domain);
                    sc.spawn(move || {
                        let mut iv: Eval<Iv> = Eval::new(p, types, tables.clone(), overrides);
                        iv.luts = luts;
                        group
                            .iter()
                            .map(|&r| cache::derive(cache, domain, &keys[shape_of[r]], k.name.span, || match (surface, trace) {
                                (Some(s), _) => lip::derive_surface(&mut iv, s, t, r, k.name.span),
                                (None, Some(f)) => lip::derive(&mut iv, f, t, r, k.name.span),
                                (None, None) => unreachable!("checked: trace or surface"),
                            }))
                            .collect::<Res<Vec<_>>>()
                    })
                })
                .collect();
            jobs.into_iter().map(|h| h.join().expect("derive thread")).collect::<Res<Vec<Vec<_>>>>().map(|v| v.into_iter().flatten().collect())
        })? };
        let pose = pose_field(k);
        let (mut words, mut lips, mut evals) = (vec![], vec![], 0u64);
        for d in &derived {
            evals += u64::from(d.evals);
        }
        for (r, row) in rows.iter().enumerate() {
            let d = &derived[shape_of[r]];
            // World bound: the local bound's eight corners through the row's rigid pose.
            let (mut lo, mut hi) = ([f32::INFINITY; 3], [f32::NEG_INFINITY; 3]);
            let x = match pose.map(|i| &row[i]) {
                Some(RowVal::X(x)) => *x,
                _ => eval::Rigid::yaw([0.0; 3], 0.0),
            };
            for c in 0..8 {
                let q = [0, 1, 2].map(|i| if c >> i & 1 == 0 { d.lo[i] as f32 } else { d.hi[i] as f32 });
                let w = rot(&x, q);
                for i in 0..3 {
                    lo[i] = lo[i].min(w[i] + x.t[i]);
                    hi[i] = hi[i].max(w[i] + x.t[i]);
                }
            }
            pack(row, &mut words);
            words.extend(lo);
            words.extend(hi);
            words.push(d.l as f32);
            lips.push(d.l as f32);
            instances.push(Instance { lo, hi, kind: id as u32, row: r as u32 });
        }
        let draw = if mesh_kind {
            let mut vertices = 0;
            for row in rows {
                let n = u32::try_from(mesh_asset(row)?.triangles.len()).ok().and_then(|n| n.checked_mul(3))
                    .ok_or_else(|| Diag::new(Code::E602, k.name.span, "mesh vertex count exceeds u32", "split the mesh"))?;
                vertices = vertices.max(n);
            }
            Draw::Mesh { vertices }
        } else { match (surface, k.query(QueryKind::Visible)) {
            (Some(s), _) => {
                let g = s.attrs.iter().find(|a| a.name.name == "grid").expect("checked grid");
                let n = |e: &Expr| match e.kind {
                    ExprKind::Num { value, .. } => value as u32,
                    _ => unreachable!("checked literal"),
                };
                Draw::Surface { nu: n(&g.args[0]), nv: n(&g.args[1]) }
            }
            (None, Some(_)) => Draw::Analytic,
            (None, None) => Draw::Traced,
        } };
        reports.push(KindReport {
            name: name.to_string(),
            rows: rows.len(),
            shapes: shapes.len(),
            l_min: lips.iter().copied().fold(f32::INFINITY, f32::min).into(),
            l_max: lips.iter().copied().fold(0.0, f32::max).into(),
            evals,
            cache_hits: derived.iter().filter(|d| d.evals == 0).count(),
            draw,
        });
        let stride = (words.len() / rows.len()) as u32;
        kinds.push(KindTable { name: name.to_string(), module: id, stride, rows: words, n_rows: rows.len() as u32, lipschitz: lips, draw });
    }
    lap("derive", &mut ms);

    let mut lut_refs = vec![];
    let asset_words = gltf::words(&meshes);
    let mut off = asset_words.len().div_ceil(12) as u32;
    for l in &luts {
        lut_refs.push(emit::LutRef { name: l.name.clone(), offset: off, n: l.frames.len() as u32, lo: l.lo, hi: l.hi });
        off += l.frames.len() as u32;
    }
    let mut texts = Vec::new();
    for (name, kt) in drawn.iter().zip(&kinds) {
        let shader = emit::kind_module(&p, &types, p.kinds[*name], light, &lut_refs, kt.draw)?;
        texts.push((name.to_string(), shader));
    }
    let mut dispatches = vec![];
    if samples.is_empty() {
        for &name in &p.kind_order {
            use crate::program::DispatchRows;
            let kind = p.kinds[name];
            let draw = drawn.iter().position(|&n| n == name);
            let table = &tables[p.kind_index(name)];
            let output = || {
                if let Some(draw) = draw {
                    return Ok((draw, DispatchRows::Drawable(draw)));
                }
                if table.is_empty() {
                    return Err(Diag::new(Code::E602, kind.name.span, "scheduled table has no rows", "emit a row or remove its scheduled operation"));
                }
                let mut words = vec![];
                for row in table {
                    pack(row, &mut words);
                    words.extend([0.; abi::HIDDEN_WORDS as usize]);
                }
                Ok((texts.len(), DispatchRows::Table {
                    name: name.into(), n_rows: table.len() as u32, stride: (words.len() / table.len()) as u32, words,
                }))
            };
            if let Some((entry, dispatch)) = emit::motion_module(&p, &types, kind, output)? {
                if let Some(draw) = draw {
                    texts[draw].1.push_str(&entry);
                } else {
                    let mut kernel = emit::table_module(&p, &types, kind);
                    kernel.push_str(&entry);
                    texts.push((name.into(), kernel));
                }
                dispatches.push(dispatch);
            }
        }
    }
    let shared = texts.len();
    texts.push(("cull".into(), emit::cull_module()));
    texts.push(("classify".into(), emit::classify_module()));
    texts.push(("present".into(), emit::present_module(&p, &types, sky)));
    lap("emit", &mut ms);
    let shader_modules = texts.into_iter().map(|(n,w)| {
        let mut m=naga_module(&n,w,src)?;
        if let Some(k)=p.kinds.get(n.as_str()) {let (file,line,col,_,_)=src.locate(k.name.span); m.span=Some((file,line,col));}
        Ok(m)
    }).collect::<Res<Vec<_>>>()?;
    lap("naga", &mut ms);

    let program = Program {
        dispatches,
        modules: shader_modules,
        kinds,
        instances,
        camera,
        sun, params,
        shadow_size,
        luts: {
            let mut words = asset_words;
            words.resize(words.len().next_multiple_of(12), 0.);
            words.extend(luts.iter().flat_map(|l| l.frames.iter().flatten().copied()));
            words
        },
        passes,
        cull: shared,
        classify: shared + 1,
        present: shared + 2,
    };
    let drawn_owned = drawn.iter().map(|d| (p.kind_index(d), d.to_string())).collect();
    drop(p);
    Ok((program, Report { kinds: reports, ms }, Compiled { modules, types, tables, luts, drawn: drawn_owned, meshes }))
}

/// A name expression, copied (bundle bodies are re-read as kind lists).
fn clone_name(e: &Expr) -> Expr {
    let kind = match &e.kind {
        ExprKind::Name(n) => ExprKind::Name(n.clone()),
        ExprKind::Unproved { .. } | ExprKind::FiniteDiff { .. } | ExprKind::If { .. } | ExprKind::Num { .. } | ExprKind::Bool(_) | ExprKind::String(_) | ExprKind::Member(..) | ExprKind::Index(..) | ExprKind::Call { .. } | ExprKind::Ctor { .. } | ExprKind::Binary(..) | ExprKind::Unary(..) => ExprKind::Bool(false),
    };
    Expr { id: e.id, span: e.span, kind }
}

pub mod program;
pub mod sexp;
pub mod tier0;
pub mod telemetry;
pub mod fusion;
pub mod world;
pub mod bvh;
pub mod bsdf;
pub mod quant;
pub mod bake_tex;
pub mod gltf;
pub mod bundle;
pub mod plan;
