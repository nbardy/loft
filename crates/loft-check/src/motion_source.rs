//! Declaration checks before motion is lowered to packed tables and GPU passes.
use super::*;
use std::collections::HashSet;

pub(super) fn check(ck: &mut Ck<'_, '_>) -> Res<()> {
    for (name, items) in ck.p.bodies.values() {
        ck.comptime = true;
        ck.scopes = vec![HashMap::new()];
        if items
            .iter()
            .filter(|item| matches!(item, BodyItem::Trunk(..)))
            .count()
            != 1
        {
            return Err(Diag::new(
                Code::E003,
                name.span,
                "a body needs exactly one trunk",
                "declare one trunk at the top of the body",
            ));
        }
        body(ck, items)?;
    }
    for state in ck.p.states.values() {
        if state.checkpoint == Some(0) || state.fields.is_empty() {
            return Err(Diag::new(
                Code::E003,
                state.name.span,
                "invalid state layout",
                "declare fields and use a positive checkpoint interval",
            ));
        }
        if let Some(home) = &state.home
            && !matches!(home.name.as_str(), "cpu" | "gpu")
        {
            return Err(Diag::new(
                Code::E003,
                home.span,
                "unknown state home",
                "use cpu or gpu",
            ));
        }
    }
    Ok(())
}

fn body(ck: &mut Ck<'_, '_>, items: &[BodyItem]) -> Res<()> {
    let mut names = HashSet::new();
    for item in items {
        match item {
            BodyItem::Trunk(name, fields) | BodyItem::Segment(name, fields) => {
                unique(&mut names, name)?;
                body_fields(ck, fields)?;
            }
            BodyItem::Chain(name, index, role, fields) => {
                if let Some(index) = index {
                    integer(ck, index)?;
                } else {
                    unique(&mut names, name)?;
                }
                if !matches!(
                    role.name.as_str(),
                    "support" | "manip" | "gaze" | "secondary"
                ) {
                    return Err(Diag::new(
                        Code::E003,
                        role.span,
                        "unknown chain role",
                        "use support, manip, gaze or secondary",
                    ));
                }
                body_fields(ck, fields)?;
            }
            BodyItem::For(var, lo, hi, items) => {
                integer(ck, lo)?;
                integer(ck, hi)?;
                ck.scopes.push(HashMap::new());
                ck.bind(
                    &var.name,
                    Local {
                        ty: Ty::I32,
                        mutable: false,
                        map: None,
                        span: var.span,
                    },
                );
                body(ck, items)?;
                ck.scopes.pop();
            }
            BodyItem::Let(name, value) => {
                let ty = ck.expr(value)?;
                ck.bind(
                    &name.name,
                    Local {
                        ty,
                        mutable: false,
                        map: None,
                        span: name.span,
                    },
                );
            }
            BodyItem::Waive(..) => {}
        }
    }
    Ok(())
}

fn integer(ck: &mut Ck<'_, '_>, e: &Expr) -> Res<()> {
    match ck.expr(e)? {
        Ty::AbstractInt | Ty::I32 | Ty::U32 => Ok(()),
        _ => Err(Diag::new(
            Code::E003,
            e.span,
            "expected an integer",
            "use an integer index or loop bound",
        )),
    }
}

fn unique<'a>(seen: &mut HashSet<&'a str>, key: &'a Ident) -> Res<()> {
    if !seen.insert(key.name.as_str()) {
        return Err(Diag::new(
            Code::E011,
            key.span,
            "duplicate declaration field",
            "give each field a distinct name",
        ));
    }
    Ok(())
}

fn body_fields(ck: &mut Ck<'_, '_>, fields: &BodyFields) -> Res<()> {
    let mut seen = HashSet::new();
    for (key, value) in fields {
        unique(&mut seen, key)?;
        match value {
            // These two fields refer to authored segment and IK symbols.
            BodyValue::Value(Expr {
                kind: ExprKind::Name(_),
                ..
            }) if matches!(key.name.as_str(), "parent" | "ik") => {}
            BodyValue::Lengths(lengths) => {
                if lengths.is_empty() {
                    return Err(Diag::new(
                        Code::E003,
                        key.span,
                        "chain has no segment lengths",
                        "specify at least one length",
                    ));
                }
                for length in lengths {
                    let ty = ck.expr(length)?;
                    ck.assignable(&ty, &Ty::F32, length.span)?;
                }
            }
            BodyValue::Value(e) => {
                ck.expr(e)?;
            }
            BodyValue::Range(lo, hi) => {
                let a = ck.expr(lo)?;
                let b = ck.expr(hi)?;
                ck.assignable(&a, &b, hi.span)?;
            }
            BodyValue::Record(record) => body_fields(ck, record)?,
        }
    }
    Ok(())
}
