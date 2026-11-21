use crate::{Eval, Prog, RowVal};
use loft_ir::{Code, Diag, Res};
use loft_parse::ast::*;

pub fn kernel_argument<'a>(
    p: &Prog<'a>, types: &mut Vec<Ty>, kernel: &'a KernelDecl, value: &'a Expr,
    ty: &TyExpr, overrides: &[(String, f64)],
) -> Res<String> {
    loft_check::check::check_kernel_argument(p, types, kernel, value, ty)?;
    let mut ev = Eval::<f32>::new(p, types, vec![], overrides);
    let v = crate::eval::concrete(&ev.expr(value)?, value.span)?;
    let float = |f: f32| format!("{f:?}");
    Ok(match v {
        RowVal::F(n) if n.is_finite() => float(n),
        RowVal::I(n) => format!("{n}i"),
        RowVal::U(n) => format!("{n}u"),
        RowVal::B(n) => n.to_string(),
        RowVal::V(v) if (2..=4).contains(&v.len()) && v.iter().all(|n| n.is_finite()) =>
            format!("vec{}f({})", v.len(), v.into_iter().map(float).collect::<Vec<_>>().join(", ")),
        _ => return Err(Diag::new(Code::E003, value.span, "kernel argument cannot be lowered as a constant", "supply a finite scalar or vector")),
    })
}
