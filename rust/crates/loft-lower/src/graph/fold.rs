use super::{Arena, Expr, LoopId, Op, Prim, Ty, rewrite::Rewrite};
use crate::Error;
use std::collections::HashMap;

type Known = Vec<f64>;

#[derive(Default)]
pub(super) struct Folded {
    expr: HashMap<Expr, Expr>,
    value: HashMap<Expr, Known>,
    loops: HashMap<LoopId, Vec<Expr>>,
}

fn f32_round(x: f64) -> f64 {
    f64::from(x as f32)
}

fn dot(a: &[f64], b: &[f64]) -> f64 {
    a.iter().enumerate().fold(0.0, |s, (i, &x)| f32_round(s + f32_round(x * b.get(i).copied().unwrap_or(f64::NAN))))
}

fn sign(x: f64) -> f64 {
    if x == 0.0 || x.is_nan() { x } else { x.signum() }
}

fn min(a: f64, b: f64) -> f64 {
    if a.is_nan() || b.is_nan() {
        return f64::NAN;
    }
    if a == b && b.is_sign_negative() {
        return b;
    }
    if b < a { b } else { a }
}

fn max(a: f64, b: f64) -> f64 {
    if a.is_nan() || b.is_nan() {
        return f64::NAN;
    }
    if a == b && b.is_sign_positive() {
        return b;
    }
    if b > a { b } else { a }
}

fn pow(x: f64, y: f64) -> f64 {
    // Math.pow gives NaN for a unit base with an infinite exponent, and for NaN exponents even at base 1.
    if y.is_nan() || (x.abs() == 1.0 && y.is_infinite()) { f64::NAN } else { x.powf(y) }
}

fn each(prim: Prim, a: f64, b: f64, c: f64) -> f64 {
    use Prim::*;
    match prim {
        Add => a + b,
        Sub => a - b,
        Mul => a * b,
        Div => a / b,
        Min => min(a, b),
        Max => max(a, b),
        Pow => pow(a, b),
        Atan2 => a.atan2(b),
        Mod => a - b * (a / b).floor(),
        Step => f64::from(b >= a),
        Neg => -a,
        Abs => a.abs(),
        Sign => sign(a),
        Floor => a.floor(),
        Sqrt => a.sqrt(),
        Exp => a.exp(),
        Log => a.ln(),
        Sin => a.sin(),
        Cos => a.cos(),
        Tan => a.tan(),
        Fract => a - a.floor(),
        Saturate => min(1.0, max(0.0, a)),
        Mix => a + (b - a) * c,
        Clamp => min(c, max(b, a)),
        Smoothstep => {
            let t = min(1.0, max(0.0, (c - a) / (b - a)));
            t * t * (3.0 - 2.0 * t)
        }
        Lt => f64::from(a < b),
        Gt => f64::from(a > b),
        Le => f64::from(a <= b),
        Ge => f64::from(a >= b),
        Eq => f64::from(a == b),
        Ne => f64::from(a != b),
        And => f64::from(a != 0.0 && b != 0.0),
        Or => f64::from(a != 0.0 || b != 0.0),
        Not => f64::from(a == 0.0),
        Ceil => a.ceil(),
        Trunc => a.trunc(),
        RoundEven => a.round_ties_even(),
        // JavaScript % is the truncated remainder, as WGSL's on floats.
        Rem => a % b,
        InverseSqrt => 1.0 / a.sqrt(),
        Exp2 => pow(2.0, a),
        Log2 => a.log2(),
        Asin => a.asin(),
        Acos => a.acos(),
        Atan => a.atan(),
        Sinh => a.sinh(),
        Cosh => a.cosh(),
        Tanh => a.tanh(),
        Normalize | Length | Distance | Dot | Cross | Select => unreachable!("handled by fold_call"),
    }
}

fn fold_call(prim: Prim, xs: &[&[f64]]) -> Known {
    let at = |cs: &[f64], i| if cs.len() == 1 { cs[0] } else { cs.get(i).copied().unwrap_or(f64::NAN) };
    let x = xs[0];
    match prim {
        Prim::Normalize if x.len() == 1 => vec![sign(x[0])],
        Prim::Normalize => {
            let len = f32_round(dot(x, x).sqrt());
            x.iter().map(|&x| f32_round(x / len)).collect()
        }
        Prim::Length => vec![f32_round(dot(x, x).sqrt())],
        Prim::Distance => {
            let d: Vec<_> =
                x.iter().enumerate().map(|(i, &x)| f32_round(x - xs[1].get(i).copied().unwrap_or(f64::NAN))).collect();
            vec![f32_round(dot(&d, &d).sqrt())]
        }
        Prim::Dot => vec![dot(x, xs[1])],
        Prim::Cross => {
            let y = xs[1];
            let term = |i, j| f32_round(f32_round(x[i] * y[j]) - f32_round(x[j] * y[i]));
            vec![term(1, 2), term(2, 0), term(0, 1)]
        }
        Prim::Select => {
            let yes = xs[1];
            let no = xs[2];
            let branch = if x[0] != 0.0 { yes } else { no };
            (0..yes.len().max(no.len())).map(|i| at(branch, i)).collect()
        }
        Prim::Add
        | Prim::Sub
        | Prim::Mul
        | Prim::Div
        | Prim::Min
        | Prim::Max
        | Prim::Pow
        | Prim::Atan2
        | Prim::Mod
        | Prim::Step
        | Prim::Neg
        | Prim::Abs
        | Prim::Sign
        | Prim::Floor
        | Prim::Sqrt
        | Prim::Exp
        | Prim::Log
        | Prim::Sin
        | Prim::Cos
        | Prim::Tan
        | Prim::Fract
        | Prim::Saturate
        | Prim::Mix
        | Prim::Clamp
        | Prim::Smoothstep
        | Prim::Lt
        | Prim::Gt
        | Prim::Le
        | Prim::Ge
        | Prim::Eq
        | Prim::Ne
        | Prim::And
        | Prim::Or
        | Prim::Not
        | Prim::Ceil
        | Prim::Trunc
        | Prim::RoundEven
        | Prim::Rem
        | Prim::InverseSqrt
        | Prim::Exp2
        | Prim::Log2
        | Prim::Asin
        | Prim::Acos
        | Prim::Atan
        | Prim::Sinh
        | Prim::Cosh
        | Prim::Tanh => {
            let width = xs.iter().map(|x| x.len()).max().unwrap();
            let b = xs.get(1).copied().unwrap_or(&[]);
            let c = xs.get(2).copied().unwrap_or(&[]);
            // Compound ops run in f64 and round once at the result, just like expr.ts EACH.
            (0..width).map(|i| f32_round(each(prim, at(x, i), at(b, i), at(c, i)))).collect()
        }
    }
}

impl Arena {
    fn fold_node(&self, e: Expr) -> Option<Known> {
        let n = &self[e];
        let kid = |e: Expr| self.folded.value.get(&e).map(Vec::as_slice);
        match &n.op {
            Op::Const(v) if n.ty == Ty::Bool => Some(vec![f64::from(v[0] != 0.0)]),
            Op::Const(v) => Some(v.iter().copied().map(f32_round).collect()),
            Op::Var(_) | Op::Input(_) | Op::Param(_) | Op::Row { .. } | Op::Fetch { .. } | Op::Iterate { .. } => None,
            Op::Vec(args) => {
                let mut flat = Vec::with_capacity(n.ty.lits() as usize);
                for &a in args {
                    flat.extend_from_slice(kid(a)?);
                }
                if flat.len() == 1 {
                    flat.resize(n.ty.lits() as usize, flat[0]);
                }
                Some(flat)
            }
            Op::Get { a, sel } => {
                let v = kid(*a)?;
                Some(sel.bytes().map(|ch| v[b"xyzw".iter().position(|&c| c == ch).unwrap()]).collect())
            }
            Op::Lib { fun, args } => {
                let js = self.lib_fn(*fun).js?;
                let args = args.iter().map(|&a| kid(a).map(<[f64]>::to_vec)).collect::<Option<Vec<_>>>()?;
                Some(js(&args).ok()?.into_iter().map(f32_round).collect())
            }
            Op::Call { prim, args } => {
                let xs = args.iter().copied().map(kid).collect::<Option<Vec<_>>>()?;
                Some(fold_call(*prim, &xs))
            }
        }
    }

    /// Constant subgraphs are evaluated in f32. A subgraph shared across programs is folded once.
    pub fn fold(&mut self, roots: &[Expr]) -> Result<Vec<Expr>, Error> {
        roots.iter().map(|&e| self.fold_visit(e)).collect()
    }

    fn fold_visit(&mut self, e: Expr) -> Result<Expr, Error> {
        if let Some(&out) = self.folded.expr.get(&e) {
            return Ok(out);
        }
        let kids = self.children(e).to_vec();
        let to = self.fold(&kids)?;
        let out = if let Some(v) = self.fold_node(e) {
            // Overflow is still known (x / infinity can be finite), but WGSL has no literal for it.
            self.folded.value.insert(e, v.clone());
            if v.iter().all(|x| x.is_finite()) {
                let out = if matches!(self[e].op, Op::Const(_)) { e } else { self.constant(self[e].ty, &v)? };
                self.folded.value.insert(out, v);
                Some(out)
            } else {
                None
            }
        } else {
            None
        };
        let out = if let Some(out) = out {
            out
        } else if kids == to {
            e
        } else if let Op::Iterate { lo, k } = self[e].op {
            if !self.folded.loops.contains_key(&lo) {
                let l = self.loop_at(lo).clone();
                let init = self.fold(&l.init)?;
                let count = self.fold_visit(l.count)?;
                let made = self.iterate(count, &init, |a, state, i| {
                    let body = a.rebase_loop_body(&l, state, i, Rewrite::Fold)?;
                    let mapped = a.fold(&body)?;
                    Ok((mapped[..l.next.len()].to_vec(), mapped.get(l.next.len()).copied()))
                })?;
                self.folded.loops.insert(lo, made);
            }
            self.folded.loops[&lo][k]
        } else {
            self.rebuild_with(e, &to, Rewrite::Fold)?
        };
        self.folded.expr.insert(e, out);
        if out != e {
            self.folded.expr.insert(out, out);
        }
        Ok(out)
    }
}
