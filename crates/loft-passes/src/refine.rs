//! Prove scalar row divisors over the declared domain before specializing the
//! spatial interval proof to an emitted shape. A lucky sample must not certify a column.
use crate::{
    Prog, as_f32,
    eval::{Eval, Scalar, concrete},
    lip::Iv,
};
use loft_ir::{Code, Diag, Res, ast::*};
use std::collections::HashMap;

#[derive(Clone, Copy)]
struct Term {
    interval: Iv,
    row: bool,
}
type Env = HashMap<String, Term>;
fn term(i: Iv, row: bool) -> Term {
    Term { interval: i, row }
}
fn num(x: f64) -> Term {
    term(Iv::c(x), false)
}

struct Proof<'a, 'b> {
    program: &'a Prog<'b>,
    fields: Env,
    stack: Vec<String>,
}
impl Proof<'_, '_> {
    fn expr(&mut self, e: &Expr, env: &Env) -> Res<Option<Term>> {
        Ok(match &e.kind {
            ExprKind::Num { value, suffix, .. } => Some(num(if *suffix == NumSuffix::Deg {
                value.to_radians()
            } else {
                *value
            })),
            ExprKind::Bool(_) => None,
            ExprKind::Name(n) => env.get(n).copied(),
            ExprKind::Member(b, f) => {
                self.expr(b, env)?;
                if matches!(&b.kind,ExprKind::Name(n) if n=="self") {
                    self.fields.get(&f.name).copied()
                } else {
                    None
                }
            }
            ExprKind::Unary(UnOp::Neg, v) => {
                self.expr(v, env)?.map(|v| term(v.interval.neg(), v.row))
            }
            ExprKind::Unary(UnOp::Not, v) => {
                self.expr(v, env)?;
                None
            }
            ExprKind::Index(a, b) => {
                self.expr(a, env)?;
                self.expr(b, env)?;
                None
            }
            ExprKind::Ctor { args, .. } => {
                for a in args {
                    self.expr(&a.value, env)?;
                }
                None
            }
            ExprKind::Binary(op, l, r) => {
                let a = self.expr(l, env)?;
                let b = self.expr(r, env)?;
                if *op == BinOp::Div
                    && let Some(den) = b
                    && den.row
                    && den.interval.lo <= 0.
                    && den.interval.hi >= 0.
                {
                    return Err(Diag::new(
                        Code::E211,
                        r.span,
                        "row divisor can reach zero over its declared range",
                        "declare a nonzero column range or clamp the divisor away from zero",
                    ));
                }
                a.zip(b).and_then(|(a, b)| {
                    let (x, y) = (a.interval, b.interval);
                    Some(term(
                        match op {
                            BinOp::Add => x.add(&y),
                            BinOp::Sub => x.sub(&y),
                            BinOp::Mul => x.mul(&y),
                            BinOp::Div => x.div(&y),
                            _ => return None,
                        },
                        a.row || b.row,
                    ))
                })
            }
            ExprKind::Call { callee, args } => {
                let args = args
                    .iter()
                    .map(|a| self.expr(&a.value, env))
                    .collect::<Res<Vec<_>>>()?;
                if let Some(f) = self.program.fns.get(callee.name.as_str()) {
                    if self.stack.contains(&callee.name) {
                        return Err(Diag::new(
                            Code::E211,
                            callee.span,
                            "recursive trace has no finite derivation",
                            "replace recursion with a bounded expression",
                        ));
                    }
                    self.stack.push(callee.name.clone());
                    let mut locals = f
                        .params
                        .iter()
                        .zip(args)
                        .filter_map(|(p, t)| t.map(|x| (p.name.name.clone(), x)))
                        .collect();
                    let result = self.block(&f.body, &mut locals)?;
                    self.stack.pop();
                    result
                } else {
                    match (callee.name.as_str(), args.as_slice()) {
                        ("max", [Some(a), Some(b)]) => {
                            Some(term(a.interval.max(&b.interval), a.row || b.row))
                        }
                        ("min", [Some(a), Some(b)]) => {
                            Some(term(a.interval.min(&b.interval), a.row || b.row))
                        }
                        ("clamp", [Some(a), Some(b), Some(c)]) => Some(term(
                            a.interval.max(&b.interval).min(&c.interval),
                            a.row || b.row || c.row,
                        )),
                        ("abs", [Some(a)]) => {
                            Some(term(a.interval.un(crate::eval::Un::Abs), a.row))
                        }
                        _ => None,
                    }
                }
            }
        })
    }
    fn block(&mut self, b: &Block, env: &mut Env) -> Res<Option<Term>> {
        for s in &b.stmts {
            match s {
                Stmt::Let { name, value, .. } => {
                    if let Some(t) = self.expr(value, env)? {
                        env.insert(name.name.clone(), t);
                    } else {
                        env.remove(&name.name);
                    }
                }
                Stmt::Assign { target, value, .. } => {
                    let t = self.expr(value, env)?;
                    if let ExprKind::Name(n) = &target.kind {
                        env.remove(n);
                        if let Some(t) = t {
                            env.insert(n.clone(), t);
                        }
                    }
                }
                Stmt::Return(e, _) => {
                    return e
                        .as_ref()
                        .map(|v| self.expr(v, env))
                        .transpose()
                        .map(Option::flatten);
                }
                Stmt::Map { head, stages, .. } => {
                    self.expr(head, env)?;
                    for a in stages.iter().flat_map(|(_, v)| v) {
                        self.expr(a, env)?;
                    }
                }
                Stmt::If {
                    cond,
                    then,
                    otherwise,
                } => {
                    self.expr(cond, env)?;
                    self.block(then, &mut env.clone())?;
                    if let Some(b) = otherwise {
                        self.block(b, &mut env.clone())?;
                    }
                }
                Stmt::For { body, .. } | Stmt::Each { body, .. } => {
                    self.block(body, &mut env.clone())?;
                }
                Stmt::Expr(e) => {
                    self.expr(e, env)?;
                }
                Stmt::Emit { .. } => {}
            }
        }
        Ok(None)
    }
}
pub fn check(p: &Prog, types: &[Ty], overrides: &[(String, f64)]) -> Res<()> {
    let mut ev: Eval<f32> = Eval::new(p, types, vec![], overrides);
    let mut scalar = |e| as_f32(concrete(&ev.expr(e)?, e.span)?, e.span);
    for k in p.kinds.values() {
        let Some(trace) = k.query(QueryKind::Trace) else {
            continue;
        };
        let mut fields = Env::new();
        for f in &k.row {
            if !matches!(&f.ty,TyExpr::Name(n) if matches!(n.name.as_str(),"f32"|"i32"|"u32")) {
                continue;
            }
            let (lo, hi) = match &f.range {
                None => (f32::NEG_INFINITY, f32::INFINITY),
                Some(r) => {
                    let x = scalar(&r.lo)?;
                    (
                        if r.strict { x.next_up() } else { x },
                        r.hi.as_ref()
                            .map(&mut scalar)
                            .transpose()?
                            .unwrap_or(f32::INFINITY),
                    )
                }
            };
            fields.insert(
                f.name.name.clone(),
                term(
                    Iv {
                        lo: lo.into(),
                        hi: hi.into(),
                        l: 0.,
                    },
                    true,
                ),
            );
        }
        Proof {
            program: p,
            fields,
            stack: vec![],
        }
        .block(&trace.body, &mut Env::new())?;
    }
    Ok(())
}
