//! The checker (draft §2A, §2B, §3): affine spaces (`Pt - Pt : Vec`, `Pt + Pt` is E104, a transform applies only in
//! its source space), generative map spaces with `.cell`/`.to_parent`/`.from_parent`, query signatures, and staging:
//! E401/E402 on kind parameters, E403 stage-0 statements outside `comptime fn`, E404 kind code reading stage 0.
//! Output: the type of every expression, by id.

#[path = "kernel_units.rs"]
mod kernel_units;
#[path = "motion_source.rs"]
pub mod motion_source;
use crate::Prog;
use loft_builtins::{ArgShape, Builtin, BuiltinSpec, PolyRule, RetShape, SigRule, lookup};
use loft_parse::ast::*;
use loft_ir::{Code, Diag, Res, Span};
use std::collections::{BTreeMap, HashMap};

#[derive(Clone)]
struct Local {
    ty: Ty,
    mutable: bool,
    /// For a `map` name: the parent space and whether a mirror stage gives it a `.cell`.
    map: Option<(Space, bool)>,
    span: Span,
}

pub struct Ck<'p, 'a> {
    pub(crate) hints: Option<&'p mut crate::infer::Hints>,
    kernel: Option<&'a KernelDecl>,
    atomic_writes: BTreeMap<String, Determinism>,
    pending_lowering: Option<Span>,
    local_units: HashMap<Span, Dimension>,
    expr_units: HashMap<u32, Dimension>,
    ownership: Option<(String,Vec<String>)>,
    p: &'p Prog<'a>,
    types: &'p mut Vec<Ty>,
    scopes: Vec<HashMap<String, Local>>,
    /// The fn being checked is stage 0.
    comptime: bool,
    ret: Ty,
    fresh: u32,
    /// Space variables of the fn being checked (`'s` in its signature).
    sig_vars: Vec<String>,
}

/// Values a kind's parameters may jointly take (spec §2.4: "cardinality ≤ 8"). A scene compiles
/// the one variant its rows use; stage 0 rejects rows that disagree.
pub const VARIANT_BUDGET: u64 = 8;

pub struct CheckedEffects {
    pub kernels: BTreeMap<String, BTreeMap<String, Determinism>>,
    pub pending_lowering: Option<Span>,
}

pub fn check(p: &Prog, types: &mut Vec<Ty>) -> Res<CheckedEffects> {
    let mut ck = Ck::new(p,types);
    for (name, target) in p.aliases.values() {
        ck.resolve_named(name, true, &mut vec![])?;
        ck.resolve(target, true)?;
    }
    for (name, fields) in p.structs.values() {
        ck.resolve_named(name, true, &mut vec![])?;
        ck.comptime = true;
        ck.scopes = vec![HashMap::new()];
        let mut seen = std::collections::HashSet::new();
        for field in *fields {
            if !seen.insert(&field.name.name) {
                return Err(Diag::new(Code::E011, field.name.span, "duplicate struct column", "give each column a distinct name"));
            }
            let t = ck.resolve(&field.ty, true)?;
            if let Some(range) = &field.range { ck.range(range)?; }
            if let Some(value) = &field.default {
                let from = ck.expr(value)?;
                ck.assignable(&from, &t, value.span)?;
            }
        }
    }
    for c in p.consts.values() {
        ck.comptime = c.comptime;
        ck.scopes = vec![HashMap::new()];
        let t = ck.expr(&c.value)?;
        if let Some(r)=&c.range {ck.range(r)?;}
        if let Some(ty) = &c.ty {
            let want = ck.resolve(ty, true)?;
            ck.assignable(&t, &want, c.value.span)?; if t==Ty::AbstractInt {ck.set(&c.value,want);}
        }
    }
    motion_source::check(&mut ck)?;
    for sc in p.scans.values() { ck.scan(sc)?; }
    for k in p.kinds.values() { ck.kind_params(k)?; }
    for f in p.fns.values() { ck.func(f, None)?; }
    for k in p.kinds.values() {
        ck.kind(k)?;
    }
    let mut owners=HashMap::new();
    let mut tiers=HashMap::new();
    for s in motion_schedule(p)? {
        let (Some(law), Some(k)) = (s.law, s.target) else { continue };
        if let Some((tier, first)) = tiers.insert(&k.name, (law.tier, law.f.name.span))
            && tier != law.tier
        {
            return Err(Diag::new(Code::E504, law.f.name.span, "Det and NonDet laws share a row table",
                "schedule the NonDet laws on their own kind").note(first, "other tier scheduled here"));
        }
        for f in &law.writes{if let Some(prev)=owners.insert((&k.name,&f.name),f.span){return Err(Diag::new(Code::E502,f.span,format!("conflicting writes on {}.{}",k.name,f.name),"give one scheduled law ownership of this field").note(prev,"first writer declared here"));}}
    }
    for law in p.laws.values().filter(|law| law.tier.is_det()) {
        let (reachable, constants) = crate::references::reachable(p, &[&law.f], &[]);
        let mut calls = crate::references::References::default();
        for name in constants {
            calls.expr(&p.consts[name.as_str()].value, &[]);
        }
        for f in std::iter::once(&law.f).chain(reachable) {
            if f.refuses_unverified_raw() {
                return Err(Diag::new(Code::E504, f.name.span, "motion reaches unverified raw WGSL", "use wgsl det (...) after verifying the block is deterministic"));
            }
            calls.calls.extend(crate::references::References::function(f).calls);
        }
        if let Some(call) = calls.calls.into_iter().find(|c| !p.fns.contains_key(c.name.as_str())
            && matches!(crate::references::motion_math(&c.name), crate::references::MotionMath::Unavailable)) {
            return Err(Diag::new(Code::E504, call.span, "no deterministic motion implementation for this operation", "rewrite it using deterministic operations"));
        }
    }
    for k in p.kinds.values(){if let Some(trace)=k.query(QueryKind::Trace) && let Some(f)=crate::references::proof_dependencies(p,trace).into_iter().find(|f|f.raw.is_some()){
        return Err(Diag::new(Code::E211,f.name.span,"raw WGSL has no proven spatial Lipschitz bound","write the trace term in Loft or move it to the material"))
    }}
    let mut effects = BTreeMap::new();
    for k in p.kernels.values() {
        effects.insert(k.f.name.name.clone(), ck.kernel(k)?);
    }
    for law in p.laws.values(){ck.motion(&law.f,Some(&law.writes))?;}
    for f in p.checks.values(){ck.motion(f,None)?;}
    Ok(CheckedEffects { kernels: effects, pending_lowering: ck.pending_lowering })
}

/// Types one stage-0 expression (a schedule argument): it may read `comptime const`s and call any fn.
pub fn check_stage0_expr(p: &Prog, types: &mut Vec<Ty>, e: &Expr) -> Res<Ty> {
    let mut ck = Ck::new(p, types);
    ck.scopes.push(HashMap::new());
    ck.comptime = true;
    ck.expr(e)
}

fn runtime_read<'a>(p: &Prog<'a>, mut refs: crate::references::References<'a>) -> Option<Span> {
    let mut visited = std::collections::HashSet::new();
    let mut next = vec![];
    loop {
        for (name, span) in refs.names {
            if name == "time" { return Some(span); }
            if let Some(c) = p.consts.get(name) && visited.insert((false, name)) {
                let mut child = crate::references::References::default();
                child.expr(&c.value, &[]);
                next.push(child);
            }
        }
        for call in refs.calls {
            let name = call.name.as_str();
            if let Some(f) = p.fns.get(name) {
                if visited.insert((true, name)) { next.push(crate::references::References::function(f)); }
            } else if matches!(name, "sample" | "sample_lod") {
                return Some(call.span);
            }
        }
        refs = next.pop()?;
    }
}

fn stage_error(site: Span, read: Span) -> Diag {
    Diag::new(Code::E403, site, "build depends on runtime time", "move this calculation into a query or a law")
        .note(read, "runtime time is read here")
}

fn swizzle_len(m: &str, max: usize) -> Option<usize> {
    let ok = m.len() <= 4
        && (m.chars().all(|c| "xyzw"[..max].contains(c)) || m.chars().all(|c| "rgba"[..max].contains(c)));
    ok.then_some(m.len())
}

pub fn vec_like(g: Geo) -> bool {
    matches!(g, Geo::Vec | Geo::Dir | Geo::Norm)
}

fn e104(span: Span, msg: impl Into<String>, fix: impl Into<String>) -> Diag {
    Diag::new(Code::E104, span, msg, fix)
}
/// A scheduled row law or check, in schedule order.
pub struct Scheduled<'a> {
    pub f: &'a FnDecl,
    /// Absent for a check.
    pub law: Option<&'a LawDecl>,
    /// The kind named by the first parameter's type.
    pub target: Option<&'a Ident>,
    /// The schedule argument.
    pub span: Span,
}

/// The one reading of `run_laws` and `check` passes: the checker enforces tiers
/// and write ownership on it, and motion lowering selects each kind's entries.
pub fn motion_schedule<'a>(p: &Prog<'a>) -> Res<Vec<Scheduled<'a>>> {
    let mut out = vec![];
    for pass in p.schedule.into_iter().flat_map(|s| &s.passes) {
        let is_law = match pass.verb.name.as_str() {
            "run_laws" => true,
            "check" => false,
            _ => continue,
        };
        for a in &pass.args {
            let span = a.value.span;
            let ExprKind::Name(n) = &a.value.kind else {
                // Each verb keeps the code it had when laws were read here and checks in lowering.
                return Err(if is_law { e003(span, "expected a law name", "name a declared law") }
                    else { Diag::new(Code::E405, span, "expected a scheduled law or check name", "use a row-local operation on a kind") });
            };
            let law = if is_law {
                Some(*p.laws.get(n.as_str()).ok_or_else(|| Diag::new(Code::E011, span, "unknown scheduled law", "declare the law before scheduling it"))?)
            } else { None };
            let f = match law {
                Some(law) => &law.f,
                None => *p.checks.get(n.as_str()).ok_or_else(|| Diag::new(Code::E011, span, "unknown scheduled law or check", "declare the named function"))?,
            };
            let target = if let Some(TyExpr::Name(k)) = f.params.first().and_then(|prm| prm.ty.as_ref()) { Some(k) } else { None };
            out.push(Scheduled { f, law, target, span });
        }
    }
    Ok(out)
}

fn e003(span: Span, msg: impl Into<String>, fix: impl Into<String>) -> Diag {
    Diag::new(Code::E003, span, msg, fix)
}

impl<'p,'a> Ck<'p,'a> {
    pub fn new(p:&'p Prog<'a>, types:&'p mut Vec<Ty>)->Self {
        Self{p,types,scopes:vec![],comptime:false,ret:Ty::Void,fresh:0,sig_vars:vec![],hints:None,ownership:None,kernel:None,atomic_writes:BTreeMap::new(),pending_lowering:None,local_units:HashMap::new(),expr_units:HashMap::new()}
    }

    fn lookup(&self, n: &str) -> Option<&Local> {
        self.scopes.iter().rev().find_map(|s| s.get(n))
    }
    fn bind(&mut self, n: &str, l: Local) {
        self.scopes.last_mut().expect("a scope").insert(n.into(), l);
    }
    fn set(&mut self, e: &Expr, t: Ty) -> Ty {
        let i = e.id as usize;
        if self.types.len() <= i {
            self.types.resize(i + 1, Ty::Void);
        }
        self.types[i] = t.clone();
        t
    }

    /// Resolves written type syntax. `sig`: a `'s` is a signature variable; otherwise it names a map in scope.
    pub fn resolve(&self, t: &TyExpr, sig: bool) -> Res<Ty> {
        self.resolve_type(t, sig, &mut vec![])
    }

    fn resolve_named(&self, name: &Ident, sig: bool, stack: &mut Vec<String>) -> Res<Ty> {
        if stack.contains(&name.name) {
            return Err(Diag::new(Code::E011, name.span, "cyclic type declaration", "use a finite, nonrecursive payload type"));
        }
        stack.push(name.name.clone());
        let result = if let Some((_, alias)) = self.p.aliases.get(name.name.as_str()) {
            self.resolve_type(alias, sig, stack)
        } else {
            for field in self.p.structs[name.name.as_str()].1 {
                self.resolve_type(&field.ty, sig, stack)?;
            }
            Ok(Ty::Struct(name.name.clone()))
        };
        stack.pop();
        result
    }

    fn resolve_type(&self, t: &TyExpr, sig: bool, stack: &mut Vec<String>) -> Res<Ty> {
        let space = |s: &SpaceRef| -> Res<Space> {
            if s.var {
                if sig {
                    return Ok(Space::Var(s.name.clone()));
                }
                return match self.lookup(&s.name).and_then(|l| l.map.as_ref().map(|_| l.ty.clone())) {
                    Some(Ty::Geo(_, sp)) => Ok(sp),
                    // In a generic fn, `'s` is the signature's space variable.
                    _ if self.sig_vars.contains(&s.name) => Ok(Space::Var(s.name.clone())),
                    _ => Err(Diag::new(
                        Code::E011,
                        s.span,
                        format!("no map `{}` in scope for the space `'{}`", s.name, s.name),
                        format!("introduce it with `map {} = ...;` before this line", s.name),
                    )),
                };
            }
            if matches!(s.name.as_str(), "World" | "Local") || self.comptime && !self.p.bodies.is_empty() && s.name == "Body" || self.p.spaces.contains(s.name.as_str()) {
                Ok(Space::Named(s.name.clone()))
            } else {
                Err(Diag::new(
                    Code::E011,
                    s.span,
                    format!("unknown space `{}`", s.name),
                    format!("declare it: `space {};`", s.name),
                ))
            }
        };
        match t {
            TyExpr::Name(i) => {
                let n = i.name.as_str();
                if let Some((_, ty)) = CORE_TYPES.iter().find(|(name, _)| *name == n) {
                    return Ok(ty.clone());
                }
                match n {
                    n if self.p.aliases.contains_key(n) || self.p.structs.contains_key(n) => self.resolve_named(i, sig, stack),
                    RADIANCE => Ok(Ty::VecN(3)),
                    n if self.p.kinds.contains_key(n) => Ok(Ty::Id(n.into())),
                    n if self.p.units.contains(n) => Ok(Ty::F32),
                    n => Err(Diag::new(
                        Code::E011,
                        i.span,
                        format!("unknown type `{n}`"),
                        "use f, u, i, bool, a declared type, p3@Space or xf@A>B",
                    )),
                }
            }
            TyExpr::Generic(i, args) => match (i.name.as_str(), args.as_slice()) {
                ("vec2" | "vec3" | "vec4", [TyExpr::Name(a)]) if a.name == "f32" => {
                    Ok(Ty::VecN(i.name.as_bytes()[3] - b'0'))
                }
                _ => Err(Diag::new(
                    Code::E405,
                    i.span,
                    format!("Phase 0 has no `{}<..>` of this element type", i.name),
                    "use vec2<f32>, vec3<f32> or vec4<f32>",
                )),
            },
            TyExpr::Array(element, count, span) => {
                if *count == 0 { return Err(e003(*span, "array length must be positive", "use a positive count")); }
                Ok(Ty::Array(Box::new(self.resolve_type(element, sig, stack)?), *count))
            }
            TyExpr::Geo(g, s) => Ok(Ty::Geo(*g, space(s)?)),
            TyExpr::Xform(a, b) => Ok(Ty::Xform(space(a)?, space(b)?)),
            TyExpr::Field(s) => space(s).map(|_| Ty::F32),
            TyExpr::Frame(s) => Ok(Ty::Frame(space(s)?)),
            TyExpr::Table(n) if self.p.kinds.contains_key(n.name.as_str()) || self.p.structs.contains_key(n.name.as_str()) => Ok(Ty::Table(n.name.clone())),
            TyExpr::Table(n) => Err(Diag::new(Code::E011, n.span, "unknown table row type", "declare the kind or struct")),
        }
    }

    /// A scan is stage 0: its init and step run once at load.
    pub(crate) fn scan(&mut self, sc: &ScanDecl) -> Res<()> {
        self.comptime = true;
        self.scopes = vec![HashMap::new()];
        let vt = self.resolve(&sc.var_ty, true)?;
        if vt != Ty::F32 {
            return Err(e003(sc.var_ty.span(), "a scan runs over an f32 parameter", "write `t: f32 in lo..hi`"));
        }
        for e in [&sc.lo, &sc.hi] {
            let t = self.expr(e)?;
            self.assignable(&t, &Ty::F32, e.span)?;
        }
        let st = self.expr(&sc.steps)?;
        if !matches!(sc.steps.kind, ExprKind::Num { .. }) || !matches!(st, Ty::U32 | Ty::AbstractInt) {
            return Err(e003(sc.steps.span, "a scan's step count is a literal", "write `steps: 256u`"));
        }
        self.bind(&sc.var.name, Local { ty: vt.clone(), mutable: false, map: None, span: sc.var.span });
        let elem = self.expr(&sc.init)?;
        if !matches!(elem, Ty::Frame(_)) {
            return Err(Diag::new(Code::E405, sc.init.span, "Phase 1 scans produce `Frame[S]` tables", "write `-> Table[Frame[S]]`"));
        }
        self.bind(&sc.var.name, Local { ty: vt.clone(), mutable: false, map: None, span: sc.var.span });
        let t = self.expr(&sc.init)?;
        self.assignable(&t, &elem, sc.init.span)?;
        self.scopes.push(HashMap::new());
        self.bind(&sc.prev.name, Local { ty: elem.clone(), mutable: false, map: None, span: sc.prev.span });
        self.bind(&sc.at.name, Local { ty: vt, mutable: false, map: None, span: sc.at.span });
        let t = self.expr(&sc.step)?;
        self.assignable(&t, &elem, sc.step.span)?;
        Ok(())
    }

    fn contextual(&mut self,e:&Expr,from:&Ty,to:&Ty)->bool{
        if matches!(from,Ty::VecN(3)) && matches!(to,Ty::Geo(_,s) if !matches!(s,Space::Var(_))) && matches!(&e.kind,ExprKind::Ctor{ty:TyExpr::Name(n),..} if n.name=="vec3f"){
            self.set(e,to.clone());return true;
        }
        false
    }
    fn assignable(&self, from: &Ty, to: &Ty, span: Span) -> Res<()> {
        let ok = from == to
            || matches!((from, to), (Ty::AbstractInt, Ty::F32 | Ty::U32 | Ty::I32))
            || matches!((from,to),(Ty::Xform(Space::Open,b),Ty::Xform(_,c)) if b==c)
            || matches!((from, to), (Ty::Geo(g, a), Ty::Geo(Geo::Vec, b)) if vec_like(*g) && a == b)
            // A space-less vec3 parameter (`rgb`, a helper like `vnoise`) takes a spaced value as plain
            // coordinates: the implicit form of `coords(v)`, which lowers to nothing. Fresh crab programs failed
            // here on `vnoise(p - at)` and `helper(n)`. Mixing two different spaces is still refused.
            || matches!((from, to), (Ty::Geo(Geo::Pt | Geo::Vec | Geo::Dir | Geo::Norm, _), Ty::VecN(3)));
        if ok {
            return Ok(());
        }
        Err(match (from, to) {
            (Ty::Geo(Geo::Attach,_), Ty::Geo(Geo::Target,_)) | (Ty::Geo(Geo::Target,_), Ty::Geo(Geo::Attach,_)) =>
                Diag::new(Code::E105, span, format!("expected {to}, found {from}"), "supply an independent goal or an attached anchor of the declared type"),
            (Ty::Geo(_, a), Ty::Geo(_, b)) if a != b => e104(
                span,
                format!("space mismatch: expected `{to}`, found `{from}`"),
                format!("move the value into `{b}` with a transform or a map's `to_parent`/`from_parent`"),
            ),
            // The old hint offered `[x, y, z]@S`, which is a point, for every geometric kind.
            (Ty::VecN(3), Ty::Geo(g, s)) => e104(
                span,
                format!("a space-less vec3 where `{to}` is expected"),
                match g {
                    Geo::Pt => format!("write the point as `[x, y, z]@{s}`"),
                    _ => format!("derive it from points in `{s}` (`p - at`), or add the space-less offset to one (`p - at - [x, y, z]`)"),
                },
            ),
            // `coords` is the explicit way to drop a space; the hint used to say only "convert".
            (Ty::Geo(..), Ty::VecN(3)) => e003(span, format!("expected `{to}`, found `{from}`"),
                "drop the space explicitly with `coords(value)`"),
            // Name the conversion call; "convert the value to `i`" left models guessing.
            (Ty::F32 | Ty::U32 | Ty::I32, Ty::F32 | Ty::U32 | Ty::I32) => {
                let call = match to { Ty::F32 => "f32", Ty::U32 => "u32", _ => "i32" };
                e003(span, format!("expected `{to}`, found `{from}`"), format!("convert with `{call}(value)`"))
            }
            _ => e003(span, format!("expected `{to}`, found `{from}`"), format!("convert the value to `{to}`")),
        })
    }

    pub fn func(&mut self, f: &FnDecl, kind: Option<&str>) -> Res<()> {
        self.comptime = f.comptime;
        if let Some(h)=&mut self.hints {h.current=if f.ret.is_none(){Some(f.name.name.clone())}else{None};}
        self.scopes = vec![HashMap::new()];
        self.sig_vars.clear();
        let mut note = |t: &TyExpr| match t {
            TyExpr::Geo(_, s) | TyExpr::Field(s) | TyExpr::Frame(s) if s.var => self.sig_vars.push(s.name.clone()),
            TyExpr::Name(_) | TyExpr::Generic(..) | TyExpr::Array(..) | TyExpr::Geo(..) | TyExpr::Field(_) | TyExpr::Frame(_) | TyExpr::Table(_) | TyExpr::Xform(..) => {}
        };
        f.params.iter().filter_map(|p| p.ty.as_ref()).chain(f.ret.as_ref()).for_each(&mut note);
        for prm in &f.params {
            let ty = match (&prm.ty, kind) {
                (Some(t), _) => self.resolve(t, true)?,
                (None, Some(k)) if prm.name.name=="self" => Ty::Row(k.into()),
                (None, _) => {
                    return Err(e003(prm.name.span, "parameter type could not be inferred", "annotate the parameter or call this helper with typed arguments"));
                }
            };
            let ty=if let Ty::Id(k)=ty {Ty::Row(k)}else{ty};
            self.bind(&prm.name.name, Local { ty, mutable: false, map: None, span: prm.name.span });
        }
        self.ret = match &f.ret {
            Some(t) => self.resolve(t, true)?,
            None => Ty::Void,
        };
        for a in &f.attrs {
            self.attr(a, f, kind.is_some())?;
        }
        self.block(&f.body)
    }

    pub(crate) fn kernel(&mut self, k: &'a KernelDecl) -> Res<BTreeMap<String, Determinism>> {
        self.local_units.clear();
        self.expr_units.clear();
        for p in &k.f.params {
            if let Some(t) = &p.ty {
                self.resolve(t, true)?;
                self.local_units.insert(p.name.span, self.declared_unit(t));
            }
        }
        self.kernel = Some(k);
        self.atomic_writes.clear();
        let result = self.func(&k.f, None);
        self.kernel = None;
        result.map(|()| std::mem::take(&mut self.atomic_writes))
    }

    fn require(&self, capability: KernelCapability, span: Span) -> Res<()> {
        if self.kernel.is_some_and(|k| k.allow.contains(&capability)) { return Ok(()); }
        Err(Diag::new(Code::E504, span, format!("operation requires kernel permission {capability:?}"),
            "declare the capability in the kernel's allow list"))
    }

    pub(crate) fn motion(&mut self,f:&FnDecl,writes:Option<&[Ident]>)->Res<()>{
        let mut checked=f.clone();
        let Some(first)=checked.params.first() else{return Err(e003(f.name.span,"a law or check needs its target row","give the first parameter its kind type"))};
        let Some(TyExpr::Name(k))=&first.ty else{return Err(e003(first.name.span,"missing target kind","annotate the first parameter with its kind"))};
        let kind=self.p.kinds.get(k.name.as_str()).ok_or_else(||Diag::new(Code::E011,k.span,"unknown target kind","declare this kind first"))?;
        for field in writes.into_iter().flatten(){if !kind.row.iter().any(|f|f.name.name==field.name){return Err(Diag::new(Code::E501,field.span,"writes names an unknown column","use a field of the target kind"))}}
        // The GPU modules fold a kind parameter to the value its rows were emitted with.
        for field in writes.into_iter().flatten(){if kind.params.iter().any(|prm|prm.name.name==field.name){return Err(Diag::new(Code::E501,field.span,format!("`{}` is a kind parameter, fixed when the row is emitted",field.name),"move it into the kind's column list to change it at run time"))}}
        self.ownership=writes.map(|w|(first.name.name.clone(),w.iter().map(|i|i.name.clone()).collect()));
        for p in checked.params.iter_mut().skip(1){
            if p.name.name=="dt" && p.ty.is_none(){p.ty=Some(TyExpr::Name(Ident{name:"f32".into(),span:p.name.span}));}
            if matches!(&p.ty,Some(TyExpr::Name(n)) if self.p.kinds.contains_key(n.name.as_str())) {return Err(Diag::new(Code::E108,p.name.span,"read-only rows require a table parameter","write [Kind] instead of Kind"));}
        }
        if writes.is_none(){checked.ret=Some(TyExpr::Name(Ident{name:"Verdict".into(),span:f.name.span}));}
        let result=self.func(&checked,None);self.ownership=None;result
    }

    /// `@lipschitz(param)` or `@lipschitz(max(a, b))`, naming the fn's own parameters; `@lobe` on a library fn
    /// ending in `n, v, l: vec3<f32>` and returning `vec4<f32>`; `@grid(nu, nv)` on a surface query.
    fn attr(&self, a: &Attr, f: &FnDecl, in_kind: bool) -> Res<()> {
        let is_param = |e: &Expr| matches!(&e.kind, ExprKind::Name(n) if f.params.iter().any(|p| &p.name.name == n));
        if a.name.name == "lobe" {
            let tail: Vec<Ty> = f.params.iter().rev().take(3).filter_map(|p| p.ty.as_ref().map(|t| self.resolve(t, true))).collect::<Res<_>>()?;
            let ret = f.ret.as_ref().map(|t| self.resolve(t, true)).transpose()?;
            if in_kind || !a.args.is_empty() || tail != [Ty::VecN(3), Ty::VecN(3), Ty::VecN(3)] || ret != Some(Ty::VecN(4)) {
                return Err(Diag::new(Code::E003, a.name.span, "a lobe is a fn `(params..., n, v, l: vec3<f32>) -> vec4<f32>`", "end the parameters with `n: vec3<f32>, v: vec3<f32>, l: vec3<f32>` and return (radiance factor, transmittance below)"));
            }
            return Ok(());
        }
        if a.name.name == "grid" {
            let lit = |e: &Expr| matches!(e.kind, ExprKind::Num { value, .. } if value >= 1.0 && value <= 4096.0);
            if in_kind && a.args.len() == 2 && a.args.iter().all(lit) {
                return Ok(());
            }
            return Err(Diag::new(Code::E003, a.name.span, "`@grid(nu, nv)` takes two literal cell counts on a `query surface`", "write `@grid(192u, 32u)`"));
        }
        let ok = a.name.name == "lipschitz"
            && match a.args.as_slice() {
                [e] if is_param(e) => true,
                [Expr { kind: ExprKind::Call { callee, args }, .. }] => {
                    callee.name == "max" && args.len() == 2 && args.iter().all(|x| is_param(&x.value))
                }
                _ => false,
            };
        if ok {
            Ok(())
        } else {
            Err(Diag::new(
                Code::E405,
                a.name.span,
                "attributes are `@lipschitz(p)`, `@lipschitz(max(a, b))`, `@lobe` and `@grid(nu, nv)`",
                "remove the attribute, or name the parameter(s) the result is 1-Lipschitz in",
            ))
        }
    }

    fn range(&mut self,r:&RowRange)->Res<()>{
        for e in std::iter::once(&r.lo).chain(r.hi.as_ref()){
            let t=self.expr(e)?;
            if !num(&t){return Err(e003(e.span,"range endpoint must be a number","use a scalar build-time expression"))}
        }
        Ok(())
    }
    /// A kind's parameters, before any `emit` is checked: a parameter is also a column, so an
    /// invalid one must be refused as a parameter rather than as a missing column.
    fn kind_params(&mut self, k: &KindDecl) -> Res<()> {
        let mut variants: u64 = 1;
        for prm in &k.params {
            let t = self.resolve(&prm.ty, true)?;
            let n = match (&t, &prm.range) {
                (Ty::Bool, None) => 2,
                (Ty::U32, Some((lo, hi))) => match (&lo.kind, &hi.kind) {
                    (ExprKind::Num { value: a, .. }, ExprKind::Num { value: b, .. }) => (*b - *a).max(0.0) as u64,
                    _ => {
                        return Err(Diag::new(
                            Code::E402,
                            prm.span,
                            "a kind parameter's range must be literal",
                            "write `kind K[n: u in 0..4] @S (...)`",
                        ));
                    }
                },
                (Ty::U32 | Ty::I32, None) => {
                    return Err(Diag::new(
                        Code::E402,
                        prm.span,
                        format!("`{}` has an unbounded domain", prm.name.name),
                        format!("bound it in the kind parameter list: `{}: u in 0..{VARIANT_BUDGET}`", prm.name.name),
                    ));
                }
                _ => {
                    return Err(Diag::new(
                        Code::E401,
                        prm.span,
                        format!("continuous value cannot parameterize kind identity: `{}` is `{t}`", prm.name.name),
                        format!(
                            "move `{}: {t}` into the kind’s column list so all instances share one compiled kind",
                            prm.name.name
                        ),
                    ));
                }
            };
            variants = variants.saturating_mul(n);
            if variants > VARIANT_BUDGET {
                return Err(Diag::new(
                    Code::E402,
                    prm.span,
                    format!("kind variant budget exceeded ({variants} > max {VARIANT_BUDGET})"),
                    format!("move `{}` into the row; each variant is a separate shader module", prm.name.name),
                ));
            }
        }
        Ok(())
    }
    fn kind(&mut self, k: &KindDecl) -> Res<()> {
        self.scopes=vec![HashMap::new()];self.comptime=true;
        for f in &k.row {
            let t = self.resolve(&f.ty, true)?;
            if let Some(r)=&f.range {if !num(&t){return Err(e003(f.name.span,"range on a non-scalar row field","refine a numeric column"))}self.range(r)?;}
            if let Some(e)=&f.default{let from=self.expr(e)?;self.assignable(&from,&t,e.span)?;if from==Ty::AbstractInt{self.set(e,t.clone());}}
            if t.words().is_none() {
                return Err(e003(f.ty.span(), format!("`{t}` cannot be stored in a row"), "use a scalar, vector, point or Xform"));
            }
        }
        let surface = k.query(QueryKind::Surface).is_some();
        let mut world: Option<(Space, Span)> = None;
        for (q, f) in &k.queries {
            self.func(f, Some(&k.name.name))?;
            let sig: Vec<Ty> =
                f.params.iter().skip(1).filter_map(|p| p.ty.as_ref().map(|t| self.resolve(t, true))).collect::<Res<_>>()?;
            let ret = f.ret.as_ref().map(|t| (t, self.resolve(t, true))).map(|(t, r)| (matches!(t, TyExpr::Field(_)), r));
            let (want, ok, space) = match q {
                QueryKind::Visible => (
                    "query visible(self, ro: Pt[S], rd: Dir[S]) -> f32",
                    matches!(sig.as_slice(), [Ty::Geo(Geo::Pt, a), Ty::Geo(Geo::Dir, b)] if a == b)
                        && matches!(ret, Some((false, Ok(Ty::F32)))),
                    sig.first(),
                ),
                QueryKind::Trace => (
                    "query trace(self, p: Pt[S]) -> Field[S]",
                    matches!(sig.as_slice(), [Ty::Geo(Geo::Pt, _)]) && matches!(ret, Some((true, Ok(_)))),
                    sig.first(),
                ),
                QueryKind::Material => (
                    if surface { "query material(self, p: Pt[S], n: Norm[S], uv: vec2<f32>) -> Bsdf" } else { "query material(self, p: Pt[S], n: Norm[S]) -> Bsdf" },
                    match sig.as_slice() {
                        [Ty::Geo(Geo::Pt, a), Ty::Geo(Geo::Norm, b)] => a == b && !surface,
                        [Ty::Geo(Geo::Pt, a), Ty::Geo(Geo::Norm, b), Ty::VecN(2)] => a == b && surface,
                        _ => false,
                    } && matches!(ret, Some((false, Ok(Ty::Bsdf)))),
                    sig.first(),
                ),
                QueryKind::Surface => {
                    let r = match &ret {
                        Some((false, Ok(t @ Ty::Geo(Geo::Pt, _)))) => Some(t.clone()),
                        _ => None,
                    };
                    let grid = f.attrs.iter().any(|a| a.name.name == "grid");
                    let ok = matches!(sig.as_slice(), [Ty::F32, Ty::F32]) && r.is_some() && grid;
                    (
                        "@grid(nu, nv) query surface(self, u: f32, v: f32) -> Pt[S]",
                        ok,
                        if ok { ret.as_ref().and_then(|(_, r)| r.as_ref().ok()) } else { None },
                    )
                }
            };
            if !ok || f.params.first().is_none_or(|p| p.name.name != "self") {
                return Err(e003(f.name.span, format!("`{}` has the wrong signature", f.name.name), format!("write `{want}`")));
            }
            let Some(Ty::Geo(_, s)) = space else { unreachable!("checked above") };
            match &world {
                Some((w, at)) if w != s => {
                    return Err(e104(
                        f.name.span,
                        format!("queries of `{}` are asked in different spaces: `{w}` and `{s}`", k.name.name),
                        format!("ask every query in `{w}`; transform inside the query"),
                    )
                    .note(*at, format!("`{w}` here")));
                }
                Some(_) => {}
                None => world = Some((s.clone(), f.name.span)),
            }
        }
        for f in &k.fns {
            self.func(f, Some(&k.name.name))?;
        }
        let has = |q| k.query(q).is_some();
        if surface && (has(QueryKind::Trace) || has(QueryKind::Visible)) {
            return Err(Diag::new(Code::E405, k.name.span, "a surface kind is drawn from its grid; `trace`/`visible` beside it are not lowered", "split it into two kinds"));
        }
        if k.queries.is_empty(){return Ok(())}
        if !(has(QueryKind::Trace) || surface) || !has(QueryKind::Material) {
            return Err(e003(
                k.name.span,
                format!("kind `{}` needs `query trace` (or `query surface`) and `query material` to be drawn and shaded", k.name.name),
                "add them; `visible` is optional (a closed-form hit replaces sphere tracing)",
            ));
        }
        // A kind asked in its own space is placed by one row field `Xform[S -> World]`; the compiler applies it.
        let (s, at) = world.expect("a query");
        if s != Space::Named("World".into()) {
            let want = Ty::Xform(s.clone(), Space::Named("World".into()));
            let mut poses = 0;
            for f in &k.row {
                if self.resolve(&f.ty, true)? == want {
                    poses += 1;
                }
            }
            if poses != 1 {
                return Err(e104(at, format!("`{}` is asked in `{s}`, so one row field must place it: found {poses} of type `{want}`", k.name.name), format!("add exactly one row field `pose: Xform[{s} -> World]`")));
            }
        }
        Ok(())
    }

    fn block(&mut self, b: &Block) -> Res<()> {
        self.scopes.push(HashMap::new());
        for s in &b.stmts {
            self.stmt(s)?;
        }
        self.scopes.pop();
        Ok(())
    }

    fn stmt(&mut self, s: &Stmt) -> Res<()> {
        match s {
            Stmt::Let { mutable, name, ty, value } => {
                let mut t = self.expr(value)?;
                if let Some(ty) = ty {
                    let want = self.resolve(ty, false)?;
                    self.assignable(&t, &want, value.span)?;
                    if t == Ty::AbstractInt { self.set(value, want.clone()); }
                    t = want;
                }
                if self.kernel.is_some() {
                    let dim = match ty {
                        Some(t) => { let d = self.declared_unit(t); self.expect_unit(value, d)?; d }
                        None => self.expr_units[&value.id],
                    };
                    self.local_units.insert(name.span, dim);
                }
                if t == Ty::AbstractInt {
                    t = Ty::I32;
                }
                self.bind(&name.name, Local { ty: t, mutable: *mutable, map: None, span: name.span });
            }
            Stmt::Assign { target, op: _, value } => {
                if let ExprKind::Member(base,field)=&target.kind {
                    let owned=self.ownership.as_ref().is_some_and(|(row,fields)|matches!(&base.kind,ExprKind::Name(n) if n==row) && fields.contains(&field.name));
                    if !owned{return Err(Diag::new(Code::E501,field.span,"field assignment outside its declared writes set","declare the target field in the owning law’s writes clause"))}
                    let want=self.expr(target)?;let t=self.expr(value)?;self.assignable(&t,&want,value.span)?;if t==Ty::AbstractInt{self.set(value,want);}
                    return Ok(());
                }
                if let ExprKind::Index(base, _) = &target.kind
                    && let ExprKind::Name(n) = &base.kind
                    && self.lookup(n).is_some_and(|local| local.mutable && matches!(local.ty, Ty::Array(..)))
                {
                    let want = self.expr(target)?;
                    let actual = self.expr(value)?;
                    self.assignable(&actual, &want, value.span)?;
                    if actual == Ty::AbstractInt { self.set(value, want); }
                    return Ok(());
                }
                let ExprKind::Name(n) = &target.kind else {
                    return Err(e003(target.span, "assignment needs a mutable local", "assign the whole variable or a mutable array element"));
                };
                let Some(l) = self.lookup(n).cloned() else {
                    return Err(Diag::new(Code::E011, target.span, format!("unknown name `{n}`"), "declare it with `var`"));
                };
                if !l.mutable {
                    return Err(e003(target.span, format!("`{n}` is a `let`"), format!("declare it with `var {n}`"))
                        .note(l.span, "declared here"));
                }
                self.set(target, l.ty.clone());
                let t = self.expr(value)?;
                self.assignable(&t, &l.ty, value.span)?;
                if self.kernel.is_some() { self.expect_unit(value, self.local_units[&l.span])?; }
            }
            Stmt::Map { name, head, stages } => self.map(name, head, stages)?,
            Stmt::Return(v, span) => {
                let t = match v {
                    Some(e) => self.expr(e)?,
                    None => Ty::Void,
                };
                if let Some(h)=&mut self.hints && let Some(n)=&h.current && t!=Ty::Void {h.returns.insert(n.clone(),t);return Ok(());}
                let want = self.ret.clone();
                self.assignable(&t, &want, v.as_ref().map_or(*span, |e| e.span))?;
                if let Some(e)=v && t==Ty::AbstractInt { self.set(e,want); }
            }
            Stmt::If { cond, then, otherwise } => {
                let t = self.expr(cond)?;
                self.assignable(&t, &Ty::Bool, cond.span)?;
                self.block(then)?;
                if let Some(o) = otherwise {
                    self.block(o)?;
                }
            }
            Stmt::Each { var,values,body,span }=>{
                if !self.comptime{return Err(Diag::new(Code::E404,*span,"build loop outside build code","move the loop into a build block"))}
                let first=values.first().ok_or_else(||e003(*span,"empty untyped iteration list","supply at least one value"))?;
                let mut ty=self.expr(first)?;
                if ty==Ty::AbstractInt{ty=Ty::F32}
                for e in values{let t=self.expr(e)?;self.assignable(&t,&ty,e.span)?;if t==Ty::AbstractInt{self.set(e,ty.clone());}}
                self.scopes.push(HashMap::new());self.bind(&var.name,Local{ty,mutable:false,map:None,span:var.span});
                self.block(body)?;self.scopes.pop();
            }
            Stmt::For { comptime, var, lo, hi, body, span } => {
                if *comptime && !self.comptime {
                    return Err(Diag::new(
                        Code::E404,
                        *span,
                        "`comptime for` is stage 0 and runs only inside a `comptime fn`",
                        "move the loop into a `comptime fn` that `emit`s rows, or drop `comptime` to keep a GPU loop",
                    ));
                }
                if self.kernel.is_some() && [lo, hi].iter().any(|e| {
                    let mut refs = crate::references::References::default();
                    refs.expr(e, &[]);
                    refs.names.iter().any(|(n, _)| self.lookup(n).is_some() || !self.p.consts.contains_key(n))
                        || runtime_read(self.p, refs).is_some()
                }) {
                    self.require(KernelCapability::DynamicLoops, *span)?;
                }
                let a = self.expr(lo)?;
                let b = self.expr(hi)?;
                let t = match (&a, &b) {
                    (Ty::AbstractInt, Ty::AbstractInt) => Ty::U32,
                    (Ty::AbstractInt, t) | (t, Ty::AbstractInt) => t.clone(),
                    _ => a.clone(),
                };
                self.assignable(&a, &t, lo.span)?;if a==Ty::AbstractInt{self.set(lo,t.clone());}
                self.assignable(&b, &t, hi.span)?;if b==Ty::AbstractInt{self.set(hi,t.clone());}
                if !matches!(t, Ty::U32 | Ty::I32) {
                    return Err(e003(lo.span, "a range loop counts integers", "write `for i in 0u..n`"));
                }
                self.scopes.push(HashMap::new());
                self.bind(&var.name, Local { ty: t, mutable: false, map: None, span: var.span });
                self.block(body)?;
                self.scopes.pop();
            }
            Stmt::ImportTable { kind, index, span, .. } => {
                if !self.comptime {
                    return Err(Diag::new(Code::E404, *span, "import_table appends a row at stage 0", "import inside a build block"));
                }
                if !self.p.kinds.contains_key(kind.name.as_str()) {
                    return Err(Diag::new(Code::E011, kind.span, format!("unknown kind {}", kind.name), "declare the kind"));
                }
                let ty = self.expr(index)?;
                self.assignable(&ty, &Ty::U32, index.span)?;
                if ty == Ty::AbstractInt {
                    self.set(index, Ty::U32);
                }
            }
            Stmt::Emit { kind, fields, span, bind } => {
                if !self.comptime {
                    return Err(Diag::new(
                        Code::E404,
                        *span,
                        "`emit` appends a row at stage 0 and runs only inside a `comptime fn`",
                        "call a `comptime fn` from the schedule's rows pass and emit there",
                    ));
                }
                let Some(k) = self.p.kinds.get(kind.name.as_str()) else {
                    return Err(Diag::new(Code::E011, kind.span, format!("unknown kind `{}`", kind.name), "declare the kind"));
                };
                let mut supplied=std::collections::HashSet::new();
                for (f, e) in fields {
                    if !supplied.insert(&f.name){return Err(Diag::new(Code::E602,f.span,"row field supplied twice","keep one value for this field"))}
                    let Some(rf) = k.row.iter().find(|r| r.name.name == f.name) else {
                        return Err(Diag::new(
                            Code::E602,
                            f.span,
                            format!("`{}` has no row field `{}`", kind.name, f.name),
                            format!("fields are: {}", k.row.iter().map(|r| r.name.name.as_str()).collect::<Vec<_>>().join(", ")),
                        ));
                    };
                    let want = self.resolve(&rf.ty, true)?;
                    let t = self.expr(e)?;
                    if !self.contextual(e,&t,&want){self.assignable(&t, &want, e.span)?;} if t==Ty::AbstractInt {self.set(e,want);}
                }
                if let Some(n) = bind {self.bind(&n.name,Local{ty:Ty::Id(kind.name.clone()),mutable:false,map:None,span:n.span});}
                if let Some(missing) = k.row.iter().find(|r| r.default.is_none() && !fields.iter().any(|(f, _)| f.name == r.name.name)) {
                    return Err(Diag::new(
                        Code::E602,
                        *span,
                        format!("row of `{}` misses `{}`", kind.name, missing.name.name),
                        format!("give every field a value; add `{}: ...`", missing.name.name),
                    ));
                }
            }
            Stmt::Expr(e) => {
                self.expr(e)?;
            }
        }
        Ok(())
    }

    fn map(&mut self, name: &Ident, head: &Expr, stages: &[(Ident, Vec<Expr>)]) -> Res<()> {
        // `mirror_x(p) |> ...`: the head call is the first stage with the point as its first argument.
        let mut all: Vec<(&Ident, &[Expr])> = vec![];
        let point = match &head.kind {
            ExprKind::Call { callee, args } if MAP_OPS.contains(&callee.name.as_str()) => {
                let Some(first) = args.first() else {
                    return Err(e003(head.span, "a map's first stage takes the point", format!("write `{}(p)`", callee.name)));
                };
                let rest: &[Arg] = &args[1..];
                for a in rest {
                    self.expr(&a.value)?;
                }
                // Stage args are checked below from `stages`; the head's extra args are re-read by later passes.
                all.push((callee, &[]));
                if !rest.is_empty() {
                    return Err(e003(
                        head.span,
                        "write the point alone in the first stage",
                        format!("`map {} = p |> {}(...)`", name.name, callee.name),
                    ));
                }
                self.expr(&first.value)?
            }
            ExprKind::Unproved { .. } | ExprKind::FiniteDiff { .. } | ExprKind::If { .. } | ExprKind::Num { .. } | ExprKind::Bool(_) | ExprKind::String(_) | ExprKind::Name(_) | ExprKind::Member(..) | ExprKind::Index(..) | ExprKind::Call { .. } | ExprKind::Ctor { .. } | ExprKind::Binary(..) | ExprKind::Unary(..) => self.expr(head)?,
        };
        let Ty::Geo(Geo::Pt, parent) = point else {
            return Err(e104(head.span, format!("a map takes a point, found `{point}`"), "map a `Pt[S]`"));
        };
        for (op, args) in stages {
            all.push((op, args));
        }
        let mut mirrors = 0;
        for (op, args) in &all {
            let want: &[Ty] = match op.name.as_str() {
                "mirror_x" => {
                    mirrors += 1;
                    &[]
                }
                "translate" => &[Ty::Geo(Geo::Vec, parent.clone())],
                "rot_x" | "rot_y" | "rot_z" => &[Ty::F32],
                other => {
                    return Err(Diag::new(
                        Code::E011,
                        op.span,
                        format!("`{other}` is not a map stage"),
                        format!("Phase 0 map stages: {}", MAP_OPS.join(", ")),
                    ));
                }
            };
            if args.len() != want.len() {
                return Err(e003(op.span, format!("`{}` takes {} argument(s)", op.name, want.len()), "match the stage's arguments"));
            }
            for (a, w) in args.iter().zip(want) {
                let t = self.expr(a)?;
                if matches!((op.name.as_str(),&t),("translate",Ty::VecN(3))) {continue;}
                self.assignable(&t, w, a.span)?;
            }
        }
        if mirrors > 1 {
            return Err(Diag::new(
                Code::E405,
                name.span,
                "Phase 0 maps carry one mirror (a scalar `.cell`)",
                "split into two maps, one mirror each",
            ));
        }
        self.fresh += 1;
        let ty = Ty::Geo(Geo::Pt, Space::Fresh(name.name.clone(), self.fresh));
        self.bind(&name.name, Local { ty, mutable: false, map: Some((parent, mirrors == 1)), span: name.span });
        Ok(())
    }

    fn expr(&mut self, e: &Expr) -> Res<Ty> {
        let t = self.expr_inner(e)?;
        if self.kernel.is_some() {
            let dim = self.kernel_unit(e)?;
            self.expr_units.insert(e.id, dim);
        }
        Ok(self.set(e, t))
    }

    fn expr_arm(&mut self, arm: &ExprBlock) -> Res<Ty> {
        self.scopes.push(HashMap::new());
        for s in &arm.body.stmts { self.stmt(s)?; }
        let t = self.expr(&arm.value);
        self.scopes.pop();
        t
    }

    fn expr_inner(&mut self, e: &Expr) -> Res<Ty> {
        match &e.kind {
            ExprKind::Unproved { step_scale, body } => {
                let scale = self.expr(step_scale)?;
                self.assignable(&scale, &Ty::F32, step_scale.span)?;
                self.set(step_scale, Ty::F32);
                self.expr_arm(body)
            }
            ExprKind::FiniteDiff { function, parameter, args } => {
                let f = self.p.fns.get(function.name.as_str()).ok_or_else(|| Diag::new(Code::E011,
                    function.span, "unknown differentiated function", "name a declared function"))?;
                let param = if parameter.name.is_empty() { f.params.first() }
                    else { f.params.iter().find(|p| p.name.name == parameter.name) };
                let param = param.ok_or_else(|| Diag::new(Code::E011, parameter.span,
                    "unknown differentiated parameter", "name a parameter of this function"))?;
                if param.ty.as_ref().map(|ty| self.resolve(ty, true)).transpose()? != Some(Ty::F32) {
                    return Err(e003(param.name.span, "finite differences require a scalar floating parameter", "differentiate with respect to an f parameter"));
                }
                match self.call(function, args, e.span)? {
                    Ty::Geo(Geo::Pt | Geo::Dir | Geo::Norm, space) => Ok(Ty::Geo(Geo::Vec, space)),
                    t @ (Ty::F32 | Ty::VecN(_) | Ty::Geo(Geo::Vec, _)) => Ok(t),
                    _ => Err(e003(function.span, "finite differences require a floating result", "differentiate a scalar or vector function")),
                }
            }
            ExprKind::If { condition, then, otherwise } => {
                let actual = self.expr(condition)?;
                self.assignable(&actual, &Ty::Bool, condition.span)?;
                let a = self.expr_arm(then)?;
                let b = self.expr_arm(otherwise)?;
                let ty = meet(&a, &b).ok_or_else(|| e003(e.span, format!("if values have different types {a} and {b}"), "return a single type from both arms"))?;
                if a == Ty::AbstractInt { self.set(&then.value, ty.clone()); }
                if b == Ty::AbstractInt { self.set(&otherwise.value, ty.clone()); }
                Ok(ty)
            }
            ExprKind::Num { suffix, float, .. } => Ok(match suffix {
                NumSuffix::None if !float => Ty::AbstractInt,
                NumSuffix::None | NumSuffix::Deg | NumSuffix::Unit => Ty::F32,
            }),
            ExprKind::Bool(_) => Ok(Ty::Bool),
            ExprKind::String(_) => Err(Diag::new(Code::E403, e.span, "string outside an asset call", "pass it to a Stage 0 asset call")),
            ExprKind::Name(n) => {
                if let Some(l) = self.lookup(n) {
                    return Ok(l.ty.clone());
                }
                if (n == "SceneWorld" || self.p.worlds.contains_key(n.as_str())) && self.p.structs.contains_key("World") { return Ok(Ty::Struct("World".into())); }
                if n=="time" {return if self.comptime{Err(Diag::new(Code::E403,e.span,"time read at build stage","read time in a query or a law"))}else{Ok(Ty::F32)}}
                if let Some(c) = self.p.consts.get(n.as_str()) {
                    if self.comptime {
                        let mut refs = crate::references::References::default();
                        refs.expr(&c.value, &[]);
                        if let Some(read) = runtime_read(self.p, refs) {
                            return Err(stage_error(e.span, read));
                        }
                    }
                    return Ok(match &c.ty {
                        Some(t) => self.resolve(t, true)?,
                        None => self.types.get(c.value.id as usize).cloned().unwrap_or(Ty::F32),
                    });
                }
                Err(Diag::new(Code::E011, e.span, format!("unknown name `{n}`"), "declare it before use"))
            }
            ExprKind::Member(base, m) => self.member(base, m),
            ExprKind::Index(base, i) => {
                let b = self.expr(base)?;
                let it = self.expr(i)?;
                if let Ty::Table(k)=b{
                    if self.kernel.is_some() && matches!(it, Ty::U32 | Ty::AbstractInt) {
                        self.set(i, Ty::U32);
                        return Ok(if self.p.structs.contains_key(k.as_str()) { Ty::Struct(k) } else { Ty::Row(k) });
                    }
                    if it!=Ty::Id(k.clone()){return Err(Diag::new(Code::E108,i.span,format!("expected an id of {k}, found {it}"),"index the table with an id emitted for this kind"))}return Ok(Ty::Row(k))
                }
                self.assignable(&it, &Ty::U32, i.span).or_else(|_| self.assignable(&it, &Ty::I32, i.span))?;
                match b {
                    Ty::VecN(_) | Ty::Geo(..) => Ok(Ty::F32),
                    Ty::Array(t, count) => {
                        if matches!(i.kind, ExprKind::Num { value, .. } if !(0.0..f64::from(count)).contains(&value)) {
                            return Err(e003(i.span, "array index is outside its declared extent", "index an element of the array"));
                        }
                        Ok(*t)
                    }
                    other @ Ty::Void | other @ Ty::Bool | other @ Ty::U32 | other @ Ty::I32 | other @ Ty::F32 | other @ Ty::AbstractInt | other @ Ty::Xform(..) | other @ Ty::Map(..) | other @ Ty::Row(_) | other @ Ty::Frame(_) | other @ Ty::Table(_) | other @ Ty::Verdict | other @ Ty::Struct(_) | other @ Ty::Id(_) | other @ Ty::Bsdf => Err(e003(base.span, format!("cannot index `{other}`"), "index a vector")),
                }
            }
            ExprKind::Unary(op, x) => {
                let t = self.expr(x)?;
                match (op, &t) {
                    (UnOp::Neg, Ty::F32 | Ty::I32 | Ty::AbstractInt | Ty::VecN(_)) => Ok(t),
                    (UnOp::Neg, Ty::Geo(g, _)) if vec_like(*g) => Ok(t),
                    (UnOp::Neg, Ty::Geo(Geo::Pt, _)) => {
                        Err(e104(e.span, "a point has no negation", "subtract an origin first: `-(p - o)` is a Vec"))
                    }
                    (UnOp::Not, Ty::Bool) => Ok(Ty::Bool),
                    _ => Err(e003(e.span, format!("cannot apply `{op:?}` to `{t}`"), "use a number or bool")),
                }
            }
            ExprKind::Binary(op, a, b) => {
                let ta = self.expr(a)?;
                let tb = self.expr(b)?;
                // Build loop counters scale physical quantities through the CPU evaluator's scalar conversion.
                if self.comptime && matches!((*op, &ta, &tb),
                    (BinOp::Mul, Ty::U32 | Ty::I32, Ty::F32) | (BinOp::Mul, Ty::F32, Ty::U32 | Ty::I32))
                {
                    return Ok(Ty::F32);
                }
                binary(*op, &ta, &tb, a.span, b.span, e.span).map_err(|d| {
                    // A value from a map space used outside it (the draft's gaze bug): quote the author's transform
                    // and point at the map that made the space.
                    let Ty::Geo(_, Space::Fresh(m, _)) = &tb else { return d };
                    let Some(l) = self.lookup(m) else { return d };
                    let d = match (&ta, path(a)) {
                        (Ty::Xform(..), Some(x)) => Diag {
                            fix: format!("lift it out of `'{m}` first: `{x} * ({m}.to_parent * value)` inverts the map's offset and mirror"),
                            ..d
                        },
                        _ => d,
                    };
                    d.note(l.span, format!("space `'{m}` introduced here"))
                })
            }
            ExprKind::Ctor { ty, args } => {
                let target = self.resolve(ty, false)?;
                let ts: Vec<Ty> = args.iter().map(|a| self.expr(&a.value)).collect::<Res<_>>()?;
                if let Ty::Xform(a, b) = &target {
                    let ok = matches!(ts.as_slice(), [Ty::Geo(Geo::Pt, s), Ty::F32 | Ty::AbstractInt] if s == b);
                    return if ok {
                        Ok(target)
                    } else {
                        Err(e003(e.span, "an Xform is built from its origin and a yaw", format!("write `Xform[{a} -> {b}](Pt[{b}](x, y, z), yaw)`")))
                    };
                }
                if let Ty::Frame(sp) = &target {
                    let ok = matches!(ts.as_slice(), [Ty::Geo(Geo::Pt, a), Ty::Geo(g1, b), Ty::Geo(g2, c), Ty::Geo(g3, d)]
                        if [a, b, c, d].iter().all(|x| *x == sp) && [g1, g2, g3].iter().all(|g| vec_like(**g)));
                    return if ok { Ok(target) } else { Err(e104(e.span, "a frame is built from its origin and three directions in its space", format!("write `Frame[{sp}](origin, tangent, normal, binormal)`"))) };
                }
                let n = match &target {
                    Ty::VecN(n) => *n,
                    Ty::Geo(..) => 3,
                    other @ Ty::Void | other @ Ty::Bool | other @ Ty::U32 | other @ Ty::I32 | other @ Ty::F32 | other @ Ty::AbstractInt | other @ Ty::Xform(..) | other @ Ty::Map(..) | other @ Ty::Row(_) | other @ Ty::Frame(_) | other @ Ty::Table(_) | other @ Ty::Verdict | other @ Ty::Struct(_) | other @ Ty::Array(..) | other @ Ty::Id(_) | other @ Ty::Bsdf => return Err(e003(e.span, format!("cannot construct `{other}`"), "construct a vector or point")),
                };
                components(&ts, n, e.span)?;
                Ok(target)
            }
            ExprKind::Call { callee, args } => self.call(callee, args, e.span),
        }
    }

    fn member(&mut self, base: &Expr, m: &Ident) -> Res<Ty> {
        if let ExprKind::Name(n) = &base.kind
            && let Some(Local { ty: Ty::Geo(_, fresh), map: Some((parent, cell)), span, .. }) = self.lookup(n).cloned()
        {
            match m.name.as_str() {
                "cell" if cell => return Ok(Ty::F32),
                "cell" => {
                    return Err(Diag::new(Code::E011, m.span, format!("map `{n}` has no `.cell`"), "only a mirror stage gives a cell")
                        .note(span, "map introduced here"));
                }
                "to_parent" => {
                    self.set(base, Ty::Geo(Geo::Pt, fresh.clone()));
                    return Ok(Ty::Map(fresh, parent));
                }
                "from_parent" => {
                    self.set(base, Ty::Geo(Geo::Pt, fresh.clone()));
                    return Ok(Ty::Map(parent, fresh));
                }
                _ => {}
            }
        }
        let b = self.expr(base)?;
        match &b {
            Ty::Row(k) => {
                let kd = self.p.kinds[k.as_str()];
                match kd.row.iter().find(|r| r.name.name == m.name) {
                    Some(r) => self.resolve(&r.ty, true),
                    None => Err(Diag::new(
                        Code::E011,
                        m.span,
                        format!("`{k}` has no row field `{}`", m.name),
                        format!("fields are: {}", kd.row.iter().map(|r| r.name.name.as_str()).collect::<Vec<_>>().join(", ")),
                    )),
                }
            }
            Ty::Struct(k) if k == "PriorSample" => match m.name.as_str() {
                "pos" => Ok(Ty::Geo(Geo::Pt, Space::Named("World".into()))),
                "vel" => Ok(Ty::Geo(Geo::Vec, Space::Named("World".into()))),
                "rot" => Ok(Ty::VecN(4)),
                "compliance" => Ok(Ty::F32),
                "active" => Ok(Ty::Bool),
                _ => Err(e003(m.span, "unknown prior sample field", "use pos, vel, rot, compliance or active")),
            },
            Ty::Struct(k) => {
                let (_, fields) = self.p.structs[k.as_str()];
                let f = fields.iter().find(|f| f.name.name == m.name).ok_or_else(||
                    Diag::new(Code::E011, m.span, format!("`{k}` has no column `{}`", m.name), "use a declared column"))?;
                self.resolve(&f.ty, true)
            }
            Ty::VecN(n) => swizzle(&m.name, *n as usize, m.span),
            Ty::Geo(..) => swizzle(&m.name, 3, m.span),
            Ty::Frame(sp) => match frame_field(&m.name) {
                "origin" => Ok(Ty::Geo(Geo::Pt, sp.clone())),
                "tangent" | "normal" | "binormal" => Ok(Ty::Geo(Geo::Dir, sp.clone())),
                _ => Err(Diag::new(Code::E011, m.span, format!("`Frame` has no member `{}`", m.name), "use origin, tangent, normal or binormal")),
            },
            other @ Ty::Void | other @ Ty::Bool | other @ Ty::U32 | other @ Ty::I32 | other @ Ty::F32 | other @ Ty::AbstractInt | other @ Ty::Xform(..) | other @ Ty::Map(..) | other @ Ty::Table(_) | other @ Ty::Verdict | other @ Ty::Id(_) | other @ Ty::Array(..) | other @ Ty::Bsdf => Err(e003(m.span, format!("`{other}` has no member `{}`", m.name), "access a row field or a swizzle")),
        }
    }

    fn call(&mut self, callee: &Ident, args: &[Arg], span: Span) -> Res<Ty> {
        if let Some(a) = args.iter().find_map(|a| a.name.as_ref()) {
            return Err(Diag::new(Code::E405, a.span, "named arguments are schedule-only in Phase 0", "pass arguments by position"));
        }
        let spec = lookup(&callee.name);
        if let Some(spec) = spec.filter(|spec| spec.id == Builtin::SamplePrior) {
            builtin_arity(spec, args.len(), span)?;
            let (prior, time) = (&args[0], &args[1]);
            let decl = match &prior.value.kind {
                ExprKind::Name(name) => self.p.plans.get(name.as_str()),
                _ => None,
            }.ok_or_else(|| e003(prior.value.span, "expected a named plan", "name a declared plan"))?;
            if self.ownership.is_none() || !crate::plan_source::closed(self.p, decl) {
                return Err(Diag::new(Code::E405, span, "sample_prior needs a closed plan and a motion law",
                    "bind every plan parameter at Stage 0 and sample from a law"));
            }
            let actual = self.expr(&time.value)?;
            self.assignable(&actual, &Ty::F32, time.value.span)?;
            self.set(&time.value, Ty::F32);
            self.set(&prior.value, Ty::U32);
            return Ok(Ty::Struct("PriorSample".into()));
        }
        if let Some(spec) = spec.filter(|spec| spec.sig == SigRule::Poly(PolyRule::TextureSample))
            && !self.p.fns.contains_key(callee.name.as_str())
        {
            builtin_arity(spec, args.len(), span)?;
            let name = &args[0].value;
            let ExprKind::Name(name) = &name.kind else {
                return Err(e003(name.span, "expected a baked texture name", "name a bake_tex declaration"));
            };
            let bake = self.p.bakes.get(name.as_str()).ok_or_else(|| Diag::new(Code::E011, span,
                format!("unknown baked texture {name}"), "declare it with bake_tex"))?;
            if matches!(bake.dim, TexDim::Cube(_)) && (bake.format.name == "r32float" || self.kernel.is_some()) {
                return Err(e003(span, "cube sampling requires a filterable shader binding", "sample a filterable cube in a render query or bake a 2D or 3D texture"));
            }
            if self.comptime || self.ownership.is_some() {
                return Err(Diag::new(Code::E403, span, "texture sampling needs a shader", "sample in a material, trace, surface or kernel"));
            }
            let coord = self.resolve(bake.texel.params[0].ty.as_ref().expect("typed bake coordinate"), true)?;
            let actual = self.expr(&args[1].value)?;
            self.assignable(&actual, &coord, args[1].value.span)?;
            if let Some(lod) = args.get(2) {
                let actual = self.expr(&lod.value)?;
                self.assignable(&actual, &Ty::F32, lod.value.span)?;
                self.set(&lod.value, Ty::F32);
            }
            return self.resolve(bake.texel.ret.as_ref().expect("typed bake value"), true);
        }
        // `sample_c2(scan, t)`: the scan is named, not passed as a value.
        if let Some(spec) = spec.filter(|spec| spec.sig == SigRule::Poly(PolyRule::ScanFrame)) {
            builtin_arity(spec, args.len(), span)?;
            let sc = match args.first().map(|a| &a.value.kind) {
                Some(ExprKind::Name(n)) => self.p.scans.get(n.as_str()).copied(),
                _ => None,
            };
            let Some(sc) = sc else {
                return Err(e003(span, format!("`{}` takes a scan's name and its parameter", callee.name), "write `sample_c2(my_scan, t)`"));
            };
            self.set(&args[0].value, Ty::Void);
            for a in &args[1..] {
                let t = self.expr(&a.value)?;
                self.assignable(&t, &Ty::F32, a.value.span)?;
            }
            return self.types.get(sc.init.id as usize).filter(|t| matches!(t, Ty::Frame(_))).cloned().ok_or_else(|| e003(sc.init.span,"scan result type unavailable","initialise the scan with a frame"));
        }
        if let Some(spec) = spec.filter(|spec| spec.id == Builtin::Unknown)
            && !self.p.fns.contains_key(callee.name.as_str())
        {
            builtin_arity(spec, args.len(), span)?;
            if let ExprKind::Name(reason) = &args[0].value.kind
                && crate::reason(reason).is_some()
            {
                return Ok(Ty::Verdict);
            }
            return Err(e003(span, "unknown verdict requires a reason tag", "write unknown(MissingSurface)"));
        }
        if callee.name == EMIT_EVENT {
            if self.ownership.is_none() && self.kernel.is_none() {
                return Err(e003(span, "emit_event requires an effectful entry", "call it in a law, solver or kernel"));
            }
            let [tag, instance, site, payload] = args else {
                return Err(e003(span, "emit_event needs four arguments", "provide tag, instance_id, site_id and payload"));
            };
            if !matches!(&tag.value.kind, ExprKind::Name(n) if n.as_bytes().first().is_some_and(u8::is_ascii_uppercase)) {
                return Err(e003(tag.value.span, "expected an event symbol", "name the event tag with an initial capital"));
            }
            self.set(&tag.value, Ty::U32);
            for (arg, want) in [(instance, Ty::U32), (site, Ty::U32), (payload, Ty::VecN(4))] {
                let actual = self.expr(&arg.value)?;
                self.assignable(&actual, &want, arg.value.span)?;
                self.set(&arg.value, want);
            }
            return Ok(Ty::Void);
        }
        let ts: Vec<Ty> = args.iter().map(|a| self.expr(&a.value)).collect::<Res<_>>()?;
        let sp: Vec<Span> = args.iter().map(|a| a.value.span).collect();
        if let Some(f) = self.p.fns.get(callee.name.as_str())
            && f.attrs.iter().any(|a| a.name.name == "lobe")
        {
            // A lobe call binds its own parameters; the shade pass supplies `n, v, l`.
            if self.comptime || ts.len() + 3 != f.params.len() {
                return Err(e003(span, format!("lobe `{}` takes {} argument(s) here (the last three, n, v, l, come from the light)", callee.name, f.params.len().saturating_sub(3)), "pass the lobe's own parameters")
                    .note(f.name.span, "declared here"));
            }
            for (index, ((prm, t), s)) in f.params.iter().zip(&ts).zip(&sp).enumerate() {
                let want = self.resolve(prm.ty.as_ref().ok_or_else(||e003(prm.name.span,"lobe parameter has no type","annotate the lobe parameter"))?, true)?;
                self.assignable(t, &want, *s)?; if *t==Ty::AbstractInt {self.set(&args[index].value,want);}
            }
            return Ok(Ty::Bsdf);
        }
        if let Some(f) = self.p.fns.get(callee.name.as_str()) {
            if self.comptime && let Some(read) = runtime_read(self.p, crate::references::References::function(f)) {
                return Err(stage_error(callee.span, read));
            }
            if f.comptime && !self.comptime {
                return Err(Diag::new(
                    Code::E403,
                    callee.span,
                    format!("kind code calls the stage-0 fn `{}`", callee.name),
                    "call it from a `build` block, or declare it as a regular `fn`",
                )
                .note(f.name.span, "declared `build fn` here"));
            }
            if f.params.len() != ts.len() {
                return Err(e003(span, format!("`{}` takes {} argument(s), got {}", callee.name, f.params.len(), ts.len()), "match the signature")
                    .note(f.name.span, "declared here"));
            }
            if f.params.iter().any(|p|p.ty.is_none()){
                if let Some(h)=&mut self.hints {h.args.entry(f.name.name.clone()).or_insert_with(||ts.clone());}
                return Err(e003(f.name.span,"parameter type has not been inferred","annotate the parameter or call the helper with a typed argument"));
            }
            let mut vars: HashMap<String, Space> = HashMap::new();
            // Space variables a space-less offset was accepted for: `sphere(local, r)` where `local` was computed
            // without spaces (3 of 19 first attempts). The offset is in whatever space the call is in.
            let mut loose: Vec<(String, Span)> = vec![];
            for (index, ((prm, t), s)) in f.params.iter().zip(&ts).zip(&sp).enumerate() {
                let want = self.resolve(prm.ty.as_ref().expect("free fns type every parameter"), true)?;
                let want=if let (Ty::Id(k),Ty::Row(_))=(&want,t){Ty::Row(k.clone())}else{want};
                let want = self.unify(&want, t, &mut vars, *s, &prm.name)?;
                let want=subst(&want,&vars);
                if self.contextual(&args[index].value,t,&want){continue;}
                if let (Ty::VecN(3), Ty::Geo(g, Space::Var(v))) = (t, &want) && vec_like(*g) {
                    loose.push((v.clone(), *s));
                    continue;
                }
                self.assignable(t, &want, *s)?; if *t==Ty::AbstractInt {self.set(&args[index].value,want);}
            }
            let ret = match &f.ret {
                Some(r) => self.resolve(r, true)?,
                None => Ty::Void,
            };
            let ret = subst(&ret, &vars);
            // An unbound space must not escape into the result: then the offset needs its space spelled out.
            if let Some((v, s)) = loose.iter().find(|(v, _)| matches!(&ret, Ty::Geo(_, Space::Var(r)) | Ty::Frame(Space::Var(r)) if r == v)) {
                return Err(e104(*s, format!("a space-less vec3 where `v3@{v}` is expected and the result is in `{v}`"),
                    "derive the offset from points (`p - at`) so the call knows its space"));
            }
            return Ok(ret);
        }
        if let Some(world) = crate::world::WorldCall::resolve(&callee.name, &ts, span)? {
            if matches!(world.query, crate::world::WorldQuery::Sky { .. }) {
                self.p.linked(callee.span, "sky_dome")?;
            }
            if !matches!(world.query, crate::world::WorldQuery::Trace { .. } | crate::world::WorldQuery::Visible { .. }
                | crate::world::WorldQuery::Sky { .. } | crate::world::WorldQuery::Medium { .. }) {
                self.pending_lowering.get_or_insert(span);
            }
            return self.world_query(world, span);
        }
        match callee.name.as_str() {
            INSCATTER_JOINT => {
                let [world, ray, distance, tail @ ..] = ts.as_slice() else {
                    return Err(e003(span, "inscatter_joint needs a world, ray, ray limit and RNG key", "write inscatter_joint(w, r, t_max, steps, rng)"));
                };
                let rng = match tail {
                    [rng] => rng,
                    [steps, rng] => {
                        self.assignable(steps, &Ty::U32, args[3].value.span)?;
                        if *steps == Ty::AbstractInt { self.set(&args[3].value, Ty::U32); }
                        rng
                    }
                    _ => return Err(e003(span, "inscatter_joint takes four or five arguments", "omit steps for 24 integration steps")),
                };
                let named = |name: &str| self.resolve(&TyExpr::Name(Ident { name: name.into(), span }), true);
                self.assignable(world, &named("World")?, sp[0])?;
                self.assignable(ray, &named("Ray")?, sp[1])?;
                self.assignable(distance, &Ty::F32, sp[2])?;
                self.assignable(rng, &named("RngKey")?, *sp.last().unwrap())?;
                if named("Radiance")? != Ty::VecN(3) {
                    return Err(Diag::new(Code::E405, span, "the RGB medium marcher needs Radiance = rgb",
                        "supply a marcher for the schedule's custom radiance"));
                }
                let result = named("Vol")?;
                if *distance == Ty::AbstractInt { self.set(&args[2].value, Ty::F32); }
                return Ok(result);
            }
            ATOMIC_ADD => {
                self.require(KernelCapability::Atomics, span)?;
                let [dst, val] = args else { return Err(e003(span, "atomic_add needs a destination and value", "write atomic_add(table[i].field, value)")); };
                let ExprKind::Member(row, _) = &dst.value.kind else { return Err(e003(dst.value.span, "atomic destination must be a table field", "index a kernel table parameter")); };
                let ExprKind::Index(table, _) = &row.kind else { return Err(e003(row.span, "expected a table row", "index a kernel table parameter")); };
                let ExprKind::Name(n) = &table.kind else { return Err(e003(table.span, "expected a table parameter", "name a kernel table parameter")); };
                let binding = self.lookup(n).map(|l| l.span);
                if !self.kernel.unwrap().f.params.iter().any(|p| p.name.name == *n && Some(p.name.span) == binding && matches!(p.ty, Some(TyExpr::Table(_)))) {
                    return Err(e003(table.span, "atomic destination is not a table parameter", "write the destination through its table parameter"));
                }
                let det = match ts[0] {
                    Ty::U32 | Ty::I32 => self.kernel.unwrap().tier,
                    Ty::F32 if self.kernel.unwrap().tier == Determinism::NonDet => Determinism::NonDet,
                    Ty::F32 => return Err(Diag::new(Code::E504, dst.value.span, "float atomic accumulation requires NonDet",
                        "tag the kernel nondet or accumulate integers with an explicit fixed-point scale")),
                    _ => return Err(e003(dst.value.span, "atomic destination must be u, i or f", "use a scalar accumulator")),
                };
                self.assignable(&ts[1], &ts[0], val.value.span)?;
                if ts[1] == Ty::AbstractInt { self.set(&val.value, ts[0].clone()); }
                self.atomic_writes.entry(n.clone()).and_modify(|d| *d = (*d).max(det)).or_insert(det);
                return Ok(Ty::Void);
            }
            GLOBAL_ID => {
                if self.kernel.is_some() && !self.comptime && let [axis] = ts.as_slice() {
                    self.assignable(axis, &Ty::U32, args[0].value.span)?;
                    self.set(&args[0].value, Ty::U32);
                    return Ok(Ty::U32);
                }
                return Err(e003(span, "global invocation ID needs a kernel and one axis", "write global_id(0) for the X axis"));
            }
            WORKGROUP_BARRIER => {
                self.require(KernelCapability::Workgroup, span)?;
                if ts.is_empty() { return Ok(Ty::Void); }
                return Err(e003(span, "workgroup_barrier takes no arguments", "write workgroup_barrier()"));
            }
            SUBGROUP_ADD => {
                self.require(KernelCapability::Subgroups, span)?;
                if let [t @ (Ty::F32 | Ty::U32 | Ty::I32)] = ts.as_slice() {
                    if *t == Ty::F32 && self.kernel.unwrap().tier.is_det() {
                        return Err(Diag::new(Code::E504, span, "floating subgroup reduction requires NonDet",
                            "tag the kernel nondet or reduce integer contributions"));
                    }
                    return Ok(t.clone());
                }
                return Err(e003(span, "subgroup_add takes one numeric scalar", "pass an f, i or u value"));
            }
            _ => {}
        }
        if MAP_OPS.contains(&callee.name.as_str()) {
            return Err(Diag::new(
                Code::E011,
                callee.span,
                format!("`{}` is a map stage", callee.name),
                format!("use it in a map statement: `map m = p |> {}(...);`", callee.name),
            ));
        }
        let Some(spec) = spec.filter(|spec| spec.sig != SigRule::Poly(PolyRule::Unchecked)) else {
            return Err(Diag::new(
                Code::E011, span, format!("unknown function `{}`", callee.name),
                "declare it with `fn`, or check the spelling",
            ));
        };
        builtin_arity(spec, ts.len(), span)?;
        match spec.sig {
            SigRule::Fixed(params, result) => {
                for ((arg, actual), shape) in args.iter().zip(&ts).zip(params) {
                    let expected = match shape {
                        ArgShape::Scalar if num(actual) => continue,
                        ArgShape::Spatial if matches!(actual, Ty::Geo(..)) => continue,
                        ArgShape::Scalar => return Err(e003(arg.value.span, "expected a numeric scalar", "convert the argument to a scalar")),
                        ArgShape::Spatial => return Err(e003(arg.value.span, "expected a spatial value", "pass a point or vector")),
                        ArgShape::F32 => {
                            if *actual != Ty::F32 {
                                return Err(e003(arg.value.span, "expected f32", "pass an f32 value"));
                            }
                            continue;
                        }
                        ArgShape::Float => Ty::F32,
                        ArgShape::U32 => Ty::U32,
                        ArgShape::Vec2 => Ty::VecN(2),
                        ArgShape::Vec3 => Ty::VecN(3),
                        ArgShape::Bsdf => Ty::Bsdf,
                        ArgShape::Plan | ArgShape::Reason => unreachable!("named builtin operands are checked before value operands"),
                    };
                    self.assignable(actual, &expected, arg.value.span)?;
                    // Packed words and BSDF weights retain their contextual types in IR.
                    if (*shape == ArgShape::U32 && *actual == Ty::AbstractInt)
                        || (spec.id == Builtin::MixBsdf && *shape == ArgShape::Float)
                    {
                        self.set(&arg.value, expected);
                    }
                }
                Ok(match result {
                    RetShape::Bool => Ty::Bool,
                    RetShape::F32 => Ty::F32,
                    RetShape::I32 => Ty::I32,
                    RetShape::U32 => Ty::U32,
                    RetShape::Vec2 => Ty::VecN(2),
                    RetShape::Vec3 => Ty::VecN(3),
                    RetShape::Vec4 => Ty::VecN(4),
                    RetShape::Verdict => Ty::Verdict,
                    RetShape::Bsdf => Ty::Bsdf,
                    RetShape::PriorSample => Ty::Struct("PriorSample".into()),
                })
            }
            SigRule::Poly(rule) => builtin_poly(spec.name, rule, &ts, &sp, span),
        }
    }

    fn world_query(&self, call: crate::world::WorldCall<Ty>, span: Span) -> Res<Ty> {
        use crate::world::{LightSelection, WorldQuery::*};
        let payload = |s: &str| Ty::Struct(s.into());
        let point = Ty::Geo(Geo::Pt, Space::Named("World".into()));
        let dir = Ty::Geo(Geo::Dir, Space::Named("World".into()));
        let named = |s: &str| self.resolve(&TyExpr::Name(Ident { name: s.into(), span }), true);
        if let Some(world) = call.world { self.assignable(&world, &payload("World"), span)?; }
        match call.query {
            Trace { ray } => { self.assignable(&ray, &payload("Ray"), span)?; named("Hit") }
            Visible { a, b } => {
                self.assignable(&a, &point, span)?;
                self.assignable(&b, &point, span)?;
                Ok(Ty::VecN(3))
            }
            Material { hit } => { self.assignable(&hit, &payload("Hit"), span)?; Ok(Ty::Bsdf) }
            Light { selection, point: p, rng } => {
                if let LightSelection::Indexed(i) = selection { self.assignable(&i, &Ty::U32, span)?; }
                self.assignable(&p, &point, span)?;
                self.assignable(&rng, &named("RngKey")?, span)?;
                named("Lit")
            }
            Sky { direction } => { self.assignable(&direction, &dir, span)?; named("Radiance") }
            Medium { point: p } => {
                self.assignable(&p, &point, span)?;
                if named("Radiance")? != Ty::VecN(3) {
                    return Err(Diag::new(Code::E405, span, "the field medium sampler needs Radiance = rgb",
                        "supply coefficient sampling for the schedule's custom radiance"));
                }
                named("Sigma")
            }
        }
    }

    /// Binds a signature's space variables from an argument; a clash is E104.
    fn unify(&self, want: &Ty, got: &Ty, vars: &mut HashMap<String, Space>, span: Span, prm: &Ident) -> Res<Ty> {
        let (v, s, rebuild): (&String, &Space, fn(&Ty, Space) -> Ty) = match (want, got) {
            (Ty::Geo(_, Space::Var(v)), Ty::Geo(_, s)) => (v, s, |w, s| match w {
                Ty::Geo(g, _) => Ty::Geo(*g, s),
                other => other.clone(),
            }),
            (Ty::Frame(Space::Var(v)), Ty::Frame(s)) => (v, s, |_, s| Ty::Frame(s)),
            (Ty::Geo(_, Space::Var(_)) | Ty::Frame(Space::Var(_)), _) => return Ok(want.clone()),
            _ => return Ok(subst(want, vars)),
        };
        match vars.get(v) {
            Some(bound) if bound != s => Err(e104(
                span,
                format!("`{}` is in `{s}` but `'{v}` is already `{bound}` in this call", prm.name),
                format!("pass every `'{v}` argument in `{bound}`, or move this one there"),
            )),
            Some(_) => Ok(rebuild(want, s.clone())),
            None => {
                vars.insert(v.clone(), s.clone());
                Ok(rebuild(want, s.clone()))
            }
        }
    }
}

/// `a.b.c` as written, for quoting the author's expression in a fix.
fn path(e: &Expr) -> Option<String> {
    match &e.kind {
        ExprKind::Name(n) => Some(n.clone()),
        ExprKind::Member(b, m) => path(b).map(|p| format!("{p}.{}", m.name)),
        ExprKind::Num { .. }
        | ExprKind::Bool(_) | ExprKind::String(_)
        | ExprKind::Index(..)
        | ExprKind::Call { .. }
        | ExprKind::Ctor { .. }
        | ExprKind::Binary(..)
        | ExprKind::Unproved { .. } | ExprKind::FiniteDiff { .. } | ExprKind::If { .. } | ExprKind::Unary(..) => None,
    }
}

pub const MAP_OPS: [&str; 5] = ["mirror_x", "translate", "rot_x", "rot_y", "rot_z"];

fn subst(t: &Ty, vars: &HashMap<String, Space>) -> Ty {
    let s = |x: &Space| match x {
        Space::Var(v) => vars.get(v).cloned().unwrap_or_else(|| x.clone()),
        Space::Open | Space::Named(_) | Space::Fresh(..) => x.clone(),
    };
    match t {
        Ty::Geo(g, x) => Ty::Geo(*g, s(x)),
        Ty::Xform(a, b) => Ty::Xform(s(a), s(b)),
        Ty::Frame(x) => Ty::Frame(s(x)),
        other @ Ty::Void | other @ Ty::Bool | other @ Ty::U32 | other @ Ty::I32 | other @ Ty::F32 | other @ Ty::AbstractInt | other @ Ty::VecN(_) | other @ Ty::Map(..) | other @ Ty::Row(_) | other @ Ty::Table(_) | other @ Ty::Verdict | other @ Ty::Struct(_) | other @ Ty::Array(..) | other @ Ty::Id(_) | other @ Ty::Bsdf => other.clone(),
    }
}

fn swizzle(m: &str, n: usize, span: Span) -> Res<Ty> {
    match swizzle_len(m, n) {
        Some(1) => Ok(Ty::F32),
        Some(k) => Ok(Ty::VecN(k as u8)),
        None => Err(Diag::new(Code::E011, span, format!("no member `{m}`"), "use a swizzle like .x or .xz")),
    }
}

/// Constructor arguments must add up to `n` components, or be one scalar to splat.
fn components(ts: &[Ty], n: u8, span: Span) -> Res<()> {
    let mut total = 0u8;
    for t in ts {
        total += match t {
            Ty::F32 | Ty::AbstractInt => 1,
            Ty::VecN(k) => *k,
            Ty::Geo(..) => {
                return Err(e104(span, "constructing from a value with a space", "drop the space explicitly with `.xyz`"));
            }
            other @ Ty::Void | other @ Ty::Bool | other @ Ty::U32 | other @ Ty::I32 | other @ Ty::Xform(..) | other @ Ty::Map(..) | other @ Ty::Row(_) | other @ Ty::Frame(_) | other @ Ty::Table(_) | other @ Ty::Verdict | other @ Ty::Struct(_) | other @ Ty::Array(..) | other @ Ty::Id(_) | other @ Ty::Bsdf => return Err(e003(span, format!("cannot build a vector from `{other}`"), "pass f32 components")),
        };
    }
    if total == n || (ts.len() == 1 && total == 1) {
        Ok(())
    } else {
        Err(e003(span, format!("{total} components for a {n}-vector"), format!("pass {n} components or one scalar")))
    }
}

fn num(t: &Ty) -> bool {
    matches!(t, Ty::F32 | Ty::U32 | Ty::I32 | Ty::AbstractInt)
}

/// The numeric type two operands meet at: an integer literal takes the other side's type.
fn meet(a: &Ty, b: &Ty) -> Option<Ty> {
    match (a, b) {
        _ if a == b => Some(a.clone()),
        (Ty::AbstractInt, t) | (t, Ty::AbstractInt) if num(t) => Some(t.clone()),
        _ => None,
    }
}

fn binary(op: BinOp, a: &Ty, b: &Ty, _sa: Span, sb: Span, span: Span) -> Res<Ty> {
    use BinOp::*;
    use Geo::*;
    let mismatch = |x: &Space, y: &Space| {
        e104(span, format!("space mismatch: `{a}` {} `{b}`", op.wgsl()), format!("bring both into one space; `{x}` and `{y}` differ"))
    };
    match (op, a, b) {
        (And | Or, Ty::Bool, Ty::Bool) => Ok(Ty::Bool),
        (Lt | Le | Gt | Ge | Eq | Ne, x, y) if meet(x, y).is_some() => Ok(Ty::Bool),
        (Eq | Ne, Ty::Bool, Ty::Bool) => Ok(Ty::Bool),
        (BitAnd | BitOr | BitXor | Shl | Shr, x, y) if matches!(meet(x, y), Some(Ty::U32 | Ty::I32 | Ty::AbstractInt)) => {
            Ok(meet(x, y).expect("checked"))
        }
        (Add | Sub | Mul | Div | Rem, x, y) if num(x) && num(y) => {
            meet(x, y).ok_or_else(|| e003(span, format!("`{a}` {} `{b}`", op.wgsl()), "convert one side, e.g. `f32(i)`"))
        }
        (Add, Ty::Bsdf, Ty::Bsdf) => Ok(Ty::Bsdf),
        (Mul, Ty::Xform(x, y), Ty::Xform(z, w)) => {
            if x == w || *x==Space::Open { Ok(Ty::Xform(z.clone(), y.clone())) } else { Err(e104(sb, format!("composing `{a}` after `{b}`"), format!("the right transform must end in `{x}`"))) }
        }
        (Add | Sub | Mul | Div, Ty::VecN(n), Ty::VecN(m)) if n == m => Ok(a.clone()),
        (Mul | Div, Ty::VecN(_), s) if num(s) => Ok(a.clone()),
        // A colour plus a number adds it to every channel, as in WGSL: `floor(lit * 4.0 + 0.5) / 4.0` is the
        // banding idiom, and refusing it was 6 of 19 first-attempt failures in crab-repair-v1's opening rounds.
        (Add | Sub, Ty::VecN(_), s) if num(s) => Ok(a.clone()),
        (Add | Sub, s, Ty::VecN(_)) if num(s) => Ok(b.clone()),
        (Mul, s, Ty::VecN(_)) if num(s) => Ok(b.clone()),
        // Affine rules (draft §2A).
        (Sub, Ty::Geo(Pt, x), Ty::Geo(Pt, y)) => if x == y { Ok(Ty::Geo(Vec, x.clone())) } else { Err(mismatch(x, y)) },
        (Add | Sub, Ty::Geo(Pt, _), Ty::VecN(3)) => Ok(a.clone()),
        (Add | Sub, Ty::Geo(Pt, x), Ty::Geo(g, y)) if vec_like(*g) => if x == y { Ok(a.clone()) } else { Err(mismatch(x, y)) },
        (Add, Ty::Geo(g, x), Ty::Geo(Pt, y)) if vec_like(*g) => if x == y { Ok(b.clone()) } else { Err(mismatch(x, y)) },
        (Add, Ty::Geo(Pt, _), Ty::Geo(Pt, _)) => Err(Diag::new(Code::E101,
            span,
            "`Pt + Pt` has no meaning: points do not add",
            "write an offset without a space (`at + [0, 1, 0]`), subtract an origin (`Pt - Pt` is a Vec), or use `mix(p, q, t)`",
        )),
        // A space-less vec3 beside an offset takes the offset's space, as it already does beside a point
        // (`at + [0, 1, 0]` above). Without this, `p - at - [0, 1, 0]` was E104 and an offset literal could
        // only be spelled `[x, y, z]@S - [0]@S`; crab64 programs failed on both.
        (Add | Sub, Ty::Geo(g, s), Ty::VecN(3)) | (Add | Sub, Ty::VecN(3), Ty::Geo(g, s)) if vec_like(*g) => {
            Ok(Ty::Geo(Vec, s.clone()))
        }
        (Add | Sub, Ty::Geo(g, x), Ty::Geo(h, y)) if vec_like(*g) && vec_like(*h) => {
            if x == y { Ok(Ty::Geo(Vec, x.clone())) } else { Err(mismatch(x, y)) }
        }
        (Mul | Div, Ty::Geo(g, _), s) if vec_like(*g) && num(s) => Ok(Ty::Geo(Vec, geo_space(a))),
        (Mul, s, Ty::Geo(g, _)) if vec_like(*g) && num(s) => Ok(Ty::Geo(Vec, geo_space(b))),
        (Mul | Div, Ty::Geo(g, _), Ty::VecN(3)) if vec_like(*g) => Ok(Ty::Geo(Vec, geo_space(a))),
        // A scaled point is no longer a position, but it is good coordinates: `vnoise(p * 3.5)` is how
        // procedural texture is written, and it failed 3 of 16 fresh crab programs. The result has no
        // space, so using it where a point is required is still refused.
        (Mul | Div, Ty::Geo(Pt, _), s) if num(s) || *s == Ty::VecN(3) => Ok(Ty::VecN(3)),
        (Mul, s, Ty::Geo(Pt, _)) if num(s) => Ok(Ty::VecN(3)),
        (Mul, Ty::Xform(x, y), Ty::Geo(g, s)) | (Mul, Ty::Map(x, y), Ty::Geo(g, s)) => {
            if x == s || *x == Space::Open {
                Ok(Ty::Geo(*g, y.clone()))
            } else {
                let fix = match (a, s) {
                    (Ty::Xform(..), Space::Fresh(m, _)) => {
                        format!("lift the value out of `'{m}` first with `{m}.to_parent`, which inverts the map's offset and mirror")
                    }
                    _ => format!("the transform takes `{x}`; bring the value into `{x}` first"),
                };
                Err(e104(sb, format!("space mismatch in transform application: expected `{g:?}[{x}]`, found `{b}`"), fix))
            }
        }
        (Add | Sub, Ty::Geo(_, s), Ty::VecN(3)) | (Add | Sub, Ty::VecN(3), Ty::Geo(_, s)) => Err(e104(
            span,
            format!("a space-less vec3 combined with a value in `{s}`"),
            // The old hint named `Vec[S](...)`, which does not parse.
            format!("put the space-less offset after the point (`at + [x, y, z]`), or write a point as `[x, y, z]@{s}`"),
        )),
        _ => Err(e003(span, format!("no operator `{}` for `{a}` and `{b}`", op.wgsl()), "convert the operands")),
    }
}

fn geo_space(t: &Ty) -> Space {
    match t {
        Ty::Geo(_, s) => s.clone(),
        other @ Ty::Void | other @ Ty::Bool | other @ Ty::U32 | other @ Ty::I32 | other @ Ty::F32 | other @ Ty::AbstractInt | other @ Ty::VecN(_) | other @ Ty::Xform(..) | other @ Ty::Map(..) | other @ Ty::Row(_) | other @ Ty::Frame(_) | other @ Ty::Table(_) | other @ Ty::Verdict | other @ Ty::Struct(_) | other @ Ty::Array(..) | other @ Ty::Id(_) | other @ Ty::Bsdf => unreachable!("geo_space of {other}"),
    }
}

const EMIT_EVENT: &str = "emit_event";
const INSCATTER_JOINT: &str = "inscatter_joint";
const ATOMIC_ADD: &str = "atomic_add";
const GLOBAL_ID: &str = "global_id";
const WORKGROUP_BARRIER: &str = "workgroup_barrier";
const SUBGROUP_ADD: &str = "subgroup_add";
pub fn builtin_names() -> impl Iterator<Item = &'static str> {
    loft_builtins::BUILTINS.iter()
        .filter(|spec| spec.sig != SigRule::Poly(PolyRule::Unchecked))
        .map(|spec| spec.name)
        .chain([EMIT_EVENT, INSCATTER_JOINT, ATOMIC_ADD, GLOBAL_ID, WORKGROUP_BARRIER, SUBGROUP_ADD])
        .chain(crate::world::QUERY_ARITIES.iter().map(|&(name, _)| name))
}

const CORE_TYPES: &[(&str, Ty)] = &[
    ("f32", Ty::F32), ("u32", Ty::U32), ("i32", Ty::I32), ("bool", Ty::Bool),
    ("vec2f", Ty::VecN(2)), ("vec3f", Ty::VecN(3)), ("vec4f", Ty::VecN(4)),
    ("Bsdf", Ty::Bsdf), ("Verdict", Ty::Verdict),
];
const RADIANCE: &str = "Radiance";

pub fn builtin_types() -> impl Iterator<Item = &'static str> {
    CORE_TYPES.iter().map(|&(name, _)| name).chain([RADIANCE])
}

fn builtin_arity(spec: &BuiltinSpec, actual: usize, span: Span) -> Res<()> {
    let (min, max) = (usize::from(spec.min_arity), usize::from(spec.max_arity));
    if (min..=max).contains(&actual) {
        return Ok(());
    }
    let expected = if min == max { min.to_string() } else { format!("{min} to {max}") };
    Err(e003(span, format!("`{}` takes {expected} argument(s), got {actual}", spec.name), "match the signature"))
}

fn builtin_poly(name: &str, rule: PolyRule, ts: &[Ty], sp: &[Span], span: Span) -> Res<Ty> {
    use Geo::*;
    let same_space = |x: &Space, y: &Space| -> Res<()> {
        if x == y { Ok(()) } else { Err(e104(sp[1], format!("`{name}` of values in `{x}` and `{y}`"), "bring both into one space")) }
    };
    // Componentwise: every argument the same numeric type (a literal converts), result that type.
    let componentwise = || -> Res<Ty> {
        let mut t = Ty::AbstractInt;
        for (x, s) in ts.iter().zip(sp) {
            t = match (&t, x) {
                (_, Ty::Geo(..)) => {
                    return Err(e104(*s, format!("`{name}` is componentwise and drops spaces"), "write `.xyz` to drop the space explicitly"));
                }
                (Ty::AbstractInt, x) if num(x) || matches!(x, Ty::VecN(_)) => x.clone(),
                (t, x) if meet(t, x).is_some() => meet(t, x).expect("checked"),
                (Ty::VecN(_), x) if num(x) => t.clone(),
                (t, Ty::VecN(_)) if num(t) => x.clone(),
                _ => return Err(e003(*s, format!("`{name}` of `{t}` and `{x}`"), "use one numeric type")),
            };
        }
        Ok(if t == Ty::AbstractInt { Ty::F32 } else { t })
    };
    match rule {
        PolyRule::Place => match ts { [Ty::Geo(Pt,s)] | [Ty::Geo(Pt,s),Ty::F32|Ty::AbstractInt] => Ok(Ty::Xform(Space::Open,s.clone())),_=>Err(e003(span,"placement requires a point and optional yaw","write place([x,y,z]@World, yaw)")) },
        PolyRule::Direction => match ts { [Ty::Geo(_,s)]=>Ok(Ty::Geo(Dir,s.clone())), _=>Err(e003(span,"direction requires a space","supply a vector in the requested space")) },
        PolyRule::FrameAxes => match ts { [Ty::Geo(Pt,s),Ty::Geo(_,a),Ty::Geo(_,b),Ty::Geo(_,c)] if s==a&&s==b&&s==c=>Ok(Ty::Frame(s.clone())),_=>Err(e104(span,"frame axes occupy different spaces","use the origin and axes from one space")) },
        PolyRule::Componentwise => componentwise(),
        PolyRule::Zero => {
            Ok(match &ts[0] {
                Ty::Geo(_, s) => Ty::Geo(Geo::Vec, s.clone()),
                Ty::AbstractInt => Ty::I32,
                other => other.clone(),
            })
        },
        PolyRule::Displacement => match ts {
            [Ty::Geo(_, s)] => Ok(Ty::Geo(Geo::Vec, s.clone())),
            _ => Err(e003(span, "`loft_vec` takes a point or vector", "internal")),
        },
        PolyRule::Mix => match ts {
            [Ty::Geo(g, x), Ty::Geo(h, y), t] if g == h && num(t) => same_space(x, y).map(|_| ts[0].clone()),
            _ => componentwise(),
        },
        PolyRule::Select => {
            if ts[2] != Ty::Bool {
                return Err(e003(sp[2], "`select(f, t, cond)` needs a bool", "compare to get a bool"));
            }
            meet(&ts[0], &ts[1]).ok_or_else(|| {
                e003(span, "`select` arms differ", "give both arms one type")
            })
        },
        PolyRule::Length => {
            match &ts[0] {
                Ty::VecN(_) | Ty::F32 => Ok(Ty::F32),
                Ty::Geo(g, _) if vec_like(*g) => Ok(Ty::F32),
                Ty::Geo(Pt, s) => Err(e104(sp[0], format!("`length` of a point in `{s}`"), "subtract an origin: `length(p - o)`")),
                other @ Ty::Void | other @ Ty::Bool | other @ Ty::U32 | other @ Ty::I32 | other @ Ty::AbstractInt | other @ Ty::Geo(..) | other @ Ty::Xform(..) | other @ Ty::Map(..) | other @ Ty::Row(_) | other @ Ty::Frame(_) | other @ Ty::Table(_) | other @ Ty::Verdict | other @ Ty::Struct(_) | other @ Ty::Array(..) | other @ Ty::Id(_) | other @ Ty::Bsdf => Err(e003(sp[0], format!("`length` of `{other}`"), "pass a vector")),
            }
        },
        PolyRule::Distance => {
            match (&ts[0], &ts[1]) {
                (Ty::VecN(n), Ty::VecN(m)) if n == m => Ok(Ty::F32),
                (Ty::Geo(Pt, x), Ty::Geo(Pt, y)) => same_space(x, y).map(|_| Ty::F32),
                _ => Err(e003(span, "`distance` of two points (or two vectors)", "pass two Pt[S]")),
            }
        },
        PolyRule::Dot | PolyRule::Cross => {
            match (&ts[0], &ts[1]) {
                (Ty::VecN(n), Ty::VecN(m)) if n == m => Ok(if rule == PolyRule::Dot { Ty::F32 } else { Ty::VecN(3) }),
                (Ty::Geo(g, x), Ty::Geo(h, y)) if vec_like(*g) && vec_like(*h) => {
                    same_space(x, y).map(|_| if rule == PolyRule::Dot { Ty::F32 } else { Ty::Geo(Vec, x.clone()) })
                }
                // A space-less vector beside a vector takes its space, as in `+`: `dot(q, [0, 1, 0])`.
                (Ty::Geo(g, x), Ty::VecN(3)) | (Ty::VecN(3), Ty::Geo(g, x)) if vec_like(*g) => {
                    Ok(if rule == PolyRule::Dot { Ty::F32 } else { Ty::Geo(Vec, x.clone()) })
                }
                (Ty::Geo(Pt, s), _) | (_, Ty::Geo(Pt, s)) => {
                    Err(e104(span, format!("`{name}` of a point in `{s}`"), "subtract an origin to get a Vec"))
                }
                _ => Err(e003(span, format!("`{name}` of `{}` and `{}`", ts[0], ts[1]), "pass two vectors in one space")),
            }
        },
        PolyRule::Normalize => {
            match &ts[0] {
                Ty::VecN(n) => Ok(Ty::VecN(*n)),
                Ty::Geo(Norm, s) => Ok(Ty::Geo(Norm, s.clone())),
                Ty::Geo(g, s) if vec_like(*g) => Ok(Ty::Geo(Dir, s.clone())),
                other @ Ty::Void | other @ Ty::Bool | other @ Ty::U32 | other @ Ty::I32 | other @ Ty::F32 | other @ Ty::AbstractInt | other @ Ty::Geo(..) | other @ Ty::Xform(..) | other @ Ty::Map(..) | other @ Ty::Row(_) | other @ Ty::Frame(_) | other @ Ty::Table(_) | other @ Ty::Verdict | other @ Ty::Struct(_) | other @ Ty::Array(..) | other @ Ty::Id(_) | other @ Ty::Bsdf => Err(e104(sp[0], format!("`normalize` of `{other}`"), "normalize a Vec")),
            }
        },
        PolyRule::Inverse => match ts {
            [Ty::Xform(a, b)] => Ok(Ty::Xform(b.clone(), a.clone())),
            _ => Err(e003(span, "`inv` takes an Xform", "pass an `Xform[A -> B]`")),
        },
        PolyRule::VectorConstructor(n) => components(ts, n, span).map(|_| Ty::VecN(n)),
        PolyRule::TextureSample | PolyRule::ScanFrame | PolyRule::Unchecked => {
            unreachable!("only value signatures reach builtin_poly")
        }
    }
}

pub fn check_kernel_argument<'a>(
    p: &Prog<'a>, types: &mut Vec<Ty>, kernel: &'a KernelDecl, value: &'a Expr,
    ty: &TyExpr,
) -> Res<()> {
    let mut ck = Ck::new(p, types);
    ck.comptime = true;
    ck.kernel = Some(kernel);
    ck.scopes.push(HashMap::new());
    let expected = ck.resolve(ty, true)?;
    let actual = ck.expr(value)?;
    ck.assignable(&actual, &expected, value.span)?;
    ck.expect_unit(value, ck.declared_unit(ty))?;
    ck.set(value, expected);
    Ok(())
}
