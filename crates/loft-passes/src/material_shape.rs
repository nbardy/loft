//! A material's layout depends on the shape of its staged lobe expression.
use loft_parse::ast::{Arg, BinOp, Expr, ExprKind};
use std::fmt::Write;

#[derive(PartialEq, Eq)]
pub(super) enum Op {
    Lobe(usize),
    Layer,
    Mix(usize),
}

#[derive(Default)]
pub(super) struct Layout<'a> {
    pub lobes: Vec<(&'a str, &'a [Arg])>,
    pub weights: Vec<&'a Expr>,
    ops: Vec<Op>,
}

impl<'a> Layout<'a> {
    pub fn add(&mut self, e: &'a Expr) -> bool {
        match &e.kind {
            ExprKind::Binary(BinOp::Add, a, b) => {
                if !self.add(a) || !self.add(b) { return false; }
                self.ops.push(Op::Layer);
            }
            ExprKind::Call { callee, args } if callee.name == "mix_bsdf" => {
                let [a, b, weight] = args.as_slice() else { return false; };
                if !self.add(&a.value) || !self.add(&b.value) { return false; }
                self.ops.push(Op::Mix(self.weights.len()));
                self.weights.push(&weight.value);
            }
            ExprKind::Call { callee, args } => {
                self.ops.push(Op::Lobe(self.lobes.len()));
                self.lobes.push((&callee.name, args));
            }
            ExprKind::Unproved { .. } | ExprKind::FiniteDiff { .. } | ExprKind::If { .. } | ExprKind::Num { .. }
            | ExprKind::Bool(_) | ExprKind::Name(_) | ExprKind::Member(..) | ExprKind::Index(..) | ExprKind::Ctor { .. }
            | ExprKind::Binary(..) | ExprKind::Unary(..) => return false,
        }
        true
    }

    pub fn same_as(&self, other: &Self) -> bool {
        self.lobes.iter().map(|l| l.0).eq(other.lobes.iter().map(|l| l.0)) &&
            (self.weights.is_empty() && other.weights.is_empty() || self.ops == other.ops)
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
