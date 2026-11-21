//! Forward-mode AD (draft §2C) as a source-to-source pass, run before indexing so the checker types the result and
//! every backend (stage 0, the CPU twin, interval bounds, WGSL) runs the same derivative code.
//!
//! `diff(f, x)(args)` was parsed as a call to `f__d_x`; this pass writes that fn using a JVP specialized to the active parameters, seeded with 1 in `x`. A kind's `query surface` gets `surface__jvp` inside the kind, which the
//! compiler calls for the surface normal. Rules are the textbook ones; a `C0` builtin (abs, min, clamp, floor, ...) is
//! E307: differentiating it needs a choice the author has not made.

use loft_builtins::{Builtin, lookup, spec};
use loft_parse::ast::*;
use loft_ir::{Code, Diag, Res, Span};
use std::collections::{BTreeMap, HashSet};

/// Tangent type: a point's tangent is a displacement; a unit direction's tangent is not unit.
fn tty(t: &TyExpr) -> TyExpr {
    match t {
        TyExpr::Geo(Geo::Pt | Geo::Dir | Geo::Norm, s) => TyExpr::Geo(Geo::Vec, s.clone()),
        other @ (TyExpr::Name(_) | TyExpr::Generic(..) | TyExpr::Array(..) | TyExpr::Geo(..) | TyExpr::Xform(..) | TyExpr::Field(_) | TyExpr::Frame(_) | TyExpr::Table(_)) => other.clone(),
    }
}

struct Ad<'a> {
    orig: BTreeMap<&'a str, &'a FnDecl>,
    next: &'a mut u32,
    /// jvp fns still to write.
    todo: Vec<(String, Vec<bool>)>,
    /// Names with a tangent in the fn being transformed.
    live: HashSet<String>,
}

impl Ad<'_> {
    fn e(&mut self, span: Span, kind: ExprKind) -> Expr {
        *self.next += 1;
        Expr { id: *self.next - 1, span, kind }
    }
    fn num(&mut self, v: f64, span: Span) -> Expr {
        self.e(span, ExprKind::Num { value: v, suffix: NumSuffix::None, text: format!("{v:?}"), float: true })
    }
    fn name(&mut self, n: &str, span: Span) -> Expr {
        self.e(span, ExprKind::Name(n.into()))
    }
    fn call(&mut self, n: &str, args: Vec<Expr>, span: Span) -> Expr {
        let args = args.into_iter().map(|value| Arg { name: None, value }).collect();
        self.e(span, ExprKind::Call { callee: Ident { name: n.into(), span }, args })
    }
    fn bin(&mut self, op: BinOp, a: Expr, b: Expr) -> Expr {
        let span = a.span.to(b.span);
        self.e(span, ExprKind::Binary(op, Box::new(a), Box::new(b)))
    }
    fn neg(&mut self, a: Expr) -> Expr {
        let span = a.span;
        self.e(span, ExprKind::Unary(UnOp::Neg, Box::new(a)))
    }
    /// A deep copy with fresh ids (the checker types every node by id).
    fn cp(&mut self, x: &Expr) -> Expr {
        let kind = match &x.kind {
            ExprKind::Unproved { .. } | ExprKind::FiniteDiff { .. } | ExprKind::If { .. } => x.kind.clone(),
            ExprKind::Num { value, suffix, text, float } => ExprKind::Num { value: *value, suffix: *suffix, text: text.clone(), float: *float },
            ExprKind::Bool(b) => ExprKind::Bool(*b),
            ExprKind::String(s) => ExprKind::String(s.clone()),
            ExprKind::Name(n) => ExprKind::Name(n.clone()),
            ExprKind::Member(b, m) => ExprKind::Member(Box::new(self.cp(b)), m.clone()),
            ExprKind::Index(a, i) => ExprKind::Index(Box::new(self.cp(a)), Box::new(self.cp(i))),
            ExprKind::Call { callee, args } => ExprKind::Call { callee: callee.clone(), args: args.iter().map(|a| Arg { name: a.name.clone(), value: self.cp(&a.value) }).collect() },
            ExprKind::Ctor { ty, args } => ExprKind::Ctor { ty: ty.clone(), args: args.iter().map(|a| Arg { name: a.name.clone(), value: self.cp(&a.value) }).collect() },
            ExprKind::Binary(op, a, b) => ExprKind::Binary(*op, Box::new(self.cp(a)), Box::new(self.cp(b))),
            ExprKind::Unary(op, a) => ExprKind::Unary(*op, Box::new(self.cp(a))),
        };
        self.e(x.span, kind)
    }
    /// The tangent, or a zero of the primal's tangent type.
    fn dz(&mut self, x: &Expr) -> Res<Expr> {
        match self.d(x)? {
            Some(d) => Ok(d),
            None if matches!(x.kind, ExprKind::Num { .. }) => Ok(self.num(0.0,x.span)),
            None => {
                let c = self.cp(x);
                Ok(self.call("loft_zero", vec![c], x.span))
            }
        }
    }
    fn add(&mut self, a: Option<Expr>, b: Option<Expr>, sub: bool) -> Option<Expr> {
        match (a, b) {
            (None, None) => None,
            (Some(a), None) => Some(a),
            (None, Some(b)) => Some(if sub { self.neg(b) } else { b }),
            (Some(a), Some(b)) => Some(self.bin(if sub { BinOp::Sub } else { BinOp::Add }, a, b)),
        }
    }

    /// The tangent of `x`; `None` is zero.
    fn d(&mut self, x: &Expr) -> Res<Option<Expr>> {
        let sp = x.span;
        let refuse = |what: &str| Diag::new(Code::E405, sp, format!("`diff` through {what} is not lowered yet"), "differentiate a fn written with arithmetic, calls and the smooth builtins");
        Ok(match &x.kind {
            ExprKind::Unproved { .. } | ExprKind::FiniteDiff { .. } => return Err(Diag::new(
                Code::E307, sp, "automatic differentiation crosses a numerical derivative boundary",
                "use finite differences inside unproved geometry")),
            ExprKind::If { .. } => return Err(Diag::new(Code::E307, x.span, "conditional expression has no smooth derivative", "use a smooth blend")),
            ExprKind::Num { .. } | ExprKind::Bool(_) | ExprKind::String(_) => None,
            ExprKind::Name(n) => self.live.contains(n).then(|| self.name(&format!("{n}__t"), sp)),
            ExprKind::Member(b, m) => {
                if matches!(&b.kind, ExprKind::Name(n) if n == "self") {
                    return Ok(None);
                }
                match self.d(b)? {
                    None => None,
                    Some(db) => {
                        let mem = self.e(sp, ExprKind::Member(Box::new(db), m.clone()));
                        // A tangent frame's `origin` holds a displacement; say so to the checker.
                        Some(if frame_field(&m.name) == "origin" { self.call("loft_vec", vec![mem], sp) } else { mem })
                    }
                }
            }
            ExprKind::Index(a, i) => match self.d(a)? {
                None => None,
                Some(da) => {
                    let i = self.cp(i);
                    Some(self.e(sp, ExprKind::Index(Box::new(da), Box::new(i))))
                }
            },
            ExprKind::Unary(UnOp::Neg, a) => self.d(a)?.map(|d| self.neg(d)),
            ExprKind::Unary(UnOp::Not, _) => None,
            ExprKind::Binary(op, a, b) => match op {
                BinOp::Add | BinOp::Sub => {
                    let (da, db) = (self.d(a)?, self.d(b)?);
                    self.add(da, db, *op == BinOp::Sub)
                }
                BinOp::Mul => {
                    let (da, db) = (self.d(a)?, self.d(b)?);
                    let l = match da {
                        Some(da) => {
                            let bc = self.cp(b);
                            Some(self.bin(BinOp::Mul, da, bc))
                        }
                        None => None,
                    };
                    let r = match db {
                        Some(db) => {
                            let ac = self.cp(a);
                            Some(self.bin(BinOp::Mul, ac, db))
                        }
                        None => None,
                    };
                    self.add(l, r, false)
                }
                BinOp::Div => {
                    // (a / b)' = a' / b - a b' / b^2
                    let (da, db) = (self.d(a)?, self.d(b)?);
                    let l = match da {
                        Some(da) => {
                            let bc = self.cp(b);
                            Some(self.bin(BinOp::Div, da, bc))
                        }
                        None => None,
                    };
                    let r = match db {
                        Some(db) => {
                            let (ac, b1, b2) = (self.cp(a), self.cp(b), self.cp(b));
                            let bb = self.bin(BinOp::Mul, b1, b2);
                            let q = self.bin(BinOp::Div, db, bb);
                            Some(self.bin(BinOp::Mul, ac, q))
                        }
                        None => None,
                    };
                    self.add(l, r, true)
                }
                BinOp::Rem | BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge | BinOp::Eq | BinOp::Ne | BinOp::And | BinOp::Or | BinOp::BitAnd | BinOp::BitOr | BinOp::BitXor | BinOp::Shl | BinOp::Shr => None,
            },
            ExprKind::Ctor { ty, args } => {
                let ds: Vec<Option<Expr>> = args.iter().map(|a| self.d(&a.value)).collect::<Res<_>>()?;
                if ds.iter().all(Option::is_none) {
                    return Ok(None);
                }
                let ty = match ty {
                    TyExpr::Geo(Geo::Pt | Geo::Vec, s) => TyExpr::Geo(Geo::Vec, s.clone()),
                    TyExpr::Name(_) | TyExpr::Generic(..) => ty.clone(),
                    TyExpr::Array(..) | TyExpr::Geo(..) | TyExpr::Xform(..) | TyExpr::Field(_) | TyExpr::Table(_) | TyExpr::Frame(_) => return Err(refuse("this constructor")),
                };
                let mut out = vec![];
                for (a, d) in args.iter().zip(ds) {
                    let v = match d {
                        Some(d) => d,
                        None => self.dz(&a.value)?,
                    };
                    out.push(Arg { name: None, value: v });
                }
                Some(self.e(sp, ExprKind::Ctor { ty, args: out }))
            }
            ExprKind::Call { callee, args } => return self.d_call(&callee.name, args, sp),
        })
    }

    fn enqueue(&mut self, name: &str, active: Vec<bool>) -> String {
        let symbol = jvp_name(name, &active);
        self.todo.push((name.into(), active));
        symbol
    }

    fn d_call(&mut self, f: &str, args: &[Arg], sp: Span) -> Res<Option<Expr>> {
        let a: Vec<&Expr> = args.iter().map(|a| &a.value).collect();
        let ds: Vec<Option<Expr>> = a.iter().map(|x| self.d(x)).collect::<Res<_>>()?;
        if ds.iter().all(Option::is_none) {
            return Ok(None);
        }
        let mut ds = ds;
        let mut take = |i: usize| ds[i].take();
        let c = |s: &mut Self, i: usize| s.cp(a[i]);
        let mul = |s: &mut Self, x: Expr, y: Expr| s.bin(BinOp::Mul, x, y);
        use Builtin::*;
        let builtin = lookup(f).map(|spec| spec.id);
        Ok(Some(match builtin {
            Some(Sin | Cos | Exp | Sqrt) => {
                let dx = take(0).expect("checked: varies");
                let x = c(self, 0);
                let k = match builtin {
                    Some(Sin) => self.call("cos", vec![x], sp),
                    Some(Cos) => {
                        let s = self.call("sin", vec![x], sp);
                        self.neg(s)
                    }
                    Some(Exp) => self.call("exp", vec![x], sp),
                    Some(Sqrt) => {
                        let r = self.call("sqrt", vec![x], sp);
                        let two = self.num(2.0, sp);
                        let r2 = mul(self, two, r);
                        return Ok(Some(self.bin(BinOp::Div, dx, r2)));
                    }
                    _ => unreachable!("smooth unary builtin"),
                };
                mul(self, k, dx)
            }
            Some(Pow) => {
                if take(1).is_some() {
                    return Err(Diag::new(Code::E405, sp, "`diff` of `pow` with a varying exponent", "write `exp(b * log(a))` once `log` lands, or keep the exponent constant"));
                }
                let dx = take(0).expect("varies");
                let (x, k1, k2) = (c(self, 0), c(self, 1), c(self, 1));
                let one = self.num(1.0, sp);
                let km1 = self.bin(BinOp::Sub, k2, one);
                let p = self.call("pow", vec![x, km1], sp);
                let kp = mul(self, k1, p);
                mul(self, kp, dx)
            }
            Some(Dot | Cross) => {
                let (da, db) = (take(0), take(1));
                let l = da.map(|da| {
                    let b = c(self, 1);
                    self.call(f, vec![da, b], sp)
                });
                let r = db.map(|db| {
                    let x = c(self, 0);
                    self.call(f, vec![x, db], sp)
                });
                self.add(l, r, false).expect("one varies")
            }
            Some(Length) => {
                let dv = take(0).expect("varies");
                let (v1, v2) = (c(self, 0), c(self, 0));
                let num = self.call("dot", vec![v1, dv], sp);
                let len = self.call("length", vec![v2], sp);
                self.bin(BinOp::Div, num, len)
            }
            Some(Distance) => {
                let (da, db) = (take(0), take(1));
                let dd = self.add(da, db, true).expect("one varies");
                let (p1, q1, p2, q2) = (c(self, 0), c(self, 1), c(self, 0), c(self, 1));
                let diff = self.bin(BinOp::Sub, p1, q1);
                let num = self.call("dot", vec![diff, dd], sp);
                let len = self.call("distance", vec![p2, q2], sp);
                self.bin(BinOp::Div, num, len)
            }
            Some(Normalize) => {
                // (v/|v|)' = (v' - n (n . v')) / |v|
                let dv = take(0).expect("varies");
                let (v1, v2, v3) = (c(self, 0), c(self, 0), c(self, 0));
                let n1 = self.call("normalize", vec![v1], sp);
                let n2 = self.call("normalize", vec![v2], sp);
                let dvc = self.cp(&dv);
                let along = self.call("dot", vec![n2, dvc], sp);
                let proj = mul(self, n1, along);
                let perp = self.bin(BinOp::Sub, dv, proj);
                let len = self.call("length", vec![v3], sp);
                self.bin(BinOp::Div, perp, len)
            }
            Some(Mix) => {
                // mix(a, b, t)' = mix(a', b', t) + (b - a) t'
                let (da, db, dt) = (take(0), take(1), take(2));
                let mut out = None;
                if da.is_some() || db.is_some() {
                    let da = match da {
                        Some(x) => x,
                        None => self.dz(a[0])?,
                    };
                    let db = match db {
                        Some(x) => x,
                        None => self.dz(a[1])?,
                    };
                    let t = c(self, 2);
                    out = Some(self.call("mix", vec![da, db, t], sp));
                }
                if let Some(dt) = dt {
                    let (b, x) = (c(self, 1), c(self, 0));
                    let ba = self.bin(BinOp::Sub, b, x);
                    let r = mul(self, ba, dt);
                    out = self.add(out, Some(r), false);
                }
                out.expect("one varies")
            }
            Some(op @ (Smoothstep | Smootherstep)) => {
                if take(0).is_some() || take(1).is_some() {
                    return Err(Diag::new(Code::E405, sp, format!("`diff` of `{f}` with varying edges"), "keep the edges constant"));
                }
                let dx = take(2).expect("varies");
                let (e0, e1, x) = (c(self, 0), c(self, 1), c(self, 2));
                let slope = if op == Smoothstep { DSmoothstep } else { DSmootherstep };
                let slope = self.call(spec(slope).name, vec![e0, e1, x], sp);
                mul(self, slope, dx)
            }
            Some(F32) => take(0).map(|d| self.call("f32", vec![d], sp)).expect("varies"),
            Some(U32 | I32 | LoftZero) => return Ok(None),
            Some(LoftVec) => take(0).expect("varies"),
            Some(Vec2 | Vec3 | Vec4 | Vec2f | Vec3f | Vec4f) => {
                let mut out = vec![];
                for (i, x) in a.iter().enumerate() {
                    out.push(match take(i) {
                        Some(d) => d,
                        None => self.dz(x)?,
                    });
                }
                self.call(f, out, sp)
            }
            Some(SampleC2) => {
                // The uniform cubic B-spline is C2 in t: its derivative is another table read, scaled by t'.
                let dt = take(1).expect("varies");
                let tab = c(self, 0);
                let t = c(self, 1);
                self.call("loft_dsample", vec![tab, t, dt], sp)
            }
            Some(DSample) => return Err(Diag::new(Code::E405, sp, "a second derivative of a sampled table", "Phase 1 reads first derivatives of scans")),
            Some(Abs | Min | Max | Clamp | Saturate | Floor | Fract | Sign | Step | Select | Round) => {
                return Err(Diag::new(Code::E307, sp, format!("cannot differentiate a `C0` expression: `{f}` has a kink"), "keep `diff` on the smooth part (smoothstep/smootherstep, sin, exp, ...), or move the kink outside the differentiated fn"));
            }
            None | Some(DSmoothstep | DSmootherstep | Acos | Atan2 | Fma | Log | Asin | Atan | Tan | Exp2 | Log2 | Conditional | Dist
                | Dir | Coords | Place | Inv | FrameAxes | Sample | SampleLod | SamplePrior | HasSky
                | SkyZenith | SkyHorizon | SunColor | Held | Missed | Unknown | EvalBsdf | MixBsdf
                | GgxEnergy | Dot4I8 | Dot4U8 | UnpackBf16x2 | PackBf16x2 | UnpackU4Lo | UnpackU4Hi
                | UnpackI4Lo | UnpackI4Hi | UnpackU2 | UnpackFp8E4m3 | UnpackFp8E5m2) => {
                if !self.orig.contains_key(f) && !f.contains("__jvp") && !f.contains("__d_") {
                    return Err(Diag::new(Code::E405, sp, format!("`diff` through `{f}` is not lowered yet"), "differentiate a fn written with arithmetic, calls and the smooth builtins"));
                }
                // An inactive argument still supplies its primal value. Giving it
                // a zero tangent parameter would make pow treat its exponent as live.
                let mut out: Vec<Expr> = (0..a.len()).map(|i| c(self, i)).collect();
                let active = ds.iter().map(Option::is_some).collect();
                out.extend(ds.into_iter().flatten());
                let j = self.enqueue(f, active);
                self.call(&j, out, sp)
            }
        }))
    }

    fn block(&mut self, b: &Block) -> Res<Block> {
        let scope = self.live.clone();
        let mut stmts = vec![];
        for s in &b.stmts {
            stmts.extend(self.stmt(s)?);
        }
        self.live = scope;
        Ok(Block { stmts })
    }

    fn stmt(&mut self, s: &Stmt) -> Res<Vec<Stmt>> {
        Ok(match s {
            Stmt::Let { mutable, name, ty, value } => {
                let d = if *mutable { Some(self.dz(value)?) } else { self.d(value)? };
                let mut stmts = vec![Stmt::Let {
                    mutable: *mutable, name: name.clone(), ty: ty.clone(), value: self.cp(value),
                }];
                if let Some(d) = d {
                    self.live.insert(name.name.clone());
                    let tname = Ident { name: format!("{}__t", name.name), span: name.span };
                    stmts.push(Stmt::Let { mutable: *mutable, name: tname, ty: ty.as_ref().map(tty), value: d });
                } else {
                    self.live.remove(&name.name);
                }
                stmts
            }
            Stmt::Assign { target, op, value } => {
                let ExprKind::Name(n) = &target.kind else { unreachable!("checked: names only") };
                // `n op= v` is `n = n op v`; the tangent goes first, while `n` still holds the old value.
                let rhs = match op {
                    Some(o) => {
                        let (t, v) = (self.cp(target), self.cp(value));
                        self.bin(*o, t, v)
                    }
                    None => self.cp(value),
                };
                let d = self.dz(&rhs)?;
                let tt = self.name(&format!("{n}__t"), target.span);
                let t2 = self.cp(target);
                vec![Stmt::Assign { target: tt, op: None, value: d }, Stmt::Assign { target: t2, op: None, value: rhs }]
            }
            Stmt::Return(v, span) => {
                let v = match v {
                    Some(e) => Some(self.dz(e)?),
                    None => None,
                };
                vec![Stmt::Return(v, *span)]
            }
            Stmt::If { cond, then, otherwise } => {
                let cond = self.cp(cond);
                let then = self.block(then)?;
                let otherwise = match otherwise {
                    Some(o) => Some(self.block(o)?),
                    None => None,
                };
                vec![Stmt::If { cond, then, otherwise }]
            }
            Stmt::For { comptime, var, lo, hi, body, span } => {
                let (lo, hi) = (self.cp(lo), self.cp(hi));
                let shadowed = self.live.remove(&var.name);
                let body = self.block(body)?;
                if shadowed {
                    self.live.insert(var.name.clone());
                }
                vec![Stmt::For { comptime: *comptime, var: var.clone(), lo, hi, body, span: *span }]
            }
            Stmt::Expr(e) => vec![Stmt::Expr(self.cp(e))],
            Stmt::Map { name, .. } => {
                return Err(Diag::new(Code::E405, name.span, "`diff` through a `map` statement is not lowered yet", "differentiate a fn without maps"));
            }
            Stmt::Each{span,..} | Stmt::Emit { span, .. } | Stmt::ImportTable { span, .. } => return Err(Diag::new(Code::E403, *span, "`emit` in a differentiated fn", "differentiate stage-1 code")),
        })
    }

    /// All primals, followed by tangents for the active parameters only.
    fn jvp(&mut self, f: &FnDecl, active: &[bool]) -> Res<FnDecl> {
        if f.raw.is_some(){return Err(Diag::new(Code::E307,f.name.span,"raw WGSL has no symbolic derivative","write the differentiated function in Loft"))}
        self.live.clear();
        let mut params: Vec<Param> = f.params.iter().map(|p| Param { name: p.name.clone(), ty: p.ty.clone() }).collect();
        for (p, active) in f.params.iter().zip(active) {
            if *active && let Some(t) = &p.ty {
                self.live.insert(p.name.name.clone());
                params.push(Param { name: Ident { name: format!("{}__t", p.name.name), span: p.name.span }, ty: Some(tty(t)) });
            }
        }
        let body = self.block(&f.body)?;
        Ok(FnDecl { raw: None,
            comptime: false,
            attrs: vec![],
            name: Ident { name: jvp_name(&f.name.name, active), span: f.name.span },
            params,
            ret: f.ret.as_ref().map(tty),
            body,
        })
    }
}

fn jvp_name(name: &str, active: &[bool]) -> String {
    if active.iter().all(|enabled| *enabled) {
        return format!("{name}__jvp");
    }
    let mask: String = active.iter().map(|enabled| if *enabled { '1' } else { '0' }).collect();
    format!("{name}__jvp_{mask}")
}

pub fn run(modules: &mut Vec<Module>, next: &mut u32) -> Res<()> {
    let mut top: Vec<FnDecl> = vec![];
    let mut in_kinds: Vec<(String, FnDecl)> = vec![];
    {
        let mut orig = BTreeMap::new();
        for item in modules.iter().flat_map(|m| &m.items) {
            if let Item::Fn(f) = item {
                orig.insert(f.name.name.as_str(), f);
            }
        }
        let diffs: Vec<(Ident, Ident)> = modules.iter().flat_map(|m| m.diffs.iter().cloned()).collect();
        let mut ad = Ad { orig, next, todo: vec![], live: HashSet::new() };
        let mut wrappers = HashSet::new();
        for (f, x) in &diffs {
            if !wrappers.insert((f.name.clone(), x.name.clone())) {
                continue;
            }
            let Some(decl) = ad.orig.get(f.name.as_str()).copied() else {
                return Err(Diag::new(Code::E011, f.span, format!("`diff` of unknown fn `{}`", f.name), "differentiate a top-level fn"));
            };
            let derivative = if x.name.is_empty() { decl.params.first().map(|p| &p.name.name) } else { Some(&x.name) };
            if !decl.params.iter().any(|p| Some(&p.name.name) == derivative) {
                return Err(Diag::new(Code::E011, x.span, format!("`{}` has no parameter `{}`", f.name, x.name), "name one of its parameters"));
            }
            let sp = f.span;
            let active = decl.params.iter().map(|p| Some(&p.name.name) == derivative).collect();
            let jvp = ad.enqueue(&f.name, active);
            let mut args: Vec<_> = decl.params.iter().map(|p| ad.name(&p.name.name, sp)).collect();
            args.push(ad.num(1.0, sp));
            let call = ad.call(&jvp, args, sp);
            top.push(FnDecl { raw: None,
                comptime: false,
                attrs: vec![],
                name: Ident { name: format!("{}__d_{}", f.name, x.name), span: sp },
                params: decl.params.iter().map(|p| Param { name: p.name.clone(), ty: p.ty.clone() }).collect(),
                ret: decl.ret.as_ref().map(tty),
                body: Block { stmts: vec![Stmt::Return(Some(call), sp)] },
            });
        }
        // Surface normals: every `query surface` gets its jvp inside its kind.
        for item in modules.iter().flat_map(|m| &m.items) {
            if let Item::Kind(k) = item
                // Opted-out surfaces use four finite-difference taps in geometry lowering.
                && k.proof == GeometryProof::Required
                && let Some(s) = k.query(QueryKind::Surface)
            {
                let mut j = ad.jvp(s, &vec![true; s.params.len()])?;
                j.name.name = "surface__jvp".into();
                in_kinds.push((k.name.name.clone(), j));
            }
        }
        let mut done = HashSet::new();
        while let Some((name, active)) = ad.todo.pop() {
            if !done.insert(jvp_name(&name, &active)) {
                continue;
            }
            let src = match ad.orig.get(name.as_str()) {
                Some(f) => ad.jvp(f, &active)?,
                None => {
                    let Some(i) = top.iter().position(|f| f.name.name == name) else {
                        return Err(Diag::new(Code::E405, Span::NONE, format!("`diff` reaches `{name}`, which has no body to differentiate"), "differentiate fns with bodies"));
                    };
                    let f = std::mem::replace(&mut top[i], FnDecl { raw: None, comptime: false, attrs: vec![], name: Ident { name: String::new(), span: Span::NONE }, params: vec![], ret: None, body: Block { stmts: vec![] } });
                    let j = ad.jvp(&f, &active);
                    top[i] = f;
                    j?
                }
            };
            top.push(src);
        }
    }
    for (k, f) in in_kinds {
        let kd = modules.iter_mut().flat_map(|m| m.items.iter_mut()).find_map(|item| match item {
            Item::Kind(kd) if kd.name.name == k => Some(kd),
            Item::Body(..) | Item::State(_) | Item::Solver(_) | Item::Plan(_) | Item::World(..) | Item::Bake(_) | Item::Retarget { .. } | Item::Kernel(_) | Item::Struct(..) | Item::TypeAlias(..) | Item::Build(_) | Item::Use(_) | Item::Law(_) | Item::Check(_) | Item::Camera(_) | Item::Kind(_) | Item::Space(_) | Item::Unit(_) | Item::Const(_) | Item::Fn(_) | Item::Scan(_) | Item::Schedule(_) => None,
        });
        kd.expect("the kind the jvp came from").fns.push(f);
    }
    modules.push(Module { items: top.into_iter().map(Item::Fn).collect(), ..Module::default() });
    Ok(())
}
