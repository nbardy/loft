//! Forward-mode AD (draft §2C) as a source-to-source pass, run before indexing so the checker types the result and
//! every backend (stage 0, the CPU twin, interval bounds, WGSL) runs the same derivative code.
//!
//! `diff(f, x)(args)` was parsed as a call to `f__d_x`; this pass writes that fn as `f__jvp` (f's body with a tangent
//! beside every value) seeded with 1 in `x`. A kind's `query surface` gets `surface__jvp` inside the kind, which the
//! compiler calls for the surface normal. Rules are the textbook ones; a `C0` builtin (abs, min, clamp, floor, ...) is
//! E307: differentiating it needs a choice the author has not made.

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
    todo: Vec<String>,
    /// Names with a tangent in the fn being transformed.
    live: HashSet<String>,
}

const C0: [&str; 11] = ["abs", "min", "max", "clamp", "saturate", "floor", "fract", "sign", "step", "select", "round"];

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
            ExprKind::Unproved { .. } | ExprKind::FiniteDiff { .. } => return Err(refuse("an author opt-out")),
            ExprKind::If { .. } => return Err(Diag::new(Code::E307, x.span, "conditional expression has no smooth derivative", "use a smooth blend")),
            ExprKind::Num { .. } | ExprKind::Bool(_) => None,
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

    fn d_call(&mut self, f: &str, args: &[Arg], sp: Span) -> Res<Option<Expr>> {
        let a: Vec<&Expr> = args.iter().map(|a| &a.value).collect();
        let ds: Vec<Option<Expr>> = a.iter().map(|x| self.d(x)).collect::<Res<_>>()?;
        if ds.iter().all(Option::is_none) {
            return Ok(None);
        }
        if C0.contains(&f) {
            return Err(Diag::new(Code::E307, sp, format!("cannot differentiate a `C0` expression: `{f}` has a kink"), "keep `diff` on the smooth part (smoothstep/smootherstep, sin, exp, ...), or move the kink outside the differentiated fn"));
        }
        let mut ds = ds;
        let mut take = |i: usize| ds[i].take();
        let c = |s: &mut Self, i: usize| s.cp(a[i]);
        let mul = |s: &mut Self, x: Expr, y: Expr| s.bin(BinOp::Mul, x, y);
        Ok(Some(match f {
            "sin" | "cos" | "exp" | "sqrt" => {
                let dx = take(0).expect("checked: varies");
                let x = c(self, 0);
                let k = match f {
                    "sin" => self.call("cos", vec![x], sp),
                    "cos" => {
                        let s = self.call("sin", vec![x], sp);
                        self.neg(s)
                    }
                    "exp" => self.call("exp", vec![x], sp),
                    _ => {
                        let r = self.call("sqrt", vec![x], sp);
                        let two = self.num(2.0, sp);
                        let r2 = mul(self, two, r);
                        return Ok(Some(self.bin(BinOp::Div, dx, r2)));
                    }
                };
                mul(self, k, dx)
            }
            "pow" => {
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
            "dot" | "cross" => {
                let (da, db) = (take(0), take(1));
                let l = match da {
                    Some(da) => {
                        let b = c(self, 1);
                        Some(self.call(f, vec![da, b], sp))
                    }
                    None => None,
                };
                let r = match db {
                    Some(db) => {
                        let x = c(self, 0);
                        Some(self.call(f, vec![x, db], sp))
                    }
                    None => None,
                };
                self.add(l, r, false).expect("one varies")
            }
            "length" => {
                let dv = take(0).expect("varies");
                let (v1, v2) = (c(self, 0), c(self, 0));
                let num = self.call("dot", vec![v1, dv], sp);
                let len = self.call("length", vec![v2], sp);
                self.bin(BinOp::Div, num, len)
            }
            "distance" => {
                let (da, db) = (take(0), take(1));
                let dd = self.add(da, db, true).expect("one varies");
                let (p1, q1, p2, q2) = (c(self, 0), c(self, 1), c(self, 0), c(self, 1));
                let diff = self.bin(BinOp::Sub, p1, q1);
                let num = self.call("dot", vec![diff, dd], sp);
                let len = self.call("distance", vec![p2, q2], sp);
                self.bin(BinOp::Div, num, len)
            }
            "normalize" => {
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
            "mix" => {
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
            "smoothstep" | "smootherstep" => {
                if take(0).is_some() || take(1).is_some() {
                    return Err(Diag::new(Code::E405, sp, format!("`diff` of `{f}` with varying edges"), "keep the edges constant"));
                }
                let dx = take(2).expect("varies");
                let (e0, e1, x) = (c(self, 0), c(self, 1), c(self, 2));
                let slope = self.call(&format!("loft_d{f}"), vec![e0, e1, x], sp);
                mul(self, slope, dx)
            }
            "f32" => take(0).map(|d| self.call("f32", vec![d], sp)).expect("varies"),
            "u32" | "i32" | "loft_zero" => return Ok(None),
            "loft_vec" => take(0).expect("varies"),
            "vec2" | "vec3" | "vec4" | "vec2f" | "vec3f" | "vec4f" => {
                let mut out = vec![];
                for (i, x) in a.iter().enumerate() {
                    out.push(match take(i) {
                        Some(d) => d,
                        None => self.dz(x)?,
                    });
                }
                self.call(f, out, sp)
            }
            "sample_c2" => {
                // The uniform cubic B-spline is C2 in t: its derivative is another table read, scaled by t'.
                let dt = take(1).expect("varies");
                let tab = c(self, 0);
                let t = c(self, 1);
                self.call("loft_dsample", vec![tab, t, dt], sp)
            }
            "loft_dsample" => return Err(Diag::new(Code::E405, sp, "a second derivative of a sampled table", "Phase 1 reads first derivatives of scans")),
            user if self.orig.contains_key(user) || user.ends_with("__jvp") || user.contains("__d_") => {
                // g(a)' = g__jvp(a, a')
                let mut out: Vec<Expr> = (0..a.len()).map(|i| c(self, i)).collect();
                for (i, x) in a.iter().enumerate() {
                    out.push(match take(i) {
                        Some(d) => d,
                        None => self.dz(x)?,
                    });
                }
                let j = format!("{user}__jvp");
                if !self.todo.contains(&user.to_string()) {
                    self.todo.push(user.to_string());
                }
                self.call(&j, out, sp)
            }
            other => return Err(Diag::new(Code::E405, sp, format!("`diff` through `{other}` is not lowered yet"), "differentiate a fn written with arithmetic, calls and the smooth builtins")),
        }))
    }

    fn block(&mut self, b: &Block) -> Res<Block> {
        let mut stmts = vec![];
        for s in &b.stmts {
            stmts.extend(self.stmt(s)?);
        }
        Ok(Block { stmts })
    }

    fn stmt(&mut self, s: &Stmt) -> Res<Vec<Stmt>> {
        Ok(match s {
            Stmt::Let { mutable, name, ty, value } => {
                let d = self.dz(value)?;
                let v = self.cp(value);
                self.live.insert(name.name.clone());
                let tname = Ident { name: format!("{}__t", name.name), span: name.span };
                vec![
                    Stmt::Let { mutable: *mutable, name: name.clone(), ty: ty.clone(), value: v },
                    Stmt::Let { mutable: *mutable, name: tname, ty: ty.as_ref().map(tty), value: d },
                ]
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
                let body = self.block(body)?;
                vec![Stmt::For { comptime: *comptime, var: var.clone(), lo, hi, body, span: *span }]
            }
            Stmt::Expr(e) => vec![Stmt::Expr(self.cp(e))],
            Stmt::Map { name, .. } => {
                return Err(Diag::new(Code::E405, name.span, "`diff` through a `map` statement is not lowered yet", "differentiate a fn without maps"));
            }
            Stmt::Each{span,..} | Stmt::Emit { span, .. } => return Err(Diag::new(Code::E403, *span, "`emit` in a differentiated fn", "differentiate stage-1 code")),
        })
    }

    /// `f__jvp(params..., tangents...)`: f's value derivative along the given tangents.
    fn jvp(&mut self, f: &FnDecl) -> Res<FnDecl> {
        if f.raw.is_some(){return Err(Diag::new(Code::E307,f.name.span,"raw WGSL has no symbolic derivative","write the differentiated function in Loft"))}
        self.live.clear();
        let mut params: Vec<Param> = f.params.iter().map(|p| Param { name: p.name.clone(), ty: p.ty.clone() }).collect();
        for p in &f.params {
            if let Some(t) = &p.ty {
                self.live.insert(p.name.name.clone());
                params.push(Param { name: Ident { name: format!("{}__t", p.name.name), span: p.name.span }, ty: Some(tty(t)) });
            }
        }
        let body = self.block(&f.body)?;
        Ok(FnDecl { raw: None,
            comptime: false,
            attrs: vec![],
            name: Ident { name: format!("{}__jvp", f.name.name), span: f.name.span },
            params,
            ret: f.ret.as_ref().map(tty),
            body,
        })
    }
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
            // f__d_x(p...) = f__jvp(p..., [1 in x, 0 elsewhere])
            let sp = f.span;
            let mut args = vec![];
            for p in &decl.params {
                args.push(ad.name(&p.name.name, sp));
            }
            for p in &decl.params {
                let t = if Some(&p.name.name) == derivative {
                    ad.num(1.0, sp)
                } else {
                    let n = ad.name(&p.name.name, sp);
                    ad.call("loft_zero", vec![n], sp)
                };
                args.push(t);
            }
            let call = ad.call(&format!("{}__jvp", f.name), args, sp);
            top.push(FnDecl { raw: None,
                comptime: false,
                attrs: vec![],
                name: Ident { name: format!("{}__d_{}", f.name, x.name), span: sp },
                params: decl.params.iter().map(|p| Param { name: p.name.clone(), ty: p.ty.clone() }).collect(),
                ret: decl.ret.as_ref().map(tty),
                body: Block { stmts: vec![Stmt::Return(Some(call), sp)] },
            });
            if !ad.todo.contains(&f.name) {
                ad.todo.push(f.name.clone());
            }
        }
        // Surface normals: every `query surface` gets its jvp inside its kind.
        for item in modules.iter().flat_map(|m| &m.items) {
            if let Item::Kind(k) = item
                // Opted-out surfaces use four finite-difference taps in geometry lowering.
                && k.proof == GeometryProof::Required
                && let Some(s) = k.query(QueryKind::Surface)
            {
                let mut j = ad.jvp(s)?;
                j.name.name = "surface__jvp".into();
                in_kinds.push((k.name.name.clone(), j));
            }
        }
        let mut done = HashSet::new();
        while let Some(name) = ad.todo.pop() {
            if !done.insert(name.clone()) {
                continue;
            }
            let src = match ad.orig.get(name.as_str()) {
                Some(f) => ad.jvp(f)?,
                None => {
                    let Some(i) = top.iter().position(|f| f.name.name == name) else {
                        return Err(Diag::new(Code::E405, Span::NONE, format!("`diff` reaches `{name}`, which has no body to differentiate"), "differentiate fns with bodies"));
                    };
                    let f = std::mem::replace(&mut top[i], FnDecl { raw: None, comptime: false, attrs: vec![], name: Ident { name: String::new(), span: Span::NONE }, params: vec![], ret: None, body: Block { stmts: vec![] } });
                    let j = ad.jvp(&f);
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
            Item::Kernel(_) | Item::Bake(_) | Item::Struct(..) | Item::TypeAlias(..) | Item::Build(_) | Item::Use(_) | Item::Law(_) | Item::Check(_) | Item::Camera(_) | Item::World(..) | Item::Kind(_) | Item::Space(_) | Item::Unit(_) | Item::Const(_) | Item::Fn(_) | Item::Scan(_) | Item::Schedule(_) => None,
        });
        kd.expect("the kind the jvp came from").fns.push(f);
    }
    modules.push(Module { items: top.into_iter().map(Item::Fn).collect(), ..Module::default() });
    Ok(())
}
