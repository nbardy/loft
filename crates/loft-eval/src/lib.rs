//! Stage-0 evaluation, CPU queries, and geometry analysis.
pub mod ad;
pub mod bvh;
pub mod eval;
pub mod guard;
mod import_table;
mod kernel;
pub mod lip;
pub mod quant;
pub mod refine;

use eval::{Eval, RowVal};
use loft_check::{FINITE_DIFF_STEP, Prog, check, finite_diff_parameter};
use loft_ir::{Code, Diag, Res, Span};
use loft_parse::ast::{Expr, Ty};
pub use kernel::kernel_argument;

/// Stage-0 value of a schedule argument.
pub fn stage0(p: &Prog, types: &mut Vec<Ty>, e: &Expr, overrides: &[(String, f64)]) -> Res<RowVal> {
    check::check_stage0_expr(p, types, e)?;
    let mut ev: Eval<f32> = Eval::new(p, types, vec![], overrides);
    eval::concrete(&ev.expr(e)?, e.span)
}

pub fn as_f32(v: RowVal, span: Span) -> Res<f32> {
    match v {
        RowVal::F(x) => Ok(x),
        RowVal::U(u) => Ok(u as f32),
        RowVal::I(i) => Ok(i as f32),
        other @ RowVal::V(_) | other @ RowVal::B(_) | other @ RowVal::X(_) => Err(Diag::new(Code::E003, span, format!("expected a number, found {other:?}"), "pass an f32")),
    }
}
/// Words of one row in the GPU table: the author's fields in order (Xform: rotation column-major, then t).
pub fn pack(row: &[RowVal], out: &mut Vec<f32>) {
    for v in row {
        match v {
            RowVal::F(x) => out.push(*x),
            RowVal::V(c) => out.extend(c),
            RowVal::U(u) => out.push(f32::from_bits(*u)),
            RowVal::I(i) => out.push(f32::from_bits(*i as u32)),
            RowVal::B(b) => out.push(if *b { 1.0 } else { 0.0 }),
            RowVal::X(x) => {
                for j in 0..3 {
                    for i in 0..3 {
                        out.push(x.r[i][j]);
                    }
                }
                out.extend(x.t);
            }
        }
    }
}
