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
fn union(a: Term, b: Term) -> Term {
    term(a.interval.join(&b.interval), a.row || b.row)
}
fn unknown(row: bool) -> Term {
    term(
        Iv {
            lo: f64::NEG_INFINITY,
            hi: f64::INFINITY,
            l: 0.,
        },
        row,
    )
}
fn num(x: f64) -> Term {
    term(Iv::c(x), false)
}

struct Proof<'a, 'b> {
    program: &'a Prog<'b>,
    fields: Env,
    constants: Env,
    stack: Vec<String>,
}
impl Proof<'_, '_> {
    fn expr(&mut self, e: &Expr, env: &Env) -> Res<Option<Term>> {
        Ok(match &e.kind {
            ExprKind::If { condition, then, otherwise } => {
                self.expr(condition, env)?;
                let mut result: Option<Term> = None;
                for arm in [then, otherwise] {
                    let mut locals = env.clone();
                    self.block(&arm.body, &mut locals)?;
                    let next = self.expr(&arm.value, &locals)?;
                    result = match (result, next) {
                        (Some(a), Some(b)) => Some(union(a, b)),
                        (a, b) => a.or(b),
                    };
                }
                result
            }
            ExprKind::Num { value, suffix, .. } => Some(num(if *suffix == NumSuffix::Deg {
                value.to_radians()
            } else {
                *value
            })),
            ExprKind::Bool(_) => None,
            ExprKind::Name(n) => env.get(n).or_else(|| self.constants.get(n)).copied(),
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
                // A missing interval must not erase a dependency on a row column.
                let depends_on_row = a.is_some_and(|x| x.row) || b.is_some_and(|x| x.row);
                let combined = a.zip(b).and_then(|(a, b)| {
                    let (x, y) = (a.interval, b.interval);
                    Some(term(
                        match op {
                            BinOp::Add => x.add(&y),
                            BinOp::Sub => x.sub(&y),
                            BinOp::Mul => x.mul(&y),
                            BinOp::Div => x.div(&y),
                            _ => return None,
                        },
                        depends_on_row,
                    ))
                });
                combined.or_else(|| depends_on_row.then(|| unknown(true)))
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
                        ("select", [Some(a), Some(b), _]) => Some(union(*a, *b)),
                        _ if args.iter().flatten().any(|t| t.row) => Some(unknown(true)),
                        _ => None,
                    }
                }
            }
        })
    }
    fn block(&mut self, b: &Block, env: &mut Env) -> Res<Option<Term>> {
        let mut returns: Option<Term> = None;
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
                    let result = e
                        .as_ref()
                        .map(|v| self.expr(v, env))
                        .transpose()
                        .map(Option::flatten)?;
                    return Ok(match (returns, result) {
                        (Some(a), Some(b)) => Some(union(a, b)),
                        (a, b) => a.or(b),
                    });
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
                    let mut yes = env.clone();
                    let mut no = env.clone();
                    let first = self.block(then, &mut yes)?;
                    let second = if let Some(b) = otherwise {
                        self.block(b, &mut no)?
                    } else {
                        None
                    };
                    for ret in [first, second].into_iter().flatten() {
                        returns = Some(returns.map_or(ret, |v| union(v, ret)));
                    }
                    // A build-time row sample cannot choose the arm of a field proof.
                    for (name, value) in env.iter_mut() {
                        let a = yes.get(name).copied().unwrap_or(unknown(false));
                        let b = no.get(name).copied().unwrap_or(unknown(false));
                        *value = union(a, b);
                    }
                }
                Stmt::For { body, .. } | Stmt::Each { body, .. } => {
                    let mut iter = env.clone();
                    if let Some(ret) = self.block(body, &mut iter)? {
                        returns = Some(returns.map_or(ret, |v| union(v, ret)));
                    }
                    for (name, value) in env.iter_mut() {
                        let next = iter.get(name).copied().unwrap_or(unknown(false));
                        if next.interval != value.interval || next.row != value.row {
                            *value = unknown(value.row || next.row);
                        }
                    }
                }
                Stmt::Expr(e) => {
                    self.expr(e, env)?;
                }
                Stmt::Emit { .. } => {}
            }
        }
        Ok(returns)
    }
}
pub fn check(p: &Prog, types: &[Ty], overrides: &[(String, f64)]) -> Res<()> {
    let mut ev: Eval<f32> = Eval::new(p, types, vec![], overrides);
    let mut scalar = |e| as_f32(concrete(&ev.expr(e)?, e.span)?, e.span);
    let constants: Env = p.consts.iter().filter_map(|(&name, c)| {
        if !matches!(types.get(c.value.id as usize), Some(Ty::F32 | Ty::I32 | Ty::U32 | Ty::AbstractInt)) {
            return None;
        }
        scalar(&c.value).ok().map(|x| (name.to_string(), num(x.into())))
    }).collect();
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
            constants: constants.clone(),
            stack: vec![],
        }
        .block(&trace.body, &mut Env::new())?;
    }
    Ok(())
}
