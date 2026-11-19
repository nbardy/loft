//! One tree-walking evaluator over the checked source, generic in its scalar. With `f32` it is stage 0 (comptime
//! rows) and the CPU twin; with `Iv` (interval + Lipschitz, `lip.rs`) it derives bounds and L. Control flow is
//! shared; a branch the domain cannot decide is an error, never a guess.

use crate::Prog;
use loft_parse::ast::*;
use loft_ir::{Code, Diag, Res, Span};

/// A scalar domain. `lip` is the scalar's Lipschitz bound in the traced point (always 0 for `f32`).
pub trait Scalar: Clone + std::fmt::Debug {
    fn c(x: f64) -> Self;
    fn frame_time()->Self;
    fn add(&self, o: &Self) -> Self;
    fn sub(&self, o: &Self) -> Self;
    fn mul(&self, o: &Self) -> Self;
    fn div(&self, o: &Self) -> Self;
    fn neg(&self) -> Self;
    fn un(&self, f: Un) -> Self;
    fn min(&self, o: &Self) -> Self;
    fn max(&self, o: &Self) -> Self;
    fn pow(&self, o: &Self) -> Self;
    /// `Some` when decided for every point.
    fn cmp(&self, op: BinOp, o: &Self) -> Option<bool>;
    /// Largest |x|.
    fn mag(&self) -> f64;
    /// Smallest |x| (0 when the value may be 0).
    fn mig(&self) -> f64;
    fn lip(&self) -> f64;
    fn with_lip(self, l: f64) -> Self;
    /// The single value when known exactly (comptime and the twin).
    fn exact(&self) -> Option<f64>;
    /// Lowest and highest value.
    fn range(&self) -> (f64, f64);
    /// Union of two branches (`select` on an undecided bool).
    fn join(&self, o: &Self) -> Self;
}

#[derive(Clone, Copy, Debug)]
pub enum Un {
    Sin,
    Cos,
    Sqrt,
    Abs,
    Exp,
    Floor,
    Fract,
    Sign,
}

/// A rigid transform: `p -> r * p + t`, `r` row-major. Always rigid: the only constructor is origin + yaw.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rigid {
    pub r: [[f32; 3]; 3],
    pub t: [f32; 3],
}

impl Rigid {
    pub fn yaw(origin: [f32; 3], yaw: f32) -> Rigid {
        let (s, c) = yaw.sin_cos();
        Rigid { r: [[c, 0.0, s], [0.0, 1.0, 0.0], [-s, 0.0, c]], t: origin }
    }
    /// `self * o`: apply `o`, then `self`.
    pub fn then(&self, o: &Rigid) -> Rigid {
        let (a, b) = (self.r, o.r);
        let r = [0, 1, 2].map(|i| [0, 1, 2].map(|j| (0..3).map(|k| a[i][k] * b[k][j]).sum()));
        let t = [0, 1, 2].map(|i| (0..3).map(|k| a[i][k] * o.t[k]).sum::<f32>() + self.t[i]);
        Rigid { r, t }
    }
    pub fn inv(&self) -> Rigid {
        let r = self.r;
        let rt = [[r[0][0], r[1][0], r[2][0]], [r[0][1], r[1][1], r[2][1]], [r[0][2], r[1][2], r[2][2]]];
        let t = [0, 1, 2].map(|i| -(rt[i][0] * self.t[0] + rt[i][1] * self.t[1] + rt[i][2] * self.t[2]));
        Rigid { r: rt, t }
    }
}

/// A concrete row value (stage 0 output, row-table input).
#[derive(Clone, Debug, PartialEq)]
pub enum RowVal {
    F(f32),
    V(Vec<f32>),
    U(u32),
    I(i32),
    B(bool),
    X(Rigid),
}

#[derive(Clone, Debug)]
pub enum Val<S> {
    Void,
    S(S),
    /// Components and the vector's Euclidean Lipschitz bound.
    V(Vec<S>, f64),
    B(Option<bool>),
    U(u32),
    I(i32),
    X(Rigid),
    Row(usize, usize),
    /// `m.to_parent` (true) or `m.from_parent` (false), applied by `*`.
    MapRef(String, bool),
    /// Lobes bottom first: the `@lobe` fn's name and the arguments bound at material time.
    Bsdf(Vec<(String, Vec<Val<S>>)>),
}

/// One baked scan: its parameter range and 12 words per frame (origin, tangent, normal, binormal).
#[derive(Clone, Debug)]
pub struct Lut {
    pub name: String,
    pub lo: f32,
    pub hi: f32,
    pub frames: Vec<[f32; 12]>,
}

/// Uniform cubic B-spline basis at `f` in [0, 1) (C2), and its derivative in `f`.
pub fn bspline(f: f64) -> ([f64; 4], [f64; 4]) {
    let g = 1.0 - f;
    (
        [g * g * g / 6.0, (3.0 * f * f * f - 6.0 * f * f + 4.0) / 6.0, (-3.0 * f * f * f + 3.0 * f * f + 3.0 * f + 1.0) / 6.0, f * f * f / 6.0],
        [-g * g / 2.0, (3.0 * f * f - 4.0 * f) / 2.0, (-3.0 * f * f + 2.0 * f + 1.0) / 2.0, f * f / 2.0],
    )
}

impl Lut {
    /// The frame at `t` (or its t-derivative), as 12 components. A known `t` is exact; a range of `t` gives the
    /// hull of the control points it touches (a B-spline lies in its control polygon's hull; its derivative in the
    /// hull of the scaled differences).
    pub fn sample<S: Scalar>(&self, t: &S, deriv: bool) -> Vec<S> {
        let n = self.frames.len();
        let scale = (n - 1) as f64 / f64::from(self.hi - self.lo);
        let at = |k: i64| &self.frames[k.clamp(0, n as i64 - 1) as usize];
        let (lo, hi) = t.range();
        let (s0, s1) = ((lo - f64::from(self.lo)) * scale, (hi - f64::from(self.lo)) * scale);
        if s0 == s1 {
            let i = s0.floor().clamp(0.0, (n - 1) as f64);
            let (w, dw) = bspline(s0 - i);
            let w = if deriv { dw.map(|x| x * scale) } else { w };
            return (0..12).map(|c| S::c((0..4).map(|k| w[k] * f64::from(at(i as i64 - 1 + k as i64)[c])).sum())).collect();
        }
        let (a, b) = (s0.floor() as i64 - 1, s1.floor() as i64 + 2);
        (0..12)
            .map(|c| {
                let vals: Vec<f64> = if deriv {
                    (a..b).map(|k| (f64::from(at(k + 1)[c]) - f64::from(at(k)[c])) * scale).collect()
                } else {
                    (a..=b).map(|k| f64::from(at(k)[c])).collect()
                };
                let (mn, mx) = vals.iter().fold((f64::INFINITY, f64::NEG_INFINITY), |(m, x), v| (m.min(*v), x.max(*v)));
                S::c(mn).join(&S::c(mx))
            })
            .collect()
    }
}

#[derive(Clone, Debug)]
enum Stage<S> {
    Mirror,
    Translate(Vec<S>, f64),
    /// Rotation about axis 0 (x), 1 (y) or 2 (z).
    Rot(usize, S),
}

#[derive(Clone, Debug)]
enum Slot<S> {
    Val(Val<S>),
    Map { p: Val<S>, cell: Option<S>, stages: Vec<Stage<S>> },
}

enum Flow<S> {
    Next,
    Ret(Val<S>),
}

/// Rows emitted at stage 0, per kind (declaration order).
pub type Tables = Vec<Vec<Vec<RowVal>>>;

pub struct Eval<'p, 'a, S> {
    pub p: &'p Prog<'a>,
    pub types: &'p [Ty],
    pub tables: Tables,
    pub overrides: &'p [(String, f64)],
    /// Names borrow the source: no allocation per binding.
    env: Vec<(&'a str, Slot<S>)>,
    /// The first expression that made a Lipschitz bound unbounded: where, and why.
    pub cause: Option<(Span, String)>,
    pub steps: u64,
    pub time: S,
    /// Baked scans, for `sample_c2`.
    pub luts: &'p [Lut],
}

fn err(span: Span, msg: impl Into<String>, fix: impl Into<String>) -> Diag {
    Diag::new(Code::E701, span, msg, fix)
}

/// Euclidean norm bound of a vector's components.
fn norm<S: Scalar>(v: &[S]) -> f64 {
    v.iter().map(|x| x.mag() * x.mag()).sum::<f64>().sqrt()
}

impl<'p, 'a, S: Scalar> Eval<'p, 'a, S> {
    pub fn new(p: &'p Prog<'a>, types: &'p [Ty], tables: Tables, overrides: &'p [(String, f64)]) -> Self {
        Eval { p, types, tables, overrides, env: vec![], cause: None, steps: 0, time:S::frame_time(), luts: &[] }
    }

    /// Binds a name for the expressions evaluated next (a scan's parameter); `unbind` drops the latest.
    pub fn bind(&mut self, name: &'a str, v: Val<S>) {
        self.env.push((name, Slot::Val(v)));
    }
    pub fn unbind(&mut self) {
        self.env.pop();
    }

    pub fn lift(r: &RowVal) -> Val<S> {
        match r {
            RowVal::F(x) => Val::S(S::c(f64::from(*x))),
            RowVal::V(v) => Val::V(v.iter().map(|x| S::c(f64::from(*x))).collect(), 0.0),
            RowVal::U(u) => Val::U(*u),
            RowVal::I(i) => Val::I(*i),
            RowVal::B(b) => Val::B(Some(*b)),
            RowVal::X(x) => Val::X(*x),
        }
    }

    /// Calls `f` with arguments; a query's `self` is `Val::Row`.
    pub fn call_fn(&mut self, f: &'a FnDecl, args: Vec<Val<S>>, span: Span) -> Res<Val<S>> {
        if f.raw.is_some(){return Err(Diag::new(Code::E405,span,"raw WGSL has no CPU evaluation","use a Loft expression for build-time evaluation or CPU twin queries"))}
        let mark = self.env.len();
        let saved = std::mem::take(&mut self.env);
        for (prm, a) in f.params.iter().zip(args.iter()) {
            self.env.push((prm.name.name.as_str(), Slot::Val(a.clone())));
        }
        let out = self.block(&f.body);
        self.env = saved;
        debug_assert_eq!(self.env.len(), mark);
        let mut v = match out? {
            Flow::Ret(v) => v,
            Flow::Next => Val::Void,
        };
        if let Some(a) = f.attrs.iter().find(|a| a.name.name == "lipschitz") {
            v = self.axiom(a, f, &args, v, span);
        }
        Ok(v)
    }

    /// `@lipschitz(p)` / `@lipschitz(max(a, b))`: the result's L is the named parameters' L; any other parameter
    /// that varies with the point voids the axiom (L unbounded, with the reason).
    fn axiom(&mut self, a: &Attr, f: &FnDecl, args: &[Val<S>], v: Val<S>, span: Span) -> Val<S> {
        let names: Vec<&str> = match &a.args[0].kind {
            ExprKind::Name(n) => vec![n],
            ExprKind::Call { args, .. } => {
                args.iter().filter_map(|x| if let ExprKind::Name(n) = &x.value.kind { Some(n.as_str()) } else { None }).collect()
            }
            ExprKind::If { .. } | ExprKind::Num { .. } | ExprKind::Bool(_) | ExprKind::Member(..) | ExprKind::Index(..) | ExprKind::Ctor { .. } | ExprKind::Binary(..) | ExprKind::Unary(..) => vec![],
        };
        let lip_of = |x: &Val<S>| match x {
            Val::S(s) => s.lip(),
            Val::V(_, l) => *l,
            Val::Void | Val::B(_) | Val::U(_) | Val::I(_) | Val::X(_) | Val::Row(..) | Val::MapRef(..) | Val::Bsdf(_) => 0.0,
        };
        let mut l = 0.0f64;
        for (prm, x) in f.params.iter().zip(args) {
            if names.contains(&prm.name.name.as_str()) {
                l = l.max(lip_of(x));
            } else if lip_of(x) > 0.0 {
                if self.cause.is_none() {
                    self.cause = Some((
                        span,
                        format!("argument `{}` of `{}` varies with the point; its axiom covers only {names:?}", prm.name.name, f.name.name),
                    ));
                }
                l = f64::INFINITY;
            }
        }
        match v {
            Val::S(s) => Val::S(s.with_lip(l)),
            other @ Val::Void | other @ Val::V(..) | other @ Val::B(_) | other @ Val::U(_) | other @ Val::I(_) | other @ Val::X(_) | other @ Val::Row(..) | other @ Val::MapRef(..) | other @ Val::Bsdf(_) => other,
        }
    }

    fn lookup(&self, n: &str) -> Option<&Slot<S>> {
        self.env.iter().rev().find(|(k, _)| *k == n).map(|(_, v)| v)
    }

    fn block(&mut self, b: &'a Block) -> Res<Flow<S>> {
        let mark = self.env.len();
        for s in &b.stmts {
            if let Flow::Ret(v) = self.stmt(s)? {
                self.env.truncate(mark);
                return Ok(Flow::Ret(v));
            }
        }
        self.env.truncate(mark);
        Ok(Flow::Next)
    }

    fn stmt(&mut self, s: &'a Stmt) -> Res<Flow<S>> {
        self.steps += 1;
        match s {
            Stmt::Let { name, value, .. } => {
                let v = self.expr(value)?;
                self.env.push((name.name.as_str(), Slot::Val(v)));
            }
            Stmt::Assign { target, op, value } => {
                let ExprKind::Name(n) = &target.kind else { unreachable!("checked: names only") };
                let mut v = self.expr(value)?;
                if let Some(op) = op {
                    let old = self.expr(target)?;
                    v = self.binary(*op, old, v, target, value, value.span)?;
                }
                let slot = self.env.iter_mut().rev().find(|(k, _)| k == n).expect("checked: declared");
                slot.1 = Slot::Val(v);
            }
            Stmt::Map { name, head, stages } => {
                let (point, mut all) = match &head.kind {
                    ExprKind::Call { callee, args } if crate::check::MAP_OPS.contains(&callee.name.as_str()) => {
                        (self.expr(&args[0].value)?, vec![(callee.name.as_str(), vec![])])
                    }
                    ExprKind::If { .. } | ExprKind::Num { .. } | ExprKind::Bool(_) | ExprKind::Name(_) | ExprKind::Member(..) | ExprKind::Index(..) | ExprKind::Call { .. } | ExprKind::Ctor { .. } | ExprKind::Binary(..) | ExprKind::Unary(..) => (self.expr(head)?, vec![]),
                };
                for (op, args) in stages {
                    let vals = args.iter().map(|a| self.expr(a)).collect::<Res<Vec<_>>>()?;
                    all.push((op.name.as_str(), vals));
                }
                let (mut p, mut cell, mut out) = (point, None, vec![]);
                for (op, vals) in all {
                    let stage = match (op, vals.as_slice()) {
                        ("mirror_x", []) => Stage::Mirror,
                        ("translate", [Val::V(t, l)]) => Stage::Translate(t.clone(), *l),
                        ("rot_x", [Val::S(a)]) => Stage::Rot(0, a.clone()),
                        ("rot_y", [Val::S(a)]) => Stage::Rot(1, a.clone()),
                        ("rot_z", [Val::S(a)]) => Stage::Rot(2, a.clone()),
                        _ => unreachable!("checked map stage {op}"),
                    };
                    if let (Stage::Mirror, Val::V(c, _)) = (&stage, &p) {
                        cell = Some(sign_cell(&c[0]));
                    }
                    p = self.stage(&stage, p, cell.as_ref(), true, true, true, head.span);
                    out.push(stage);
                }
                self.env.push((name.name.as_str(), Slot::Map { p, cell, stages: out }));
            }
            Stmt::Return(v, _) => {
                return Ok(Flow::Ret(match v {
                    Some(e) => self.expr(e)?,
                    None => Val::Void,
                }));
            }
            Stmt::If { cond, then, otherwise } => {
                let c = self.expr(cond)?;
                let Val::B(Some(b)) = c else {
                    return Err(err(
                        cond.span,
                        "this branch depends on the point and cannot be decided over a region",
                        "Phase 0 derives bounds for straight-line fields; write the branch with min/max/select",
                    ));
                };
                let blk = if b { Some(then) } else { otherwise.as_ref() };
                if let Some(blk) = blk {
                    return self.block(blk);
                }
            }
            Stmt::Each { var, values, body, .. }=>{
                for e in values{let value=self.expr(e)?; self.bind(&var.name,value); let flow=self.block(body)?;self.unbind();if let Flow::Ret(_)=flow{return Ok(flow)}}
            }
            Stmt::For { var, lo, hi, body, .. } => {
                let (a, b) = (self.expr(lo)?, self.expr(hi)?);
                let (a, b, unsigned) = match (a, b) {
                    (Val::U(a), Val::U(b)) => (i64::from(a), i64::from(b), true),
                    (Val::I(a), Val::I(b)) => (i64::from(a), i64::from(b), false),
                    (Val::U(a), Val::I(b)) | (Val::I(b), Val::U(a)) => (i64::from(a), i64::from(b), true),
                    _ => return Err(err(lo.span, "loop bounds are not known integers", "bound the loop with integers")),
                };
                for i in a..b {
                    let v = if unsigned { Val::U(i as u32) } else { Val::I(i as i32) };
                    self.env.push((var.name.as_str(), Slot::Val(v)));
                    let f = self.block(body)?;
                    self.env.pop();
                    if let Flow::Ret(v) = f {
                        return Ok(Flow::Ret(v));
                    }
                }
            }
            Stmt::Emit { kind, fields, bind, .. } => {
                let k = self.p.kind_index(&kind.name);
                let decl = self.p.kinds[kind.name.as_str()];
                let mut row = vec![];
                for rf in &decl.row {
                    let supplied = fields.iter().find(|(f, _)| f.name == rf.name.name);
                    let (e, v) = if let Some((_, e)) = supplied {
                        (e, self.expr(e)?)
                    } else {
                        let e = rf.default.as_ref().expect("checked required field");
                        (e, self.module_expr(e)?)
                    };
                    let v=concrete(&v,e.span)?;
                    if let Some(range)=&rf.range {self.validate_range(&rf.name,range,crate::as_f32(v.clone(),e.span)?,e.span)?;}
                    row.push(v);
                }
                if let Some(n)=bind{self.bind(&n.name,Val::U(self.tables[k].len() as u32));}
                self.tables[k].push(row);
            }
            Stmt::Expr(e) => {
                self.expr(e)?;
            }
        }
        Ok(Flow::Next)
    }

    /// Defaults, refinements and module constants cannot see the emitting caller's locals.
    pub(crate) fn module_expr(&mut self, e: &'a Expr) -> Res<Val<S>> {
        let saved = std::mem::take(&mut self.env);
        let result = self.expr(e);
        self.env = saved;
        result
    }

    pub(crate) fn validate_range(&mut self,name:&Ident,r:&'a RowRange,x:f32,sp:Span)->Res<()>{
        let lo=crate::as_f32(concrete(&self.module_expr(&r.lo)?,r.lo.span)?,r.lo.span)?;
        let hi=match &r.hi{Some(e)=>crate::as_f32(concrete(&self.module_expr(e)?,e.span)?,e.span)?,None=>f32::INFINITY};
        if !x.is_finite() || !(if r.strict{x>lo}else{x>=lo}) || !(x<=hi){
            return Err(Diag::new(Code::E602,sp,format!("{} = {x} outside declared range",name.name),format!("supply a value {} {lo} and <= {hi}",if r.strict{">"}else{">="})).note(r.lo.span,"declared here"))
        }
        Ok(())
    }

    /// One map stage, forward (`fwd`) or inverse, on a point or a direction. `fold`: the map statement itself,
    /// where a mirror folds (`abs`); `to_parent`/`from_parent` instead multiply by the recorded cell.
    #[allow(clippy::too_many_arguments)]
    fn stage(&mut self, st: &Stage<S>, x: Val<S>, cell: Option<&S>, fwd: bool, point: bool, fold: bool, span: Span) -> Val<S> {
        let Val::V(c, l) = x else { unreachable!("checked: maps act on vec3") };
        match st {
            Stage::Mirror => {
                let x0 = match (fold, cell) {
                    (true, _) | (false, None) => c[0].un(Un::Abs),
                    (false, Some(s)) => c[0].mul(s),
                };
                Val::V(vec![x0, c[1].clone(), c[2].clone()], l)
            }
            Stage::Translate(t, lt) if point => {
                let out = c.iter().zip(t).map(|(a, b)| if fwd { a.add(b) } else { a.sub(b) }).collect();
                Val::V(out, l + lt)
            }
            Stage::Translate(..) => Val::V(c, l),
            Stage::Rot(axis, a) => {
                let a = if fwd { a.clone() } else { a.neg() };
                if a.lip() > 0.0 && self.cause.is_none() {
                    self.cause = Some((span, "a rotation angle that varies with the point".into()));
                }
                // The plane (i, j) turns by a: i' = cos i + sin j, j' = cos j - sin i (matches loft_rot_* in emit).
                let (i, j) = match axis {
                    0 => (1, 2),
                    // Match loft_rot_y in WGSL: the inverse of +90° takes +Z to −X.
                    1 => (0, 2),
                    _ => (0, 1),
                };
                let (s, co) = (a.un(Un::Sin), a.un(Un::Cos));
                let mut out = c.clone();
                out[i] = co.mul(&c[i]).add(&s.mul(&c[j]));
                out[j] = co.mul(&c[j]).sub(&s.mul(&c[i]));
                Val::V(out, if a.lip() > 0.0 { f64::INFINITY } else { l })
            }
        }
    }

    pub fn expr(&mut self, e: &'a Expr) -> Res<Val<S>> {
        // A composite abstract integer (for example -1 or 1 + 2) can
        // acquire a concrete type at its return or argument boundary.
        let v = match (self.ty(e).clone(),self.expr_inner(e)?) {
            (Ty::F32,Val::I(x))=>Val::S(S::c(f64::from(x))),
            (Ty::F32,Val::U(x))=>Val::S(S::c(f64::from(x))),
            (Ty::U32,Val::I(x))=>Val::U(x as u32),
            (_,v)=>v,
        };
        let inf = match &v {
            Val::S(s) => s.lip().is_infinite(),
            Val::V(_, l) => l.is_infinite(),
            Val::Void | Val::B(_) | Val::U(_) | Val::I(_) | Val::X(_) | Val::Row(..) | Val::MapRef(..) | Val::Bsdf(_) => false,
        };
        if inf && self.cause.is_none() {
            self.cause = Some((e.span, "this expression has no finite Lipschitz bound over the region".into()));
        }
        Ok(v)
    }

    fn ty(&self, e: &Expr) -> &Ty {
        &self.types[e.id as usize]
    }

    fn expr_arm(&mut self, arm: &'a ExprBlock) -> Res<Val<S>> {
        let mark = self.env.len();
        let result = (|| {
            for stmt in &arm.body.stmts {
                if let Flow::Ret(value) = self.stmt(stmt)? { return Ok(value); }
            }
            self.expr(&arm.value)
        })();
        self.env.truncate(mark);
        result
    }

    fn expr_inner(&mut self, e: &'a Expr) -> Res<Val<S>> {
        match &e.kind {
            ExprKind::If { condition, then, otherwise } => {
                match self.expr(condition)? {
                    Val::B(Some(true)) => self.expr_arm(then),
                    Val::B(Some(false)) => self.expr_arm(otherwise),
                    Val::B(None) if then.body.stmts.is_empty() && otherwise.body.stmts.is_empty() => {
                        let a = self.expr(&then.value)?;
                        let b = self.expr(&otherwise.value)?;
                        Ok(match (a, b) {
                            (Val::S(a), Val::S(b)) => Val::S(a.join(&b)),
                            (Val::V(a, al), Val::V(b, bl)) => Val::V(a.into_iter().zip(b).map(|(a, b)| a.join(&b)).collect(), al.max(bl)),
                            (Val::B(a), Val::B(b)) => Val::B(if a == b { a } else { None }),
                            (_, _) => return Err(err(e.span, "incompatible interval branches", "use a continuous field expression")),
                        })
                    }
                    _ => Err(Diag::new(Code::E211, condition.span, "conditional block cannot be bounded over both paths", "use a continuous field expression")),
                }
            }
            ExprKind::Num { value, suffix, .. } => {
                let v = if *suffix == NumSuffix::Deg { value.to_radians() } else { *value };
                Ok(match self.ty(e) {
                    Ty::U32 => Val::U(v as u32),
                    Ty::I32 | Ty::AbstractInt => Val::I(v as i32),
                    Ty::Void | Ty::Bool | Ty::F32 | Ty::VecN(_) | Ty::Geo(..) | Ty::Xform(..) | Ty::Map(..) | Ty::Row(_) | Ty::Frame(_) | Ty::Table(_) | Ty::Verdict | Ty::Struct(_) | Ty::Array(..) | Ty::Id(_) | Ty::Bsdf => Val::S(S::c(f64::from(v as f32))),
                })
            }
            ExprKind::Bool(b) => Ok(Val::B(Some(*b))),
            ExprKind::Name(n) => {
                if let Some(slot) = self.lookup(n) {
                    return Ok(match slot {
                        Slot::Val(v) | Slot::Map { p: v, .. } => v.clone(),
                    });
                }
                if n=="time"{return Ok(Val::S(self.time.clone()))}
                let c = self.p.consts[n.as_str()];
                if let Some((_, x)) = self.overrides.iter().find(|(k, _)| k == n) {
                    return Ok(match self.types[c.value.id as usize] {
                        Ty::U32 => Val::U(*x as u32),
                        Ty::I32 | Ty::AbstractInt => Val::I(*x as i32),
                        Ty::Void | Ty::Bool | Ty::F32 | Ty::VecN(_) | Ty::Geo(..) | Ty::Xform(..) | Ty::Map(..) | Ty::Row(_) | Ty::Frame(_) | Ty::Table(_) | Ty::Verdict | Ty::Struct(_) | Ty::Array(..) | Ty::Id(_) | Ty::Bsdf => Val::S(S::c(*x)),
                    });
                }
                self.module_expr(&c.value)
            }
            ExprKind::Member(base, m) => {
                if let ExprKind::Name(n) = &base.kind
                    && let Some(Slot::Map { cell, .. }) = self.lookup(n)
                {
                    match m.name.as_str() {
                        "cell" => return Ok(Val::S(cell.clone().expect("checked: a mirror stage"))),
                        "to_parent" => return Ok(Val::MapRef(n.clone(), true)),
                        "from_parent" => return Ok(Val::MapRef(n.clone(), false)),
                        _ => {}
                    }
                }
                let b = self.expr(base)?;
                match b {
                    Val::Row(k, r) => {
                        let decl = self.p.kinds[self.p.kind_order[k]];
                        let i = decl.row.iter().position(|f| f.name.name == m.name).expect("checked field");
                        Ok(Self::lift(&self.tables[k][r][i]))
                    }
                    Val::V(c, l) if c.len() == 12 => {
                        let o = match frame_field(&m.name) {
                            "origin" => 0,
                            "tangent" => 3,
                            "normal" => 6,
                            _ => 9,
                        };
                        Ok(Val::V(c[o..o + 3].to_vec(), l))
                    }
                    Val::V(c, l) => {
                        let idx: Vec<usize> =
                            m.name.chars().map(|ch| "xyzw".find(ch).or_else(|| "rgba".find(ch)).expect("checked")).collect();
                        if idx.len() == 1 {
                            Ok(Val::S(c[idx[0]].clone().with_lip(l)))
                        } else {
                            Ok(Val::V(idx.iter().map(|&i| c[i].clone().with_lip(l)).collect(), l))
                        }
                    }
                    other @ Val::Void | other @ Val::S(_) | other @ Val::B(_) | other @ Val::U(_) | other @ Val::I(_) | other @ Val::X(_) | other @ Val::MapRef(..) | other @ Val::Bsdf(_) => unreachable!("checked member of {other:?}"),
                }
            }
            ExprKind::Index(base, i) => {
                let (b, i) = (self.expr(base)?, self.expr(i)?);
                let i = match i {
                    Val::U(x) => x as usize,
                    Val::I(x) => x as usize,
                    Val::Void | Val::S(_) | Val::V(..) | Val::B(_) | Val::X(_) | Val::Row(..) | Val::MapRef(..) | Val::Bsdf(_) => return Err(err(e.span, "index not a known integer", "index with an integer")),
                };
                match b {
                    Val::V(c, l) => c.get(i).map(|x| Val::S(x.clone().with_lip(l))).ok_or_else(|| err(e.span, "index out of range", "index 0..n")),
                    other @ Val::Void | other @ Val::S(_) | other @ Val::B(_) | other @ Val::U(_) | other @ Val::I(_) | other @ Val::X(_) | other @ Val::Row(..) | other @ Val::MapRef(..) | other @ Val::Bsdf(_) => unreachable!("checked index of {other:?}"),
                }
            }
            ExprKind::Unary(op, x) => {
                let v = self.expr(x)?;
                Ok(match (op, v) {
                    (UnOp::Neg, Val::S(s)) => Val::S(s.neg()),
                    (UnOp::Neg, Val::V(c, l)) => Val::V(c.iter().map(S::neg).collect(), l),
                    (UnOp::Neg, Val::I(i)) => Val::I(-i),
                    (UnOp::Not, Val::B(b)) => Val::B(b.map(|b| !b)),
                    (_, other) => unreachable!("checked unary on {other:?}"),
                })
            }
            ExprKind::Binary(op, a, b) => {
                let (x, y) = (self.expr(a)?, self.expr(b)?);
                self.binary(*op, x, y, a, b, e.span)
            }
            ExprKind::Ctor { ty: _, args } => {
                let vals = args.iter().map(|a| self.expr(&a.value)).collect::<Res<Vec<_>>>()?;
                match self.ty(e).clone() {
                    Ty::Xform(..) => {
                        let (Val::V(o, _), Val::S(yaw)) = (&vals[0], &vals[1]) else { unreachable!("checked") };
                        let ex = |s: &S| s.exact().ok_or_else(|| err(e.span, "an Xform built from values that vary", "build Xforms at stage 0"));
                        Ok(Val::X(Rigid::yaw([ex(&o[0])? as f32, ex(&o[1])? as f32, ex(&o[2])? as f32], ex(yaw)? as f32)))
                    }
                    Ty::Frame(_) => Ok(build(vals, 12)),
                    Ty::Geo(Geo::Dir | Geo::Norm, _) => Ok(normalize(build(vals, 3))),
                    Ty::Geo(..) => Ok(build(vals, 3)),
                    Ty::VecN(n) => Ok(build(vals, n as usize)),
                    other @ Ty::Void | other @ Ty::Bool | other @ Ty::U32 | other @ Ty::I32 | other @ Ty::F32 | other @ Ty::AbstractInt | other @ Ty::Map(..) | other @ Ty::Row(_) | other @ Ty::Table(_) | other @ Ty::Verdict | other @ Ty::Struct(_) | other @ Ty::Array(..) | other @ Ty::Id(_) | other @ Ty::Bsdf => unreachable!("checked ctor of {other}"),
                }
            }
            ExprKind::Call { callee, args } => {
                if callee.name=="loft_conditional" && let Val::B(Some(b))=self.expr(&args[2].value)? {
                    return self.expr(&args[usize::from(b)].value);
                }
                if callee.name=="place" {
                    let Val::V(o,_)=self.expr(&args[0].value)? else {unreachable!("typed placement")};
                    let exact=|s:&S|s.exact().map(|v|v as f32).ok_or_else(||err(e.span,"placement varies across the query","store it in a row"));
                    let yaw=match args.get(1) {Some(a)=>match self.expr(&a.value)?{Val::S(x)=>exact(&x)?,Val::I(n)=>n as f32,_=>unreachable!("typed yaw")},None=>0.};
                    return Ok(Val::X(Rigid::yaw([exact(&o[0])?,exact(&o[1])?,exact(&o[2])?],yaw)));
                }
                if matches!(callee.name.as_str(), "sample_c2" | "loft_dsample") {
                    let ExprKind::Name(n) = &args[0].value.kind else { unreachable!("checked scan name") };
                    let lut = self.luts.iter().find(|l| &l.name == n).ok_or_else(|| err(e.span, format!("scan `{n}` is not baked yet"), "a scan reads only scans declared before it"))?;
                    let Val::S(t) = self.expr(&args[1].value)? else { unreachable!("checked f32") };
                    let mut c = lut.sample(&t, callee.name == "loft_dsample");
                    if callee.name == "loft_dsample" {
                        let Val::S(dt) = self.expr(&args[2].value)? else { unreachable!("checked f32") };
                        c = c.iter().map(|x| x.mul(&dt)).collect();
                    }
                    return Ok(Val::V(c, 0.0));
                }
                let vals = args.iter().map(|a| self.expr(&a.value)).collect::<Res<Vec<_>>>()?;
                if let Some(f) = self.p.fns.get(callee.name.as_str()) {
                    if f.attrs.iter().any(|a| a.name.name == "lobe") {
                        return Ok(Val::Bsdf(vec![(callee.name.clone(), vals)]));
                    }
                    return self.call_fn(f, vals, e.span);
                }
                if callee.name == "eval_bsdf" {
                    return self.eval_bsdf(&vals[0], &vals[1], &vals[2], &vals[3], e.span);
                }
                Ok(self.builtin(&callee.name, vals, e))
            }
        }
    }

    fn binary(&mut self, op: BinOp, x: Val<S>, y: Val<S>, a: &Expr, b: &Expr, span: Span) -> Res<Val<S>> {
        use BinOp::*;
        let sv = |i: &Val<S>| match i {
            Val::I(i) => Val::S(S::c(f64::from(*i))),
            Val::U(u) => Val::S(S::c(f64::from(*u))),
            other @ Val::Void | other @ Val::S(_) | other @ Val::V(..) | other @ Val::B(_) | other @ Val::X(_) | other @ Val::Row(..) | other @ Val::MapRef(..) | other @ Val::Bsdf(_) => other.clone(),
        };
        Ok(match (op, &x, &y) {
            (_, Val::MapRef(name, inverse), Val::V(..)) => {
                let point = matches!(self.ty(b), Ty::Geo(Geo::Pt, _));
                let Some(Slot::Map { cell, stages, .. }) = self.lookup(name).cloned() else { unreachable!("checked map") };
                let mut v = y.clone();
                let order: Vec<&Stage<S>> = if *inverse { stages.iter().rev().collect() } else { stages.iter().collect() };
                for st in order {
                    v = self.stage(st, v, cell.as_ref(), !inverse, point, false, span);
                }
                v
            }
            (Add, Val::Bsdf(p), Val::Bsdf(q)) => Val::Bsdf(p.iter().chain(q).cloned().collect()),
            (Mul, Val::X(p), Val::X(q)) => Val::X(p.then(q)),
            (Mul, Val::X(r), Val::V(c, l)) => {
                let point = matches!(self.ty(b), Ty::Geo(Geo::Pt, _));
                let out = (0..3)
                    .map(|i| {
                        let mut s = c[0].mul(&S::c(f64::from(r.r[i][0])));
                        s = s.add(&c[1].mul(&S::c(f64::from(r.r[i][1]))));
                        s = s.add(&c[2].mul(&S::c(f64::from(r.r[i][2]))));
                        if point { s.add(&S::c(f64::from(r.t[i]))) } else { s }
                    })
                    .collect();
                // Rigid: an isometry keeps the bound.
                Val::V(out, *l)
            }
            (_, Val::U(p), Val::U(q)) => match op {
                Add => Val::U(p.wrapping_add(*q)),
                Sub => Val::U(p.wrapping_sub(*q)),
                Mul => Val::U(p.wrapping_mul(*q)),
                Div => Val::U(p.checked_div(*q).ok_or_else(|| err(span, "u32 division by zero", "guard the divisor"))?),
                Rem => Val::U(p.checked_rem(*q).ok_or_else(|| err(span, "u32 remainder by zero", "guard the divisor"))?),
                BitAnd => Val::U(p & q),
                BitOr => Val::U(p | q),
                BitXor => Val::U(p ^ q),
                Shl => Val::U(p.wrapping_shl(*q)),
                Shr => Val::U(p.wrapping_shr(*q)),
                Lt => Val::B(Some(p < q)),
                Le => Val::B(Some(p <= q)),
                Gt => Val::B(Some(p > q)),
                Ge => Val::B(Some(p >= q)),
                Eq => Val::B(Some(p == q)),
                Ne => Val::B(Some(p != q)),
                And | Or => unreachable!("checked"),
            },
            (_, Val::U(_), Val::I(q)) => return self.binary(op, x.clone(), Val::U(*q as u32), a, b, span),
            (_, Val::I(p), Val::U(_)) => return self.binary(op, Val::U(*p as u32), y.clone(), a, b, span),
            (_, Val::I(p), Val::I(q)) if !matches!(self.ty(a), Ty::F32) => match op {
                Add => Val::I(p.wrapping_add(*q)),
                Sub => Val::I(p.wrapping_sub(*q)),
                Mul => Val::I(p.wrapping_mul(*q)),
                Div => Val::I(p.checked_div(*q).ok_or_else(|| err(span, "i32 division by zero", "guard the divisor"))?),
                Rem => Val::I(p.checked_rem(*q).ok_or_else(|| err(span, "i32 remainder by zero", "guard the divisor"))?),
                Lt => Val::B(Some(p < q)),
                Le => Val::B(Some(p <= q)),
                Gt => Val::B(Some(p > q)),
                Ge => Val::B(Some(p >= q)),
                Eq => Val::B(Some(p == q)),
                Ne => Val::B(Some(p != q)),
                BitAnd => Val::I(p & q),
                BitOr => Val::I(p | q),
                BitXor => Val::I(p ^ q),
                Shl => Val::I(p.wrapping_shl(*q as u32)),
                Shr => Val::I(p.wrapping_shr(*q as u32)),
                And | Or => unreachable!("checked"),
            },
            (And, Val::B(p), Val::B(q)) => Val::B(match (p, q) {
                (Some(false), _) | (_, Some(false)) => Some(false),
                (Some(true), Some(true)) => Some(true),
                _ => None,
            }),
            (Or, Val::B(p), Val::B(q)) => Val::B(match (p, q) {
                (Some(true), _) | (_, Some(true)) => Some(true),
                (Some(false), Some(false)) => Some(false),
                _ => None,
            }),
            (Eq | Ne, Val::B(p), Val::B(q)) => Val::B(p.zip(*q).map(|(p, q)| (p == q) == (op == Eq))),
            _ => match (sv(&x), sv(&y)) {
                (Val::S(p), Val::S(q)) => match op {
                    Add => Val::S(p.add(&q)),
                    Sub => Val::S(p.sub(&q)),
                    Mul => Val::S(p.mul(&q)),
                    Div => Val::S(p.div(&q)),
                    Lt | Le | Gt | Ge | Eq | Ne => Val::B(p.cmp(op, &q)),
                    Rem | And | Or | BitAnd | BitOr | BitXor | Shl | Shr => unreachable!("checked float op {op:?}"),
                },
                (Val::V(p, lp), Val::V(q, lq)) => {
                    let c: Vec<S> = p
                        .iter()
                        .zip(&q)
                        .map(|(u, v)| match op {
                            Add => u.add(v),
                            Sub => u.sub(v),
                            Mul => u.mul(v),
                            Div => u.div(v),
                            Rem | Lt | Le | Gt | Ge | Eq | Ne | And | Or | BitAnd | BitOr | BitXor | Shl | Shr => unreachable!("checked vector op"),
                        })
                        .collect();
                    let l = match op {
                        Add | Sub => lp + lq,
                        Mul => max_mag(&p) * lq + max_mag(&q) * lp,
                        Div | Rem | Lt | Le | Gt | Ge | Eq | Ne | And | Or | BitAnd | BitOr | BitXor | Shl | Shr => {
                            let mig = q.iter().map(S::mig).fold(f64::INFINITY, f64::min);
                            if mig > 0.0 { (lp * max_mag(&q) + max_mag(&p) * lq) / (mig * mig) } else { f64::INFINITY }
                        }
                    };
                    Val::V(c, if lp == 0.0 && lq == 0.0 { 0.0 } else { l })
                }
                (Val::V(p, lp), Val::S(s)) => {
                    let c: Vec<S> = p.iter().map(|u| if op == Mul { u.mul(&s) } else { u.div(&s) }).collect();
                    let l = if op == Mul {
                        norm(&p) * s.lip() + s.mag() * lp
                    } else if s.mig() > 0.0 {
                        (lp * s.mag() + norm(&p) * s.lip()) / (s.mig() * s.mig())
                    } else {
                        f64::INFINITY
                    };
                    Val::V(c, if lp == 0.0 && s.lip() == 0.0 { 0.0 } else { l })
                }
                (Val::S(s), Val::V(p, lp)) => {
                    let c: Vec<S> = p.iter().map(|u| s.mul(u)).collect();
                    let l = norm(&p) * s.lip() + s.mag() * lp;
                    Val::V(c, if lp == 0.0 && s.lip() == 0.0 { 0.0 } else { l })
                }
                (p, q) => unreachable!("checked binary {op:?} on {p:?} and {q:?}"),
            },
        })
    }

    fn builtin(&mut self, name: &str, mut v: Vec<Val<S>>, e: &'a Expr) -> Val<S> {
        let f32ty = matches!(self.ty(e), Ty::F32 | Ty::VecN(_) | Ty::Geo(..));
        // Integer literals meet f32 in componentwise maths.
        if f32ty {
            for x in &mut v {
                if let Val::I(i) = x {
                    *x = Val::S(S::c(f64::from(*i)));
                }
            }
        }
        let un = |x: &Val<S>, f: Un| match x {
            Val::S(s) => Val::S(s.un(f)),
            Val::V(c, l) => {
                let c: Vec<S> = c.iter().map(|s| s.un(f)).collect();
                // abs is 1-Lipschitz componentwise, so it keeps the vector bound; the rest bound per component.
                let l = match f {
                    Un::Abs => *l,
                    Un::Sin | Un::Cos | Un::Sqrt | Un::Exp | Un::Floor | Un::Fract | Un::Sign => c.iter().map(|s| s.lip() * s.lip()).sum::<f64>().sqrt(),
                };
                Val::V(c, l)
            }
            other @ Val::Void | other @ Val::B(_) | other @ Val::U(_) | other @ Val::I(_) | other @ Val::X(_) | other @ Val::Row(..) | other @ Val::MapRef(..) | other @ Val::Bsdf(_) => unreachable!("checked unary builtin on {other:?}"),
        };
        let zip2 = |x: &Val<S>, y: &Val<S>, f: &dyn Fn(&S, &S) -> S| -> Val<S> {
            match (x, y) {
                (Val::S(a), Val::S(b)) => Val::S(f(a, b)),
                (Val::V(a, la), Val::V(b, lb)) => Val::V(a.iter().zip(b).map(|(p, q)| f(p, q)).collect(), la.hypot(*lb)),
                (Val::V(a, la), Val::S(b)) => Val::V(a.iter().map(|p| f(p, b)).collect(), la.hypot(b.lip())),
                (Val::S(a), Val::V(b, lb)) => Val::V(b.iter().map(|q| f(a, q)).collect(), lb.hypot(a.lip())),
                other => unreachable!("checked binary builtin on {other:?}"),
            }
        };
        match name {
            "frame_axes" => build(v,12),
            "has_sky"=>Val::B(Some(self.p.kind_order.iter().position(|k|*k=="Sky").and_then(|i|self.tables.get(i)).is_some_and(|t|!t.is_empty()))),
            "sky_zenith" | "sky_horizon" | "sun_color" => {
                let (kind, column, default) = match name {
                    "sky_zenith" => ("Sky", 0, crate::defaults::ENV_DEFAULTS[1]),
                    "sky_horizon" => ("Sky", 1, crate::defaults::ENV_DEFAULTS[2]),
                    _ => ("Sun", 1, crate::defaults::ENV_DEFAULTS[0]),
                };
                if let Some(row) = self.p.kind_order.iter().position(|k| *k == kind)
                    .and_then(|i| self.tables.get(i)).and_then(|rows| rows.first())
                {
                    Self::lift(&row[column])
                } else {
                    self.module_expr(&self.p.consts[default].value).expect("validated prelude default")
                }
            }
            "coords" => v.remove(0),
            "dir" => normalize(v.remove(0)),
            "loft_zero" => match &v[0] {
                Val::S(_) => Val::S(S::c(0.0)),
                Val::V(c, _) => Val::V(vec![S::c(0.0); c.len()], 0.0),
                Val::U(_) => Val::U(0),
                Val::I(_) => Val::I(0),
                other @ Val::Void | other @ Val::B(_) | other @ Val::X(_) | other @ Val::Row(..) | other @ Val::MapRef(..) | other @ Val::Bsdf(_) => unreachable!("checked zero of {other:?}"),
            },
            "loft_vec" => v[0].clone(),
            "smootherstep" | "loft_dsmoothstep" | "loft_dsmootherstep" => {
                // t = clamp((x - a) / (b - a)); value t^3 (6t^2 - 15t + 10); slopes 6t(1-t)/(b-a), 30t^2(1-t)^2/(b-a).
                let (e0, e1, x) = (v[0].clone(), v[1].clone(), v[2].clone());
                let span = self.arith(BinOp::Sub, e1, e0.clone());
                let num = self.arith(BinOp::Sub, x, e0);
                let t = self.arith(BinOp::Div, num, span.clone());
                let t = zip2(&t, &Val::S(S::c(0.0)), &|a, z| a.max(z).min(&S::c(1.0)));
                let c = |k: f64| Val::S(S::c(k));
                let one_t = self.arith(BinOp::Sub, c(1.0), t.clone());
                let tt = self.arith(BinOp::Mul, t.clone(), t.clone());
                match name {
                    "smootherstep" => {
                        let a = self.arith(BinOp::Mul, c(6.0), t.clone());
                        let a = self.arith(BinOp::Sub, a, c(15.0));
                        let a = self.arith(BinOp::Mul, a, t.clone());
                        let a = self.arith(BinOp::Add, a, c(10.0));
                        let t3 = self.arith(BinOp::Mul, tt, t);
                        let out = self.arith(BinOp::Mul, t3, a);
                        // Constant edges: h is monotone, so its range is [h(lo), h(hi)], and L(h(x)) = sup|h'| L(x)
                        // with sup|h'| = 15 / (8 |b - a|) (draft §2D). Interval arithmetic on the polynomial would
                        // claim a range of [-5, 10] and ~40x the slope.
                        match (&v[0], &v[1], &v[2]) {
                            (Val::S(a), Val::S(b), Val::S(x)) if a.exact().is_some() && b.exact().is_some() && a.exact() != b.exact() => {
                                let (a, b) = (a.exact().expect("exact"), b.exact().expect("exact"));
                                let h = |x: f64| {
                                    let t = ((x - a) / (b - a)).clamp(0.0, 1.0);
                                    t * t * t * ((6.0 * t - 15.0) * t + 10.0)
                                };
                                match x.exact() {
                                    Some(_) => out,
                                    None => {
                                        let (lo, hi) = x.range();
                                        let (p, q) = (h(lo), h(hi));
                                        Val::S(S::c(p.min(q)).join(&S::c(p.max(q))).with_lip(1.875 / (b - a).abs() * x.lip()))
                                    }
                                }
                            }
                            _ => out,
                        }
                    }
                    "loft_dsmoothstep" => {
                        let a = self.arith(BinOp::Mul, c(6.0), t);
                        let a = self.arith(BinOp::Mul, a, one_t);
                        self.arith(BinOp::Div, a, span)
                    }
                    _ => {
                        let oo = self.arith(BinOp::Mul, one_t.clone(), one_t);
                        let a = self.arith(BinOp::Mul, c(30.0), tt);
                        let a = self.arith(BinOp::Mul, a, oo);
                        self.arith(BinOp::Div, a, span)
                    }
                }
            }
            "abs" => un(&v[0], Un::Abs),
            "sqrt" => un(&v[0], Un::Sqrt),
            "sin" => un(&v[0], Un::Sin),
            "cos" => un(&v[0], Un::Cos),
            "exp" => un(&v[0], Un::Exp),
            "floor" => un(&v[0], Un::Floor),
            "fract" => un(&v[0], Un::Fract),
            "sign" => un(&v[0], Un::Sign),
            "min" => zip2(&v[0], &v[1], &|a, b| a.min(b)),
            "max" => zip2(&v[0], &v[1], &|a, b| a.max(b)),
            "pow" => zip2(&v[0], &v[1], &|a, b| a.pow(b)),
            "step" => zip2(&v[0], &v[1], &|e, x| {
                let d = x.sub(e);
                let s = d.un(Un::Sign).max(&S::c(0.0));
                // step(e, x) = 1 where x >= e: sign maps 0 to 0, so lift the boundary.
                s.max(&S::c(if d.exact() == Some(0.0) { 1.0 } else { 0.0 }))
            }),
            "saturate" => zip2(&v[0], &Val::S(S::c(0.0)), &|a, z| a.max(z).min(&S::c(1.0))),
            "clamp" => {
                let lo = zip2(&v[0], &v[1], &|a, b| a.max(b));
                zip2(&lo, &v[2], &|a, b| a.min(b))
            }
            "mix" => {
                // a + (b - a) * t, through the same rules.
                let (a, b, t) = (v[0].clone(), v[1].clone(), v[2].clone());
                let d = self.arith(BinOp::Sub, b, a.clone());
                let dt = self.arith(BinOp::Mul, d, t);
                self.arith(BinOp::Add, a, dt)
            }
            "smoothstep" => {
                let (e0, e1, x) = (v[0].clone(), v[1].clone(), v[2].clone());
                let span = self.arith(BinOp::Sub, e1, e0.clone());
                let num = self.arith(BinOp::Sub, x, e0);
                let t = self.arith(BinOp::Div, num, span);
                let t = zip2(&t, &Val::S(S::c(0.0)), &|a, z| a.max(z).min(&S::c(1.0)));
                let tt = self.arith(BinOp::Mul, t.clone(), t.clone());
                let two_t = self.arith(BinOp::Mul, Val::S(S::c(2.0)), t);
                let k = self.arith(BinOp::Sub, Val::S(S::c(3.0)), two_t);
                self.arith(BinOp::Mul, tt, k)
            }
            "select" | "loft_conditional" => match &v[2] {
                Val::B(Some(true)) => v[1].clone(),
                Val::B(Some(false)) => v[0].clone(),
                Val::Void | Val::S(_) | Val::V(..) | Val::B(_) | Val::U(_) | Val::I(_) | Val::X(_) | Val::Row(..) | Val::MapRef(..) | Val::Bsdf(_) => match (&v[0], &v[1]) {
                    (Val::S(a), Val::S(b)) => Val::S(a.join(b).with_lip(f64::INFINITY)),
                    (Val::V(a, _), Val::V(b, _)) => Val::V(a.iter().zip(b).map(|(p, q)| p.join(q)).collect(), f64::INFINITY),
                    other => unreachable!("checked select on {other:?}"),
                },
            },
            "length" => match &v[0] {
                Val::S(s) => Val::S(s.un(Un::Abs)),
                Val::V(c, l) => Val::S(sum_sq(c).un(Un::Sqrt).with_lip(*l)),
                other @ Val::Void | other @ Val::B(_) | other @ Val::U(_) | other @ Val::I(_) | other @ Val::X(_) | other @ Val::Row(..) | other @ Val::MapRef(..) | other @ Val::Bsdf(_) => unreachable!("checked length of {other:?}"),
            },
            "distance" | "dist" => {
                let d = self.arith(BinOp::Sub, v[0].clone(), v[1].clone());
                match d {
                    Val::V(c, l) => Val::S(sum_sq(&c).un(Un::Sqrt).with_lip(l)),
                    Val::S(s) => Val::S(s.un(Un::Abs)),
                    other @ Val::Void | other @ Val::B(_) | other @ Val::U(_) | other @ Val::I(_) | other @ Val::X(_) | other @ Val::Row(..) | other @ Val::MapRef(..) | other @ Val::Bsdf(_) => unreachable!("checked distance of {other:?}"),
                }
            }
            "dot" => {
                let (Val::V(a, la), Val::V(b, lb)) = (&v[0], &v[1]) else { unreachable!("checked dot") };
                let mut s = a[0].mul(&b[0]);
                for i in 1..a.len() {
                    s = s.add(&a[i].mul(&b[i]));
                }
                Val::S(s.with_lip(norm(a) * lb + norm(b) * la))
            }
            "cross" => {
                let (Val::V(a, la), Val::V(b, lb)) = (&v[0], &v[1]) else { unreachable!("checked cross") };
                let c = vec![
                    a[1].mul(&b[2]).sub(&a[2].mul(&b[1])),
                    a[2].mul(&b[0]).sub(&a[0].mul(&b[2])),
                    a[0].mul(&b[1]).sub(&a[1].mul(&b[0])),
                ];
                Val::V(c, norm(a) * lb + norm(b) * la)
            }
            "normalize" => normalize(v[0].clone()),
            "inv" => match &v[0] {
                Val::X(x) => Val::X(x.inv()),
                other @ Val::Void | other @ Val::S(_) | other @ Val::V(..) | other @ Val::B(_) | other @ Val::U(_) | other @ Val::I(_) | other @ Val::Row(..) | other @ Val::MapRef(..) | other @ Val::Bsdf(_) => unreachable!("checked inv of {other:?}"),
            },
            "f32" => match &v[0] {
                Val::U(u) => Val::S(S::c(f64::from(*u))),
                Val::I(i) => Val::S(S::c(f64::from(*i))),
                other @ Val::Void | other @ Val::S(_) | other @ Val::V(..) | other @ Val::B(_) | other @ Val::X(_) | other @ Val::Row(..) | other @ Val::MapRef(..) | other @ Val::Bsdf(_) => other.clone(),
            },
            // Between integers WGSL reinterprets the bits (u32(-1) is 0xffffffff); from f32 it saturates.
            "u32" if matches!(v[0], Val::I(_)) => match v[0] {
                Val::I(i) => Val::U(i as u32),
                _ => unreachable!("matched"),
            },
            "i32" if matches!(v[0], Val::U(_)) => match v[0] {
                Val::U(u) => Val::I(u as i32),
                _ => unreachable!("matched"),
            },
            "u32" | "i32" => {
                let x = match &v[0] {
                    Val::U(u) => f64::from(*u),
                    Val::I(i) => f64::from(*i),
                    Val::S(s) => s.exact().unwrap_or(f64::NAN),
                    other @ Val::Void | other @ Val::V(..) | other @ Val::B(_) | other @ Val::X(_) | other @ Val::Row(..) | other @ Val::MapRef(..) | other @ Val::Bsdf(_) => unreachable!("checked conversion of {other:?}"),
                };
                if name == "u32" { Val::U(x as u32) } else { Val::I(x as i32) }
            }
            "vec2" | "vec3" | "vec4" | "vec2f" | "vec3f" | "vec4f" => build(v, (name.as_bytes()[3] - b'0') as usize),
            other => unreachable!("checked builtin {other}"),
        }
    }

    /// `eval_bsdf(b, n, v, l)`: each lobe bottom first, `c = c * below + own` (the lobe's vec4 is own rgb and the
    /// transmittance it leaves the layers under it).
    pub fn eval_bsdf(&mut self, b: &Val<S>, n: &Val<S>, v: &Val<S>, l: &Val<S>, span: Span) -> Res<Val<S>> {
        let Val::Bsdf(lobes) = b else { unreachable!("checked Bsdf") };
        let mut c = Val::V(vec![S::c(0.0); 3], 0.0);
        for (name, args) in lobes {
            let f = self.p.fns[name.as_str()];
            let mut a = args.clone();
            a.extend([n.clone(), v.clone(), l.clone()]);
            let Val::V(r, _) = self.call_fn(f, a, span)? else { unreachable!("checked vec4") };
            let below = Val::S(r[3].clone());
            let own = Val::V(r[..3].to_vec(), 0.0);
            let kept = self.arith(BinOp::Mul, c, below);
            c = self.arith(BinOp::Add, kept, own);
        }
        Ok(c)
    }

    /// Arithmetic on values without source expressions (used by builtins composed of ops).
    fn arith(&mut self, op: BinOp, x: Val<S>, y: Val<S>) -> Val<S> {
        let dummy = Expr { id: u32::MAX, span: Span::default(), kind: ExprKind::Bool(false) };
        // Only float/vector operands reach here, so the integer arms that read `ty` are never taken.
        self.binary(op, x, y, &dummy, &dummy, Span::default()).expect("float arithmetic cannot fail")
    }
}

fn max_mag<S: Scalar>(v: &[S]) -> f64 {
    v.iter().map(S::mag).fold(0.0, f64::max)
}

fn sum_sq<S: Scalar>(c: &[S]) -> S {
    let mut s = c[0].mul(&c[0]);
    for x in &c[1..] {
        s = s.add(&x.mul(x));
    }
    s
}

fn sign_cell<S: Scalar>(x: &S) -> S {
    // cell = +1 where x >= 0, else -1 (matches the emitted `select(-1.0, 1.0, x >= 0.0)`).
    match x.cmp(BinOp::Ge, &S::c(0.0)) {
        Some(true) => S::c(1.0),
        Some(false) => S::c(-1.0),
        None => S::c(-1.0).join(&S::c(1.0)).with_lip(f64::INFINITY),
    }
}

fn build<S: Scalar>(vals: Vec<Val<S>>, n: usize) -> Val<S> {
    let mut c = vec![];
    let mut l2 = 0.0;
    for v in vals {
        match v {
            Val::S(s) => {
                l2 += s.lip() * s.lip();
                c.push(s);
            }
            Val::V(x, l) => {
                l2 += l * l;
                c.extend(x);
            }
            Val::I(i) => c.push(S::c(f64::from(i))),
            Val::U(u) => c.push(S::c(f64::from(u))),
            other @ Val::Void | other @ Val::B(_) | other @ Val::X(_) | other @ Val::Row(..) | other @ Val::MapRef(..) | other @ Val::Bsdf(_) => unreachable!("checked component {other:?}"),
        }
    }
    if c.len() == 1 {
        c = vec![c[0].clone(); n];
        l2 *= n as f64;
    }
    Val::V(c, l2.sqrt())
}

fn normalize<S: Scalar>(v: Val<S>) -> Val<S> {
    let Val::V(c, l) = v else { unreachable!("checked normalize") };
    let len = sum_sq(&c).un(Un::Sqrt);
    let out = c.iter().map(|x| x.div(&len)).collect();
    Val::V(out, if l == 0.0 { 0.0 } else { f64::INFINITY })
}

/// A value fixed at stage 0, for a row table.
pub fn concrete<S: Scalar>(v: &Val<S>, span: Span) -> Res<RowVal> {
    let ex = |s: &S| {
        s.exact().map(|x| x as f32).ok_or_else(|| err(span, "a row value that is not a single number", "compute rows from stage-0 values"))
    };
    Ok(match v {
        Val::S(s) => RowVal::F(ex(s)?),
        Val::V(c, _) => RowVal::V(c.iter().map(ex).collect::<Res<_>>()?),
        Val::U(u) => RowVal::U(*u),
        Val::I(i) => RowVal::I(*i),
        Val::B(Some(b)) => RowVal::B(*b),
        Val::X(x) => RowVal::X(*x),
        other @ Val::Void | other @ Val::B(_) | other @ Val::Row(..) | other @ Val::MapRef(..) | other @ Val::Bsdf(_) => return Err(err(span, format!("cannot store {other:?} in a row"), "store numbers, vectors, points or Xforms")),
    })
}

impl Scalar for f32 {
    fn frame_time()->Self{0.0}
    fn c(x: f64) -> f32 {
        x as f32
    }
    fn add(&self, o: &f32) -> f32 {
        self + o
    }
    fn sub(&self, o: &f32) -> f32 {
        self - o
    }
    fn mul(&self, o: &f32) -> f32 {
        self * o
    }
    fn div(&self, o: &f32) -> f32 {
        self / o
    }
    fn neg(&self) -> f32 {
        -self
    }
    fn un(&self, f: Un) -> f32 {
        match f {
            Un::Sin => self.sin(),
            Un::Cos => self.cos(),
            Un::Sqrt => self.sqrt(),
            Un::Abs => self.abs(),
            Un::Exp => self.exp(),
            Un::Floor => self.floor(),
            Un::Fract => self - self.floor(),
            // WGSL sign(0) = 0.
            Un::Sign => {
                if *self > 0.0 {
                    1.0
                } else if *self < 0.0 {
                    -1.0
                } else {
                    0.0
                }
            }
        }
    }
    fn min(&self, o: &f32) -> f32 {
        f32::min(*self, *o)
    }
    fn max(&self, o: &f32) -> f32 {
        f32::max(*self, *o)
    }
    fn pow(&self, o: &f32) -> f32 {
        self.powf(*o)
    }
    fn cmp(&self, op: BinOp, o: &f32) -> Option<bool> {
        Some(match op {
            BinOp::Lt => self < o,
            BinOp::Le => self <= o,
            BinOp::Gt => self > o,
            BinOp::Ge => self >= o,
            BinOp::Eq => self == o,
            BinOp::Ne => self != o,
            BinOp::Add | BinOp::Sub | BinOp::Mul | BinOp::Div | BinOp::Rem | BinOp::And | BinOp::Or | BinOp::BitAnd | BinOp::BitOr | BinOp::BitXor | BinOp::Shl | BinOp::Shr => unreachable!("comparison"),
        })
    }
    fn mag(&self) -> f64 {
        f64::from(self.abs())
    }
    fn mig(&self) -> f64 {
        f64::from(self.abs())
    }
    fn lip(&self) -> f64 {
        0.0
    }
    fn with_lip(self, _: f64) -> f32 {
        self
    }
    fn exact(&self) -> Option<f64> {
        Some(f64::from(*self))
    }
    fn range(&self) -> (f64, f64) {
        (f64::from(*self), f64::from(*self))
    }
    fn join(&self, _: &f32) -> f32 {
        *self
    }
}
