//! The checker (draft §2A, §2B, §3): affine spaces (`Pt - Pt : Vec`, `Pt + Pt` is E104, a transform applies only in
//! its source space), generative map spaces with `.cell`/`.to_parent`/`.from_parent`, query signatures, and staging:
//! E401/E402 on kind parameters, E403 stage-0 statements outside `comptime fn`, E404 kind code reading stage 0.
//! Output: the type of every expression, by id.

use crate::Prog;
use loft_ir::ast::*;
use loft_ir::{Code, Diag, Res, Span};
use std::collections::HashMap;

#[derive(Clone)]
struct Local {
    ty: Ty,
    mutable: bool,
    /// For a `map` name: the parent space and whether a mirror stage gives it a `.cell`.
    map: Option<(Space, bool)>,
    span: Span,
}

struct Ck<'p, 'a> {
    p: &'p Prog<'a>,
    types: &'p mut Vec<Ty>,
    scopes: Vec<HashMap<String, Local>>,
    /// The fn being checked is stage 0.
    comptime: bool,
    ret: Ty,
    fresh: u32,
}

/// Kind parameter variants allowed per kind (draft §3: "E402: kind variant budget exceeded (12 > max 4)").
pub const VARIANT_BUDGET: u64 = 4;

pub fn check(p: &Prog, types: &mut Vec<Ty>) -> Res<()> {
    let mut ck = Ck { p, types, scopes: vec![], comptime: false, ret: Ty::Void, fresh: 0 };
    for c in p.consts.values() {
        ck.comptime = c.comptime;
        ck.scopes = vec![HashMap::new()];
        let t = ck.expr(&c.value)?;
        if let Some(ty) = &c.ty {
            let want = ck.resolve(ty, true)?;
            ck.assignable(&t, &want, c.value.span)?;
        }
    }
    for f in p.fns.values() {
        ck.func(f, None)?;
    }
    for k in p.kinds.values() {
        ck.kind(k)?;
    }
    Ok(())
}

/// Types one stage-0 expression (a schedule argument): it may read `comptime const`s and call any fn.
pub fn check_stage0_expr(p: &Prog, types: &mut Vec<Ty>, e: &Expr) -> Res<Ty> {
    let mut ck = Ck { p, types, scopes: vec![HashMap::new()], comptime: true, ret: Ty::Void, fresh: 0 };
    ck.expr(e)
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
fn e003(span: Span, msg: impl Into<String>, fix: impl Into<String>) -> Diag {
    Diag::new(Code::E003, span, msg, fix)
}

impl Ck<'_, '_> {
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
        let space = |s: &SpaceRef| -> Res<Space> {
            if s.var {
                if sig {
                    return Ok(Space::Var(s.name.clone()));
                }
                return match self.lookup(&s.name).and_then(|l| l.map.as_ref().map(|_| l.ty.clone())) {
                    Some(Ty::Geo(_, sp)) => Ok(sp),
                    _ => Err(Diag::new(
                        Code::E002,
                        s.span,
                        format!("no map `{}` in scope for the space `'{}`", s.name, s.name),
                        format!("introduce it with `map {} = ...;` before this line", s.name),
                    )),
                };
            }
            if self.p.spaces.contains(s.name.as_str()) {
                Ok(Space::Named(s.name.clone()))
            } else {
                Err(Diag::new(
                    Code::E002,
                    s.span,
                    format!("unknown space `{}`", s.name),
                    format!("declare it: `space {};`", s.name),
                ))
            }
        };
        match t {
            TyExpr::Name(i) => match i.name.as_str() {
                "f32" => Ok(Ty::F32),
                "u32" => Ok(Ty::U32),
                "i32" => Ok(Ty::I32),
                "bool" => Ok(Ty::Bool),
                "vec2f" => Ok(Ty::VecN(2)),
                "vec3f" => Ok(Ty::VecN(3)),
                "vec4f" => Ok(Ty::VecN(4)),
                n if self.p.units.contains(n) => Ok(Ty::F32),
                n => Err(Diag::new(
                    Code::E002,
                    i.span,
                    format!("unknown type `{n}`"),
                    "use f32, u32, i32, bool, vecN<f32>, a declared `unit`, Pt/Vec/Dir/Norm[S] or Xform[A -> B]",
                )),
            },
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
            TyExpr::Geo(g, s) => Ok(Ty::Geo(*g, space(s)?)),
            TyExpr::Xform(a, b) => Ok(Ty::Xform(space(a)?, space(b)?)),
            TyExpr::Field(s) => space(s).map(|_| Ty::F32),
        }
    }

    fn assignable(&self, from: &Ty, to: &Ty, span: Span) -> Res<()> {
        let ok = from == to
            || matches!((from, to), (Ty::AbstractInt, Ty::F32 | Ty::U32 | Ty::I32))
            || matches!((from, to), (Ty::Geo(g, a), Ty::Geo(Geo::Vec, b)) if vec_like(*g) && a == b);
        if ok {
            return Ok(());
        }
        Err(match (from, to) {
            (Ty::Geo(_, a), Ty::Geo(_, b)) if a != b => e104(
                span,
                format!("space mismatch: expected `{to}`, found `{from}`"),
                format!("move the value into `{b}` with a transform or a map's `to_parent`/`from_parent`"),
            ),
            (Ty::VecN(3), Ty::Geo(g, s)) => e104(
                span,
                format!("a space-less vec3 where `{to}` is expected"),
                format!("say which space it is in: `{g:?}[{s}](...)`"),
            ),
            _ => e003(span, format!("expected `{to}`, found `{from}`"), format!("convert the value to `{to}`")),
        })
    }

    fn func(&mut self, f: &FnDecl, kind: Option<&str>) -> Res<()> {
        self.comptime = f.comptime;
        self.scopes = vec![HashMap::new()];
        for prm in &f.params {
            let ty = match (&prm.ty, kind) {
                (Some(t), _) => self.resolve(t, true)?,
                (None, Some(k)) => Ty::Row(k.into()),
                (None, None) => {
                    return Err(e003(prm.name.span, "`self` outside a kind", "give the parameter a type"));
                }
            };
            self.bind(&prm.name.name, Local { ty, mutable: false, map: None, span: prm.name.span });
        }
        self.ret = match &f.ret {
            Some(t) => self.resolve(t, true)?,
            None => Ty::Void,
        };
        if let Some(a) = f.attrs.first() {
            self.attr(a, f)?;
        }
        self.block(&f.body)
    }

    /// `@lipschitz(param)` or `@lipschitz(max(a, b))`, naming the fn's own parameters.
    fn attr(&self, a: &Attr, f: &FnDecl) -> Res<()> {
        let is_param = |e: &Expr| matches!(&e.kind, ExprKind::Name(n) if f.params.iter().any(|p| &p.name.name == n));
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
                "Phase 0 knows one attribute: `@lipschitz(p)` or `@lipschitz(max(a, b))` over the fn's parameters",
                "remove the attribute, or name the parameter(s) the result is 1-Lipschitz in",
            ))
        }
    }

    fn kind(&mut self, k: &KindDecl) -> Res<()> {
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
                            "write `comptime n: u32 in 0u..4u`",
                        ));
                    }
                },
                (Ty::U32 | Ty::I32, None) => {
                    return Err(Diag::new(
                        Code::E402,
                        prm.span,
                        format!("`{}` has an unbounded domain", prm.name.name),
                        format!("bound it: `comptime {}: u32 in 0u..{VARIANT_BUDGET}u`", prm.name.name),
                    ));
                }
                _ => {
                    return Err(Diag::new(
                        Code::E401,
                        prm.span,
                        format!("continuous value cannot parameterize kind identity: `{}` is `{t}`", prm.name.name),
                        format!(
                            "move `{}: {t}` into the `row {{ ... }}` block so all instances share one compiled kind",
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
        for f in &k.row {
            let t = self.resolve(&f.ty, true)?;
            if t.words().is_none() {
                return Err(e003(f.ty.span(), format!("`{t}` cannot be stored in a row"), "use a scalar, vector, point or Xform"));
            }
        }
        let mut world: Option<(Space, Span)> = None;
        for (q, f) in &k.queries {
            self.func(f, Some(&k.name.name))?;
            let sig: Vec<Ty> =
                f.params.iter().skip(1).filter_map(|p| p.ty.as_ref().map(|t| self.resolve(t, true))).collect::<Res<_>>()?;
            let ret = f.ret.as_ref().map(|t| (t, self.resolve(t, true))).map(|(t, r)| (matches!(t, TyExpr::Field(_)), r));
            let (want, ok) = match q {
                QueryKind::Visible => (
                    "query visible(self, ro: Pt[S], rd: Dir[S]) -> f32",
                    matches!(sig.as_slice(), [Ty::Geo(Geo::Pt, a), Ty::Geo(Geo::Dir, b)] if a == b)
                        && matches!(ret, Some((false, Ok(Ty::F32)))),
                ),
                QueryKind::Trace => (
                    "query trace(self, p: Pt[S]) -> Field[T]",
                    matches!(sig.as_slice(), [Ty::Geo(Geo::Pt, _)]) && matches!(ret, Some((true, Ok(_)))),
                ),
                QueryKind::Material => (
                    "query material(self, p: Pt[S], n: Norm[S]) -> vec3<f32>",
                    matches!(sig.as_slice(), [Ty::Geo(Geo::Pt, a), Ty::Geo(Geo::Norm, b)] if a == b)
                        && matches!(ret, Some((false, Ok(Ty::VecN(3))))),
                ),
            };
            if !ok || f.params.first().is_none_or(|p| p.name.name != "self") {
                return Err(e003(f.name.span, format!("`{}` has the wrong signature", f.name.name), format!("write `{want}`")));
            }
            let Some(Ty::Geo(_, s)) = sig.first() else { unreachable!("checked above") };
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
        let has = |q| k.queries.iter().any(|(x, _)| *x == q);
        if !has(QueryKind::Trace) || !has(QueryKind::Material) {
            return Err(e003(
                k.name.span,
                format!("kind `{}` needs `query trace` and `query material` to be drawn and shaded", k.name.name),
                "add both; `visible` is optional (a closed-form hit replaces sphere tracing)",
            ));
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
                    t = want;
                }
                if t == Ty::AbstractInt {
                    t = Ty::I32;
                }
                self.bind(&name.name, Local { ty: t, mutable: *mutable, map: None, span: name.span });
            }
            Stmt::Assign { target, op: _, value } => {
                let ExprKind::Name(n) = &target.kind else {
                    return Err(e003(target.span, "Phase 0 assigns to names only", "assign the whole variable"));
                };
                let Some(l) = self.lookup(n).cloned() else {
                    return Err(Diag::new(Code::E002, target.span, format!("unknown name `{n}`"), "declare it with `var`"));
                };
                if !l.mutable {
                    return Err(e003(target.span, format!("`{n}` is a `let`"), format!("declare it with `var {n}`"))
                        .note(l.span, "declared here"));
                }
                self.set(target, l.ty.clone());
                let t = self.expr(value)?;
                self.assignable(&t, &l.ty, value.span)?;
            }
            Stmt::Map { name, head, stages } => self.map(name, head, stages)?,
            Stmt::Return(v, span) => {
                let t = match v {
                    Some(e) => self.expr(e)?,
                    None => Ty::Void,
                };
                let want = self.ret.clone();
                self.assignable(&t, &want, v.as_ref().map_or(*span, |e| e.span))?;
            }
            Stmt::If { cond, then, otherwise } => {
                let t = self.expr(cond)?;
                self.assignable(&t, &Ty::Bool, cond.span)?;
                self.block(then)?;
                if let Some(o) = otherwise {
                    self.block(o)?;
                }
            }
            Stmt::For { comptime, var, lo, hi, body, span } => {
                if *comptime && !self.comptime {
                    return Err(Diag::new(
                        Code::E403,
                        *span,
                        "`comptime for` is stage 0 and runs only inside a `comptime fn`",
                        "move the loop into a `comptime fn` that `emit`s rows, or drop `comptime` to keep a GPU loop",
                    ));
                }
                let a = self.expr(lo)?;
                let b = self.expr(hi)?;
                let t = match (&a, &b) {
                    (Ty::AbstractInt, Ty::AbstractInt) => Ty::I32,
                    (Ty::AbstractInt, t) | (t, Ty::AbstractInt) => t.clone(),
                    _ => a.clone(),
                };
                self.assignable(&a, &t, lo.span)?;
                self.assignable(&b, &t, hi.span)?;
                if !matches!(t, Ty::U32 | Ty::I32) {
                    return Err(e003(lo.span, "a range loop counts integers", "write `for i in 0u..n`"));
                }
                self.scopes.push(HashMap::new());
                self.bind(&var.name, Local { ty: t, mutable: false, map: None, span: var.span });
                self.block(body)?;
                self.scopes.pop();
            }
            Stmt::Emit { kind, fields, span } => {
                if !self.comptime {
                    return Err(Diag::new(
                        Code::E403,
                        *span,
                        "`emit` appends a row at stage 0 and runs only inside a `comptime fn`",
                        "call a `comptime fn` from the schedule's rows pass and emit there",
                    ));
                }
                let Some(k) = self.p.kinds.get(kind.name.as_str()) else {
                    return Err(Diag::new(Code::E002, kind.span, format!("unknown kind `{}`", kind.name), "declare the kind"));
                };
                for (f, e) in fields {
                    let Some(rf) = k.row.iter().find(|r| r.name.name == f.name) else {
                        return Err(Diag::new(
                            Code::E601,
                            f.span,
                            format!("`{}` has no row field `{}`", kind.name, f.name),
                            format!("fields are: {}", k.row.iter().map(|r| r.name.name.as_str()).collect::<Vec<_>>().join(", ")),
                        ));
                    };
                    let want = self.resolve(&rf.ty, true)?;
                    let t = self.expr(e)?;
                    self.assignable(&t, &want, e.span)?;
                }
                if let Some(missing) = k.row.iter().find(|r| !fields.iter().any(|(f, _)| f.name == r.name.name)) {
                    return Err(Diag::new(
                        Code::E601,
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
            ExprKind::Num { .. } | ExprKind::Bool(_) | ExprKind::Name(_) | ExprKind::Member(..) | ExprKind::Index(..) | ExprKind::Call { .. } | ExprKind::Ctor { .. } | ExprKind::Binary(..) | ExprKind::Unary(..) => self.expr(head)?,
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
                "rot_y" => &[Ty::F32],
                other => {
                    return Err(Diag::new(
                        Code::E002,
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
        Ok(self.set(e, t))
    }

    fn expr_inner(&mut self, e: &Expr) -> Res<Ty> {
        match &e.kind {
            ExprKind::Num { suffix, float, .. } => Ok(match suffix {
                NumSuffix::U => Ty::U32,
                NumSuffix::I => Ty::I32,
                NumSuffix::None if !float => Ty::AbstractInt,
                NumSuffix::None
                | NumSuffix::F
                | NumSuffix::M
                | NumSuffix::Rad
                | NumSuffix::Deg
                | NumSuffix::Sec
                | NumSuffix::Kg => Ty::F32,
            }),
            ExprKind::Bool(_) => Ok(Ty::Bool),
            ExprKind::Name(n) => {
                if let Some(l) = self.lookup(n) {
                    return Ok(l.ty.clone());
                }
                if let Some(c) = self.p.consts.get(n.as_str()) {
                    if c.comptime && !self.comptime {
                        return Err(Diag::new(
                            Code::E404,
                            e.span,
                            format!("kind code reads the stage-0 constant `{n}`"),
                            format!("pass `{n}` through a row field, or make it a plain `const` (it then fixes the shader)"),
                        )
                        .note(c.name.span, "declared `comptime` here"));
                    }
                    return Ok(match &c.ty {
                        Some(t) => self.resolve(t, true)?,
                        None => self.types.get(c.value.id as usize).cloned().unwrap_or(Ty::F32),
                    });
                }
                Err(Diag::new(Code::E002, e.span, format!("unknown name `{n}`"), "declare it before use"))
            }
            ExprKind::Member(base, m) => self.member(base, m),
            ExprKind::Index(base, i) => {
                let b = self.expr(base)?;
                let it = self.expr(i)?;
                self.assignable(&it, &Ty::U32, i.span).or_else(|_| self.assignable(&it, &Ty::I32, i.span))?;
                match b {
                    Ty::VecN(_) | Ty::Geo(..) => Ok(Ty::F32),
                    other @ Ty::Void | other @ Ty::Bool | other @ Ty::U32 | other @ Ty::I32 | other @ Ty::F32 | other @ Ty::AbstractInt | other @ Ty::Xform(..) | other @ Ty::Map(..) | other @ Ty::Row(_) => Err(e003(base.span, format!("cannot index `{other}`"), "index a vector")),
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
                binary(*op, &ta, &tb, a.span, b.span, e.span)
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
                let n = match &target {
                    Ty::VecN(n) => *n,
                    Ty::Geo(..) => 3,
                    other @ Ty::Void | other @ Ty::Bool | other @ Ty::U32 | other @ Ty::I32 | other @ Ty::F32 | other @ Ty::AbstractInt | other @ Ty::Xform(..) | other @ Ty::Map(..) | other @ Ty::Row(_) => return Err(e003(e.span, format!("cannot construct `{other}`"), "construct a vector or point")),
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
                    return Err(Diag::new(Code::E002, m.span, format!("map `{n}` has no `.cell`"), "only a mirror stage gives a cell")
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
                        Code::E002,
                        m.span,
                        format!("`{k}` has no row field `{}`", m.name),
                        format!("fields are: {}", kd.row.iter().map(|r| r.name.name.as_str()).collect::<Vec<_>>().join(", ")),
                    )),
                }
            }
            Ty::VecN(n) => swizzle(&m.name, *n as usize, m.span),
            Ty::Geo(..) => swizzle(&m.name, 3, m.span),
            other @ Ty::Void | other @ Ty::Bool | other @ Ty::U32 | other @ Ty::I32 | other @ Ty::F32 | other @ Ty::AbstractInt | other @ Ty::Xform(..) | other @ Ty::Map(..) => Err(e003(m.span, format!("`{other}` has no member `{}`", m.name), "access a row field or a swizzle")),
        }
    }

    fn call(&mut self, callee: &Ident, args: &[Arg], span: Span) -> Res<Ty> {
        if let Some(a) = args.iter().find_map(|a| a.name.as_ref()) {
            return Err(Diag::new(Code::E405, a.span, "named arguments are schedule-only in Phase 0", "pass arguments by position"));
        }
        let ts: Vec<Ty> = args.iter().map(|a| self.expr(&a.value)).collect::<Res<_>>()?;
        let sp: Vec<Span> = args.iter().map(|a| a.value.span).collect();
        if let Some(f) = self.p.fns.get(callee.name.as_str()) {
            if f.comptime && !self.comptime {
                return Err(Diag::new(
                    Code::E404,
                    callee.span,
                    format!("kind code calls the stage-0 fn `{}`", callee.name),
                    "call it from the schedule's rows pass, or drop `comptime` from it",
                )
                .note(f.name.span, "declared `comptime fn` here"));
            }
            if f.params.len() != ts.len() {
                return Err(e003(span, format!("`{}` takes {} argument(s), got {}", callee.name, f.params.len(), ts.len()), "match the signature")
                    .note(f.name.span, "declared here"));
            }
            let mut vars: HashMap<String, Space> = HashMap::new();
            for ((prm, t), s) in f.params.iter().zip(&ts).zip(&sp) {
                let want = self.resolve(prm.ty.as_ref().expect("free fns type every parameter"), true)?;
                let want = self.unify(&want, t, &mut vars, *s, &prm.name)?;
                self.assignable(t, &want, *s)?;
            }
            let ret = match &f.ret {
                Some(r) => self.resolve(r, true)?,
                None => Ty::Void,
            };
            return Ok(subst(&ret, &vars));
        }
        if MAP_OPS.contains(&callee.name.as_str()) {
            return Err(Diag::new(
                Code::E002,
                callee.span,
                format!("`{}` is a map stage", callee.name),
                format!("use it in a map statement: `map m = p |> {}(...);`", callee.name),
            ));
        }
        builtin(&callee.name, &ts, &sp, span)
    }

    /// Binds a signature's space variables from an argument; a clash is E104.
    fn unify(&self, want: &Ty, got: &Ty, vars: &mut HashMap<String, Space>, span: Span, prm: &Ident) -> Res<Ty> {
        let Ty::Geo(g, Space::Var(v)) = want else { return Ok(subst(want, vars)) };
        let Ty::Geo(_, s) = got else { return Ok(want.clone()) };
        match vars.get(v) {
            Some(bound) if bound != s => Err(e104(
                span,
                format!("`{}` is in `{s}` but `'{v}` is already `{bound}` in this call", prm.name),
                format!("pass every `'{v}` argument in `{bound}`, or move this one there"),
            )),
            Some(_) => Ok(Ty::Geo(*g, s.clone())),
            None => {
                vars.insert(v.clone(), s.clone());
                Ok(Ty::Geo(*g, s.clone()))
            }
        }
    }
}

pub const MAP_OPS: [&str; 3] = ["mirror_x", "translate", "rot_y"];

fn subst(t: &Ty, vars: &HashMap<String, Space>) -> Ty {
    let s = |x: &Space| match x {
        Space::Var(v) => vars.get(v).cloned().unwrap_or_else(|| x.clone()),
        Space::Named(_) | Space::Fresh(..) => x.clone(),
    };
    match t {
        Ty::Geo(g, x) => Ty::Geo(*g, s(x)),
        Ty::Xform(a, b) => Ty::Xform(s(a), s(b)),
        other @ Ty::Void | other @ Ty::Bool | other @ Ty::U32 | other @ Ty::I32 | other @ Ty::F32 | other @ Ty::AbstractInt | other @ Ty::VecN(_) | other @ Ty::Map(..) | other @ Ty::Row(_) => other.clone(),
    }
}

fn swizzle(m: &str, n: usize, span: Span) -> Res<Ty> {
    match swizzle_len(m, n) {
        Some(1) => Ok(Ty::F32),
        Some(k) => Ok(Ty::VecN(k as u8)),
        None => Err(Diag::new(Code::E002, span, format!("no member `{m}`"), "use a swizzle like .x or .xz")),
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
            other @ Ty::Void | other @ Ty::Bool | other @ Ty::U32 | other @ Ty::I32 | other @ Ty::Xform(..) | other @ Ty::Map(..) | other @ Ty::Row(_) => return Err(e003(span, format!("cannot build a vector from `{other}`"), "pass f32 components")),
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

fn binary(op: BinOp, a: &Ty, b: &Ty, sa: Span, sb: Span, span: Span) -> Res<Ty> {
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
        (Add | Sub | Mul | Div, Ty::VecN(n), Ty::VecN(m)) if n == m => Ok(a.clone()),
        (Mul | Div, Ty::VecN(_), s) if num(s) => Ok(a.clone()),
        (Mul, s, Ty::VecN(_)) if num(s) => Ok(b.clone()),
        // Affine rules (draft §2A).
        (Sub, Ty::Geo(Pt, x), Ty::Geo(Pt, y)) => if x == y { Ok(Ty::Geo(Vec, x.clone())) } else { Err(mismatch(x, y)) },
        (Add | Sub, Ty::Geo(Pt, x), Ty::Geo(g, y)) if vec_like(*g) => if x == y { Ok(a.clone()) } else { Err(mismatch(x, y)) },
        (Add, Ty::Geo(g, x), Ty::Geo(Pt, y)) if vec_like(*g) => if x == y { Ok(b.clone()) } else { Err(mismatch(x, y)) },
        (Add, Ty::Geo(Pt, _), Ty::Geo(Pt, _)) => Err(e104(
            span,
            "`Pt + Pt` has no meaning: points do not add",
            "subtract an origin from one side (`Pt - Pt` is a Vec), or use `mix(p, q, t)`",
        )),
        (Add | Sub, Ty::Geo(g, x), Ty::Geo(h, y)) if vec_like(*g) && vec_like(*h) => {
            if x == y { Ok(Ty::Geo(Vec, x.clone())) } else { Err(mismatch(x, y)) }
        }
        (Mul | Div, Ty::Geo(g, _), s) if vec_like(*g) && num(s) => Ok(Ty::Geo(Vec, geo_space(a))),
        (Mul, s, Ty::Geo(g, _)) if vec_like(*g) && num(s) => Ok(Ty::Geo(Vec, geo_space(b))),
        (Mul | Div, Ty::Geo(g, _), Ty::VecN(3)) if vec_like(*g) => Ok(Ty::Geo(Vec, geo_space(a))),
        (Mul | Div, Ty::Geo(Pt, _), s) if num(s) || *s == Ty::VecN(3) => Err(e104(
            sa,
            "a point has no scale",
            "subtract an origin first: `(p - o) * s` is a Vec; add it back with `o + ...`",
        )),
        (Mul, Ty::Xform(x, y), Ty::Geo(g, s)) | (Mul, Ty::Map(x, y), Ty::Geo(g, s)) => {
            if x == s {
                Ok(Ty::Geo(*g, y.clone()))
            } else {
                let fix = match (a, s) {
                    (Ty::Xform(..), Space::Fresh(m, _)) => format!(
                        "lift the value out of `'{m}` first: `{} * ({m}.to_parent * value)` inverts the map's offset and mirror",
                        "xform"
                    ),
                    _ => format!("the transform takes `{x}`; bring the value into `{x}` first"),
                };
                Err(e104(sb, format!("space mismatch in transform application: expected `{g:?}[{x}]`, found `{b}`"), fix))
            }
        }
        (Add | Sub, Ty::Geo(_, s), Ty::VecN(3)) | (Add | Sub, Ty::VecN(3), Ty::Geo(_, s)) => Err(e104(
            span,
            format!("a space-less vec3 combined with a value in `{s}`"),
            format!("say which space the vector is in: `Vec[{s}](...)`"),
        )),
        _ => Err(e003(span, format!("no operator `{}` for `{a}` and `{b}`", op.wgsl()), "convert the operands")),
    }
}

fn geo_space(t: &Ty) -> Space {
    match t {
        Ty::Geo(_, s) => s.clone(),
        other @ Ty::Void | other @ Ty::Bool | other @ Ty::U32 | other @ Ty::I32 | other @ Ty::F32 | other @ Ty::AbstractInt | other @ Ty::VecN(_) | other @ Ty::Xform(..) | other @ Ty::Map(..) | other @ Ty::Row(_) => unreachable!("geo_space of {other}"),
    }
}

/// Builtin signatures. Componentwise maths on f32/vecN; geometry functions keep or check spaces.
fn builtin(name: &str, ts: &[Ty], sp: &[Span], span: Span) -> Res<Ty> {
    use Geo::*;
    let arity = |n: usize| -> Res<()> {
        if ts.len() == n { Ok(()) } else { Err(e003(span, format!("`{name}` takes {n} argument(s), got {}", ts.len()), "match the signature")) }
    };
    let same_space = |x: &Space, y: &Space| -> Res<()> {
        if x == y { Ok(()) } else { Err(e104(sp[1], format!("`{name}` of values in `{x}` and `{y}`"), "bring both into one space")) }
    };
    // Componentwise: every argument the same numeric type (a literal converts), result that type.
    let componentwise = |n: usize| -> Res<Ty> {
        arity(n)?;
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
    match name {
        "abs" | "sqrt" | "sin" | "cos" | "exp" | "floor" | "fract" | "sign" | "saturate" => componentwise(1),
        "min" | "max" | "pow" | "step" => componentwise(2),
        "clamp" | "smoothstep" => componentwise(3),
        "mix" => match ts {
            [Ty::Geo(g, x), Ty::Geo(h, y), t] if g == h && num(t) => same_space(x, y).map(|_| ts[0].clone()),
            _ => componentwise(3),
        },
        "select" => {
            arity(3)?;
            if ts[2] != Ty::Bool {
                return Err(e003(sp[2], "`select(f, t, cond)` needs a bool", "compare to get a bool"));
            }
            meet(&ts[0], &ts[1]).or_else(|| (ts[0] == ts[1]).then(|| ts[0].clone())).ok_or_else(|| {
                e003(span, "`select` arms differ", "give both arms one type")
            })
        }
        "length" => {
            arity(1)?;
            match &ts[0] {
                Ty::VecN(_) | Ty::F32 => Ok(Ty::F32),
                Ty::Geo(g, _) if vec_like(*g) => Ok(Ty::F32),
                Ty::Geo(Pt, s) => Err(e104(sp[0], format!("`length` of a point in `{s}`"), "subtract an origin: `length(p - o)`")),
                other @ Ty::Void | other @ Ty::Bool | other @ Ty::U32 | other @ Ty::I32 | other @ Ty::AbstractInt | other @ Ty::Geo(..) | other @ Ty::Xform(..) | other @ Ty::Map(..) | other @ Ty::Row(_) => Err(e003(sp[0], format!("`length` of `{other}`"), "pass a vector")),
            }
        }
        "distance" => {
            arity(2)?;
            match (&ts[0], &ts[1]) {
                (Ty::VecN(n), Ty::VecN(m)) if n == m => Ok(Ty::F32),
                (Ty::Geo(Pt, x), Ty::Geo(Pt, y)) => same_space(x, y).map(|_| Ty::F32),
                _ => Err(e003(span, "`distance` of two points (or two vectors)", "pass two Pt[S]")),
            }
        }
        "dot" | "cross" => {
            arity(2)?;
            match (&ts[0], &ts[1]) {
                (Ty::VecN(n), Ty::VecN(m)) if n == m => Ok(if name == "dot" { Ty::F32 } else { Ty::VecN(3) }),
                (Ty::Geo(g, x), Ty::Geo(h, y)) if vec_like(*g) && vec_like(*h) => {
                    same_space(x, y).map(|_| if name == "dot" { Ty::F32 } else { Ty::Geo(Vec, x.clone()) })
                }
                (Ty::Geo(Pt, s), _) | (_, Ty::Geo(Pt, s)) => {
                    Err(e104(span, format!("`{name}` of a point in `{s}`"), "subtract an origin to get a Vec"))
                }
                _ => Err(e003(span, format!("`{name}` of `{}` and `{}`", ts[0], ts[1]), "pass two vectors in one space")),
            }
        }
        "normalize" => {
            arity(1)?;
            match &ts[0] {
                Ty::VecN(n) => Ok(Ty::VecN(*n)),
                Ty::Geo(Norm, s) => Ok(Ty::Geo(Norm, s.clone())),
                Ty::Geo(g, s) if vec_like(*g) => Ok(Ty::Geo(Dir, s.clone())),
                other @ Ty::Void | other @ Ty::Bool | other @ Ty::U32 | other @ Ty::I32 | other @ Ty::F32 | other @ Ty::AbstractInt | other @ Ty::Geo(..) | other @ Ty::Xform(..) | other @ Ty::Map(..) | other @ Ty::Row(_) => Err(e104(sp[0], format!("`normalize` of `{other}`"), "normalize a Vec")),
            }
        }
        "inv" => match ts {
            [Ty::Xform(a, b)] => Ok(Ty::Xform(b.clone(), a.clone())),
            _ => Err(e003(span, "`inv` takes an Xform", "pass an `Xform[A -> B]`")),
        },
        "f32" | "u32" | "i32" => {
            arity(1)?;
            if !num(&ts[0]) {
                return Err(e003(sp[0], format!("`{name}` of `{}`", ts[0]), "convert a scalar"));
            }
            Ok(match name {
                "f32" => Ty::F32,
                "u32" => Ty::U32,
                _ => Ty::I32,
            })
        }
        "vec2" | "vec3" | "vec4" | "vec2f" | "vec3f" | "vec4f" => {
            let n = name.as_bytes()[3] - b'0';
            components(ts, n, span).map(|_| Ty::VecN(n))
        }
        _ => Err(Diag::new(Code::E002, span, format!("unknown function `{name}`"), "declare it with `fn`, or check the spelling")),
    }
}
