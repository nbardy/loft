//! Stage-0 initialization for authored state solver columns.
use crate::{Prog, pack, stage0};
use loft_ir::Res;
use loft_parse::ast::{Expr, Ty};

pub fn initial_words(p: &Prog<'_>, types: &mut Vec<Ty>, e: &Expr, overrides: &[(String, f64)]) -> Res<Vec<f32>> {
    let value = stage0(p, types, e, overrides)?;
    let mut initial = Vec::new();
    pack(&[value], &mut initial);
    Ok(initial)
}
