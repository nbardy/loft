use super::*;
use std::collections::BTreeMap;

type Axes = [u8; 2];

/// Axis dependencies for straight-line scalar and vec2 arithmetic. Unknown
/// operations can depend on both coordinates, so their results stay in the strip.
fn axes(
    em: &Emitter<'_, '_>, e: &Expr, locals: &BTreeMap<&str, Axes>, hoisted: &mut Option<u32>,
) -> Axes {
    let join = |a: Axes, b: Axes| [a[0] | b[0], a[1] | b[1]];
    match &e.kind {
        ExprKind::Num { .. } | ExprKind::Bool(_) => [0; 2],
        ExprKind::Name(n) => locals.get(n.as_str()).copied().unwrap_or([3; 2]),
        ExprKind::Member(value, field) => {
            let a = axes(em, value, locals, hoisted);
            match field.name.as_str() { "x" => [a[0]; 2], "y" => [a[1]; 2], _ => [3; 2] }
        }
        ExprKind::Binary(_, left, right) => join(axes(em, left, locals, hoisted), axes(em, right, locals, hoisted)),
        ExprKind::Unary(_, v) => axes(em, v, locals, hoisted),
        ExprKind::Ctor { args, .. } if args.len() == 2 && matches!(em.ty_of(e), Ty::VecN(2)) => {
            [axes(em, &args[0].value, locals, hoisted)[0], axes(em, &args[1].value, locals, hoisted)[0]]
        }
        ExprKind::Call { callee, args } => {
            let all = args.iter().fold([0; 2], |a, arg| join(a, axes(em, &arg.value, locals, hoisted)));
            let name = callee.name.as_str();
            match (name, args.as_slice()) {
                ("loft_vec", [_]) => all,
                ("abs" | "min" | "max" | "clamp" | "smoothstep", _) if !em.p.fns.contains_key(name) => all,
                ("sin" | "cos" | "exp" | "log" | "sqrt", [_]) if !em.p.fns.contains_key(name) => {
                    if all[0] == 1 && matches!(em.ty_of(e), Ty::F32) && hoisted.is_none() {
                        *hoisted = Some(e.id);
                    }
                    all
                }
                _ => [3; 2],
            }
        }
        _ => [3; 2],
    }
}

fn expression(e: &Expr, id: u32) -> Option<&Expr> {
    if e.id == id { return Some(e); }
    match &e.kind {
        ExprKind::Member(e, _) | ExprKind::Unary(_, e) => expression(e, id),
        ExprKind::Binary(_, a, b) => expression(a, id).or_else(|| expression(b, id)),
        ExprKind::Call { args, .. } | ExprKind::Ctor { args, .. } =>
            args.iter().find_map(|arg| expression(&arg.value, id)),
        _ => None,
    }
}

pub(super) fn emit(em: &mut Emitter<'_, '_>, texel: &FnDecl) -> Option<u32> {
    if texel.raw.is_some() { return None; }
    let coord = texel.params.first()?.name.name.as_str();
    let mut locals = BTreeMap::from([(coord, [1, 2])]);
    let mut prefix = Vec::new();
    for statement in &texel.body.stmts {
        let (bind, value) = match statement {
            Stmt::Let { mutable: false, name, value, .. } => (Some(name.name.as_str()), value),
            Stmt::Return(Some(value), _) | Stmt::Expr(value) => (None, value),
            _ => break,
        };
        let mut hoisted = None;
        let dependence = axes(em, value, &locals, &mut hoisted);
        if let Some((id, value)) = hoisted.and_then(|id| expression(value, id).map(|value| (id, value))) {
            let coord_wgsl = wgsl_name(coord);
            let _ = writeln!(em.out, "fn loft_bake_column({coord_wgsl}: vec2f) -> f32 {{");
            em.maps.clear();
            em.locals = vec![coord.to_owned()];
            for prior in prefix { em.stmt(prior, 1); }
            let out = em.expr(value);
            let _ = writeln!(em.out, "  return {out};\n}}");
            return Some(id);
        }
        let Some(bind) = bind else { break };
        if dependence == [3, 3] { break; }
        locals.insert(bind, dependence);
        prefix.push(statement);
    }
    None
}
