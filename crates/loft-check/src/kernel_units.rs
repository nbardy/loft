//! Retain source units while checking a typed kernel. Storage and SSA use f32.
use super::*;

const SCALAR: Dimension = [0.; 4];
const ANGLE: Dimension = [0., 0., 0., 1.];
const LENGTH: Dimension = [1., 0., 0., 0.];

impl Ck<'_, '_> {
    pub(super) fn declared_unit(&self, t: &TyExpr) -> Dimension {
        if let TyExpr::Name(n) = t
            && let Some((_, alias)) = self.p.aliases.get(n.name.as_str()) {
            return self.declared_unit(alias);
        }
        match t {
            TyExpr::Geo(Geo::Pt | Geo::Vec, _) => LENGTH,
            _ => self.p.dimensions.get(&t.span()).copied().unwrap_or(SCALAR),
        }
    }

    pub(super) fn expect_unit(&self, e: &Expr, expected: Dimension) -> Res<()> {
        let got = self.expr_units[&e.id];
        if got == expected || matches!(e.kind, ExprKind::Num { value: 0., suffix: NumSuffix::None, .. }) {
            return Ok(());
        }
        Err(Diag::new(Code::E110, e.span, format!("expected dimensions {expected:?}, found {got:?}"),
            "use compatible length, time, mass and angle units"))
    }

    pub(super) fn kernel_unit(&self, e: &Expr) -> Res<Dimension> {
        let dim = |x: &Expr| self.expr_units[&x.id];
        let common = |a: &Expr, b: &Expr| -> Res<Dimension> {
            let want = if matches!(a.kind, ExprKind::Num { value: 0., suffix: NumSuffix::None, .. }) { dim(b) } else { dim(a) };
            self.expect_unit(b, want)?;
            Ok(want)
        };
        use ExprKind::*;
        Ok(match &e.kind {
            Num { .. } => self.p.dimensions.get(&e.span).copied().unwrap_or(SCALAR),
            Bool(_) => SCALAR,
            String(_) => return Err(Diag::new(Code::E403, e.span, "string in a kernel", "keep asset names in Stage 0")),
            Name(n) => {
                if let Some(local) = self.lookup(n) { self.local_units.get(&local.span).copied().unwrap_or(SCALAR) }
                else if let Some(c) = self.p.consts.get(n.as_str()) {
                    c.ty.as_ref().map(|t| self.declared_unit(t)).or_else(|| self.p.dimensions.get(&c.value.span).copied()).unwrap_or(SCALAR)
                } else { SCALAR }
            }
            Member(b, field) => {
                let fields = match self.types.get(b.id as usize) {
                    Some(Ty::Struct(k)) => Some(self.p.structs[k.as_str()].1),
                    Some(Ty::Row(k)) => Some(self.p.kinds[k.as_str()].row.as_slice()),
                    _ => None,
                };
                fields.and_then(|fs| fs.iter().find(|f| f.name.name == field.name))
                    .map_or_else(|| self.expr_units.get(&b.id).copied().unwrap_or(SCALAR), |f| self.declared_unit(&f.ty))
            }
            Unary(_, x) | Index(x, _) => dim(x),
            Binary(op, a, b) => match op {
                BinOp::Mul => std::array::from_fn(|i| dim(a)[i] + dim(b)[i]),
                BinOp::Div => std::array::from_fn(|i| dim(a)[i] - dim(b)[i]),
                BinOp::Add | BinOp::Sub | BinOp::Rem => common(a, b)?,
                BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge | BinOp::Eq | BinOp::Ne => { common(a,b)?; SCALAR }
                _ => SCALAR,
            },
            If { then, otherwise, cases, .. } => {
                let mut d = common(&then.value, &otherwise.value)?;
                if let Some(cases) = cases {
                    for (_, value) in &cases.rest {
                        self.expect_unit(value, d)?;
                        if d == SCALAR { d = dim(value); }
                    }
                }
                d
            }
            Ctor { ty: TyExpr::Geo(g, _), .. } => if matches!(g, Geo::Pt | Geo::Vec) { LENGTH } else { SCALAR },
            Ctor { args, .. } => {
                let d = args.first().map_or(SCALAR, |a| dim(&a.value));
                for a in args.iter().skip(1) { self.expect_unit(&a.value, d)?; }
                d
            }
            Call { callee, args } => {
                if let Some(f) = self.p.fns.get(callee.name.as_str()) {
                    for (p, a) in f.params.iter().zip(args) {
                        if let Some(t) = &p.ty { self.expect_unit(&a.value, self.declared_unit(t))?; }
                    }
                    return Ok(f.ret.as_ref().map_or(SCALAR, |t| self.declared_unit(t)));
                }
                let first = || dim(&args[0].value);
                match callee.name.as_str() {
                    "sin" | "cos" | "tan" => { self.expect_unit(&args[0].value, ANGLE)?; SCALAR }
                    "atan2" => { common(&args[0].value, &args[1].value)?; ANGLE }
                    "acos" | "asin" | "atan" => { self.expect_unit(&args[0].value, SCALAR)?; ANGLE }
                    "exp" | "log" | "pow" => { for a in args { self.expect_unit(&a.value, SCALAR)?; } SCALAR }
                    "abs" | "length" | "coords" | "subgroup_add" | "floor" | "fract" => first(),
                    "sqrt" => first().map(|x| x * 0.5),
                    "dot" | "cross" => std::array::from_fn(|i| first()[i] + dim(&args[1].value)[i]),
                    "min" | "max" | "dist" | "distance" | "atomic_add" => common(&args[0].value, &args[1].value)?,
                    "clamp" => { let d = common(&args[0].value,&args[1].value)?; self.expect_unit(&args[2].value, d)?; d }
                    "mix" => { self.expect_unit(&args[2].value, SCALAR)?; common(&args[0].value,&args[1].value)? }
                    "select" => common(&args[0].value,&args[1].value)?,
                    _ => SCALAR,
                }
            }
            Unproved { .. } | FiniteDiff { .. } => unreachable!("only inside geometry"),
        })
    }
}
