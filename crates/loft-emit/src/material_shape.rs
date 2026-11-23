//! A material's layout depends on the shape of its staged lobe expression.
//!
//! A Bsdf value is a flat struct: each lobe's bound arguments in lobe order, then each `mix_bsdf` weight.
//! Values compose: `a + b` and `mix_bsdf(a, b, t)` concatenate the fields of `a` and `b`, so a let-bound
//! Bsdf, or a helper that returns one, can sit inside a larger stack. A struct is named by its fields alone;
//! the kind's material struct is `Bsdf`, and only the material's shape is ever evaluated.
use loft_check::Prog;
use loft_ir::{Code, Diag, Res};
use loft_parse::ast::{BinOp, Block, Expr, ExprKind, FnDecl, Stmt, TyExpr};
use std::collections::BTreeSet;
use std::fmt::Write;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum Op {
    Lobe(usize),
    Layer,
    Mix(usize),
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(super) struct Shape {
    /// Each lobe fn and its bound-argument count.
    pub lobes: Vec<(String, usize)>,
    pub weights: usize,
    ops: Vec<Op>,
}

impl Shape {
    pub fn lobe(name: &str, arity: usize) -> Self {
        Shape { lobes: vec![(name.to_owned(), arity)], weights: 0, ops: vec![Op::Lobe(0)] }
    }

    /// `a + b` (layer) or `mix_bsdf(a, b, t)`: b's lobes and weights follow a's, and the mix weight comes last.
    pub fn join(a: Shape, b: Shape, mix: bool) -> Self {
        let (lobes, weights) = (a.lobes.len(), a.weights);
        let mut ops = a.ops;
        ops.extend(b.ops.into_iter().map(|op| match op {
            Op::Lobe(i) => Op::Lobe(i + lobes),
            Op::Mix(k) => Op::Mix(k + weights),
            Op::Layer => Op::Layer,
        }));
        let weights = a.weights + b.weights + usize::from(mix);
        ops.push(if mix { Op::Mix(weights - 1) } else { Op::Layer });
        let mut all = a.lobes;
        all.extend(b.lobes);
        Shape { lobes: all, weights, ops }
    }

    /// Two return paths can share one struct and one evaluation.
    pub fn same_as(&self, other: &Self) -> bool {
        self.lobes == other.lobes && (self.weights == 0 && other.weights == 0 || self.ops == other.ops)
    }

    /// The struct a value of this shape is stored in: its fields, not how they are combined.
    pub fn key(&self) -> String {
        format!("{:?}/{}", self.lobes, self.weights)
    }

    pub fn eval(&self, shader: &mut String) -> String {
        let mut stack = Vec::new();
        for (i, op) in self.ops.iter().enumerate() {
            let result = format!("m{i}");
            match *op {
                Op::Lobe(lobe) => {
                    let r = format!("r{lobe}");
                    if self.lobes[lobe].0 == "emit_lobe" {
                        let _ = writeln!(shader, "  let {result} = LoftMaterial(vec3<f32>(0.0), 1.0, {r}.xyz);");
                    } else {
                        let _ = writeln!(shader, "  let {result} = LoftMaterial({r}.xyz, {r}.w, vec3<f32>(0.0));");
                    }
                }
                Op::Layer | Op::Mix(_) => {
                    let b = stack.pop().expect("upper material");
                    let a = stack.pop().expect("base material");
                    let value = match *op {
                        Op::Layer => format!("LoftMaterial({a}.c*{b}.t+{b}.c, {a}.t*{b}.t, {a}.e+{b}.e)"),
                        Op::Mix(weight) => {
                            let t = format!("clamp(b.mix{weight},0.0,1.0)");
                            format!("LoftMaterial(mix({a}.c,{b}.c,{t}), mix({a}.t,{b}.t,{t}), mix({a}.e,{b}.e,{t}))")
                        }
                        Op::Lobe(_) => unreachable!("a lobe is a leaf"),
                    };
                    let _ = writeln!(shader, "  let {result} = {value};");
                }
            }
            stack.push(result);
        }
        let material = stack.pop().expect("static material");
        format!("{material}.c+{material}.e")
    }
}

pub(super) fn is_lobe(p: &Prog, name: &str) -> bool {
    p.fns.get(name).is_some_and(|f| f.attrs.iter().any(|a| a.name.name == "lobe"))
}

pub(super) fn returns_bsdf(f: &FnDecl) -> bool {
    matches!(&f.ret, Some(TyExpr::Name(n)) if n.name == "Bsdf")
}

fn no_layout(e: &Expr) -> Diag {
    Diag::new(Code::E405, e.span, "material has no static lobe layout",
        "build it from lobes, `+`, `mix_bsdf`, Bsdf lets and helpers that return a Bsdf")
}

/// The shape of a Bsdf expression. `local` gives the shape of a Bsdf let or parameter in scope.
pub(super) fn shape_of(p: &Prog, e: &Expr, local: &dyn Fn(&str) -> Option<Shape>, stack: &mut BTreeSet<String>) -> Res<Shape> {
    match &e.kind {
        ExprKind::Binary(BinOp::Add, a, b) => Ok(Shape::join(shape_of(p, a, local, stack)?, shape_of(p, b, local, stack)?, false)),
        ExprKind::Call { callee, args } if callee.name == "mix_bsdf" => {
            let [a, b, _] = args.as_slice() else { return Err(no_layout(e)) };
            Ok(Shape::join(shape_of(p, &a.value, local, stack)?, shape_of(p, &b.value, local, stack)?, true))
        }
        ExprKind::Call { callee, args } if is_lobe(p, &callee.name) => Ok(Shape::lobe(&callee.name, args.len())),
        ExprKind::Call { callee, .. } => match p.fns.get(callee.name.as_str()) {
            Some(f) if returns_bsdf(f) => fn_shape(p, f, stack),
            Some(_) | None => Err(no_layout(e)),
        },
        ExprKind::Name(n) => local(n).ok_or_else(|| no_layout(e)),
        ExprKind::Unproved { .. } | ExprKind::FiniteDiff { .. } | ExprKind::If { .. } | ExprKind::Num { .. }
        | ExprKind::Bool(_) | ExprKind::Member(..) | ExprKind::Index(..) | ExprKind::Ctor { .. }
        | ExprKind::Binary(..) | ExprKind::Unary(..) | ExprKind::String(_) => Err(no_layout(e)),
    }
}

/// The one shape every `return` of a Bsdf fn builds (it fixes the fn's struct).
pub(super) fn fn_shape(p: &Prog, f: &FnDecl, stack: &mut BTreeSet<String>) -> Res<Shape> {
    if !stack.insert(f.name.name.clone()) {
        return Err(Diag::new(Code::E405, f.name.span, "recursive material helper", "return a finite lobe stack"));
    }
    let mut shape = None;
    walk(p, &f.body, &mut Vec::new(), stack, &mut shape)?;
    stack.remove(&f.name.name);
    shape.ok_or_else(|| Diag::new(Code::E003, f.name.span, "the material returns nothing", "return a sum of lobes"))
}

/// A let is a Bsdf when its value is built by a Bsdf call (an error there is the author's); a sum or a name
/// is one only if it resolves, since `a + b` may be arithmetic.
fn let_shape(p: &Prog, e: &Expr, env: &[(String, Option<Shape>)], stack: &mut BTreeSet<String>) -> Res<Option<Shape>> {
    let local = |n: &str| env.iter().rev().find(|(name, _)| name == n).and_then(|(_, s)| s.clone());
    let bsdf_call = matches!(&e.kind, ExprKind::Call { callee, .. } if callee.name == "mix_bsdf" || is_lobe(p, &callee.name)
        || p.fns.get(callee.name.as_str()).is_some_and(|f| returns_bsdf(f)));
    if bsdf_call {
        return shape_of(p, e, &local, stack).map(Some);
    }
    Ok(shape_of(p, e, &local, stack).ok())
}

fn walk(p: &Prog, b: &Block, env: &mut Vec<(String, Option<Shape>)>, stack: &mut BTreeSet<String>, out: &mut Option<Shape>) -> Res<()> {
    let mark = env.len();
    for s in &b.stmts {
        match s {
            Stmt::Let { name, value, .. } => {
                let shape = let_shape(p, value, env, stack)?;
                env.push((name.name.clone(), shape));
            }
            Stmt::Assign { target, value, .. } => {
                let ExprKind::Name(n) = &target.kind else { continue };
                let Some(Some(was)) = env.iter().rev().find(|(name, _)| name == n).map(|(_, s)| s.clone()) else { continue };
                if !let_shape(p, value, env, stack)?.is_some_and(|now| now.key() == was.key()) {
                    return Err(Diag::new(Code::E405, value.span, "a Bsdf var keeps the lobes it was declared with",
                        "bind the new stack with its own `let`"));
                }
            }
            Stmt::Return(Some(e), _) => {
                let local = |n: &str| env.iter().rev().find(|(name, _)| name == n).and_then(|(_, s)| s.clone());
                let shape = shape_of(p, e, &local, stack)?;
                if !out.as_ref().is_none_or(|s| s.same_as(&shape)) {
                    return Err(Diag::new(Code::E405, e.span, "material paths use different lobes", "keep the same lobes on every path and vary their weights"));
                }
                *out = Some(shape);
            }
            Stmt::If { then, otherwise, .. } => {
                walk(p, then, env, stack, out)?;
                if let Some(o) = otherwise { walk(p, o, env, stack, out)?; }
            }
            Stmt::Each { body, .. } | Stmt::For { body, .. } => walk(p, body, env, stack, out)?,
            Stmt::Return(None, _) | Stmt::Map { .. } | Stmt::Emit { .. } | Stmt::ImportTable { .. } | Stmt::Expr(_) => {}
        }
    }
    env.truncate(mark);
    Ok(())
}
