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
    for spec in ck.p.solvers.values() {
        solver(ck.p, spec)?;
    }
    for plan in ck.p.plans.values() {
        crate::plan::check_source(plan)?;
    }
    if let Some(schedule) = ck.p.schedule {
        for pass in schedule
            .passes
            .iter()
            .filter(|p| p.verb.name == "run_solver")
        {
            let [arg] = pass.args.as_slice() else {
                return Err(Diag::new(
                    Code::E003,
                    pass.verb.span,
                    "run_solver takes one declaration",
                    "pass the name of a declared solver",
                ));
            };
            if !matches!(&arg.value.kind, ExprKind::Name(name) if ck.p.solvers.contains_key(name.as_str()))
            {
                return Err(Diag::new(
                    Code::E011,
                    arg.value.span,
                    "unknown solver",
                    "use a declared solver",
                ));
            }
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

fn solver(p: &Prog<'_>, solver: &SolverDecl) -> Res<()> {
    let [table] = solver.table.as_slice() else {
        return Err(Diag::new(
            Code::E405,
            solver.name.span,
            "solver table paths are not lowered",
            "use a top-level state table",
        ));
    };
    let state = p.states.get(table.name.as_str()).ok_or_else(|| {
        Diag::new(
            Code::E011,
            table.span,
            "unknown state table",
            "declare the solver's state table",
        )
    })?;
    let mut writes = HashSet::new();
    if let Some((owner, fields)) = &solver.writes {
        if owner.name != table.name {
            return Err(Diag::new(
                Code::E501,
                owner.span,
                "writes names a different state table",
                "declare writes on the state named after over",
            ));
        }
        if fields.is_empty() {
            writes.extend(state.fields.iter().map(|f| f.name.name.as_str()));
        }
        for field in fields {
            if !state.fields.iter().any(|f| f.name.name == field.name) {
                return Err(Diag::new(
                    Code::E501,
                    field.span,
                    "unknown state field",
                    "use a field of the target state",
                ));
            }
            unique(&mut writes, field)?;
        }
    }
    let mut config = HashSet::new();
    for (key, value) in &solver.config {
        unique(&mut config, key)?;
        let positive_int = |e: &Expr| {
            matches!(e.kind, ExprKind::Num { value, float: false, .. }
            if value > 0. && value <= f64::from(u32::MAX))
        };
        let valid = match key.name.as_str() {
            "strategy" => match &value.kind {
                ExprKind::Name(name) => name == "sym_gs",
                ExprKind::Call { callee, args } => {
                    callee.name == "colored_gs"
                        && matches!(args.as_slice(), [arg] if arg.name.is_none() && positive_int(&arg.value))
                }
                _ => false,
            },
            "substeps" | "iters" => positive_int(value),
            "shared_mem" => matches!(value.kind, ExprKind::Bool(_)),
            _ => false,
        };
        if !valid {
            return Err(Diag::new(
                Code::E003,
                value.span,
                format!("invalid solver config field {}", key.name),
                "use sym_gs or colored_gs(n), positive substeps and iters, and a boolean shared_mem",
            ));
        }
    }
    let mut locals = HashSet::new();
    for step in &solver.steps {
        match step {
            SolverStep::Statement(stmt) => solver_statement(stmt, &writes, &mut locals)?,
            SolverStep::Constraint { a, b, block, .. } => {
                if a.name == b.name {
                    return Err(Diag::new(
                        Code::E503,
                        b.span,
                        "constraint endpoints have the same binder",
                        "use a separate name for each endpoint",
                    ));
                }
                solver_block(block, &writes, &locals)?;
            }
        }
    }
    Ok(())
}

fn solver_block<'a>(
    block: &'a Block,
    writes: &HashSet<&str>,
    locals: &HashSet<&'a str>,
) -> Res<()> {
    let mut locals = locals.clone();
    for stmt in &block.stmts {
        solver_statement(stmt, writes, &mut locals)?;
    }
    Ok(())
}

fn solver_statement<'a>(
    stmt: &'a Stmt,
    writes: &HashSet<&str>,
    locals: &mut HashSet<&'a str>,
) -> Res<()> {
    match stmt {
        Stmt::Let { name, mutable, .. } => {
            locals.remove(name.name.as_str());
            if *mutable {
                locals.insert(&name.name);
            }
        }
        Stmt::Assign { target, .. } => {
            let mut base = target;
            while let ExprKind::Index(parent, _) | ExprKind::Member(parent, _) = &base.kind {
                base = parent;
            }
            if !matches!(&base.kind, ExprKind::Name(name) if writes.contains(name.as_str()) || locals.contains(name.as_str()))
            {
                return Err(Diag::new(
                    Code::E501,
                    target.span,
                    "assignment outside the solver's writes set",
                    "declare the destination field in writes, or use a mutable local",
                ));
            }
        }
        Stmt::If {
            then, otherwise, ..
        } => {
            solver_block(then, writes, locals)?;
            if let Some(otherwise) = otherwise {
                solver_block(otherwise, writes, locals)?;
            }
        }
        Stmt::For { body, .. } | Stmt::Each { body, .. } => solver_block(body, writes, locals)?,
        Stmt::Emit { span, .. } => {
            return Err(Diag::new(
                Code::E403,
                *span,
                "emit constructs rows at build time",
                "use emit_event to report a solver event",
            ));
        }
        Stmt::Return(..) | Stmt::Expr(_) | Stmt::Map { .. } => {}
    }
    Ok(())
}
