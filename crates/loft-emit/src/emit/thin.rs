//! Clip an original proxy to the screen projection of a stock exact segment.
use super::{Emitter, render_template, wgsl_name};
use loft_ir::Sources;
use loft_parse::ast::{Expr, ExprKind, KindDecl, QueryKind, Stmt};

// These closed forms (including cone_cap and cone_side) intersect the hull
// of the two endpoint balls. A fork with different mathematics gets the cube.
const ANALYTIC: &str = "267de6837aef99ac265696d676a7c6f4d5117770823e71798a0c3940f2506255";

fn row_value(e: &Expr, row: &str) -> bool {
    match &e.kind {
        ExprKind::Member(base, _) => matches!(&base.kind, ExprKind::Name(n) if n == row),
        ExprKind::Num { .. } => true,
        ExprKind::Ctor { args, .. } => args.iter().all(|arg| row_value(&arg.value, row)),
        _ => false,
    }
}

pub(super) fn position(em: &mut Emitter<'_, '_>, k: &KindDecl, sources: &Sources) -> Option<String> {
    let query = k.query(QueryKind::Visible)?;
    let [row, ro, rd] = query.params.as_slice() else { return None };
    let [Stmt::Return(Some(result), _)] = query.body.stmts.as_slice() else { return None };
    let ExprKind::Call { callee, args } = &result.kind else { return None };
    let solver = em.p.fns.get(callee.name.as_str())?;
    let span = solver.name.span;
    let source = &sources.files.get(usize::from(span.file))?.1;
    let name = source.get(span.lo as usize..span.hi as usize)?;
    let (a, b, ra, rb) = match (name, args.as_slice()) {
        ("ray_capsule", [_, _, a, b, r]) => (a, b, r, r),
        ("ray_round_cone", [_, _, a, b, ra, rb]) => (a, b, ra, rb),
        _ => return None,
    };
    if blake3::hash(source.as_bytes()).to_hex().as_str() != ANALYTIC
        || !matches!(&args[0].value.kind, ExprKind::Name(n) if n == &ro.name.name)
        || !matches!(&args[1].value.kind, ExprKind::Name(n) if n == &rd.name.name)
        || ![a, b, ra, rb].into_iter().all(|arg| row_value(&arg.value, &row.name.name))
    {
        return None;
    }
    let [a, b, ra, rb] = [a, b, ra, rb].map(|arg| em.expr(&arg.value));
    Some(render_template(include_str!("../../../../stdlib/core/thin_clip.wgsl"), &[
        ("ROW", &wgsl_name(&row.name.name)), ("A", &a), ("B", &b), ("RA", &ra), ("RB", &rb),
    ]))
}
