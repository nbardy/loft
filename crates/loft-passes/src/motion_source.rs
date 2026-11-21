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
        Stmt::Emit { span, .. } | Stmt::ImportTable { span, .. } => {
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

#[derive(Clone, Copy)]
pub(crate) enum Strategy {
    Symmetric,
    Colored(u32),
}

pub(crate) enum StateMemory {
    Private,
    Workgroup,
}

pub(crate) struct SolverConfig {
    pub strategy: Strategy,
    pub memory: StateMemory,
    pub substeps: u32,
    pub iterations: u32,
}

pub(crate) struct Distance<'a> {
    pub field: &'a str,
    pub endpoints: [&'a Ident; 2],
    pub start: u32,
    pub end: u32,
    pub args: [&'a Expr; 4],
}

pub(crate) enum Step<'a> {
    Statement(&'a Stmt),
    Distance(Distance<'a>),
}

pub(crate) fn scheduled<'a>(
    p: &Prog<'a>,
    types: &mut Vec<Ty>,
    decl: &'a SolverDecl,
) -> Res<(&'a StateDecl, SolverConfig, Vec<Step<'a>>)> {
    let mut ck = Ck::new(p, types);
    ck.scopes.push(HashMap::new());
    let table = p.states[decl.table[0].name.as_str()];
    if table.home.as_ref().is_some_and(|home| home.name == "cpu") {
        return Err(solver_error(
            table.name.span,
            "a GPU solver cannot write CPU state",
        ));
    }
    let writes = |name: &str| {
        decl.writes
            .as_ref()
            .is_some_and(|(_, fields)| fields.is_empty() || fields.iter().any(|f| f.name == name))
    };
    for field in &table.fields {
        let ty = ck.resolve(&field.ty, true)?;
        ck.bind(
            &field.name.name,
            Local {
                ty,
                mutable: writes(&field.name.name),
                map: None,
                span: field.name.span,
            },
        );
    }
    ck.bind(
        "dt",
        Local {
            ty: Ty::F32,
            mutable: false,
            map: None,
            span: decl.name.span,
        },
    );
    let mut config = SolverConfig {
        strategy: Strategy::Symmetric,
        memory: StateMemory::Private,
        substeps: 1,
        iterations: 1,
    };
    for (key, value) in &decl.config {
        match key.name.as_str() {
            "strategy" => {
                config.strategy = match &value.kind {
                    ExprKind::Name(_) => Strategy::Symmetric,
                    ExprKind::Call { args, .. } => Strategy::Colored(small_count(&args[0].value)?),
                    _ => unreachable!("checked solver strategy"),
                }
            }
            "substeps" => config.substeps = small_count(value)?,
            "iters" => config.iterations = small_count(value)?,
            "shared_mem" => {
                config.memory = match value.kind {
                    ExprKind::Bool(true) => StateMemory::Workgroup,
                    ExprKind::Bool(false) => StateMemory::Private,
                    _ => unreachable!("checked solver storage"),
                }
            }
            _ => unreachable!("checked solver config"),
        }
    }
    let mut steps = Vec::new();
    for step in &decl.steps {
        steps.push(match step {
            SolverStep::Statement(stmt) => {
                ck.stmt(stmt)?;
                Step::Statement(stmt)
            }
            SolverStep::Constraint {
                a,
                b,
                lo,
                hi,
                block,
                ..
            } => Step::Distance(distance(&mut ck, [a, b], [lo, hi], block)?),
        });
    }
    Ok((table, config, steps))
}

fn solver_error(span: Span, message: &str) -> Diag {
    Diag::new(
        Code::E405,
        span,
        message,
        "use GPU state and a bounded distance constraint",
    )
}

fn small_count(e: &Expr) -> Res<u32> {
    match e.kind {
        ExprKind::Num { value, .. } if (1.0..=64.0).contains(&value) => Ok(value as u32),
        _ => Err(solver_error(
            e.span,
            "solver configuration exceeds 64 steps",
        )),
    }
}

fn distance<'a>(
    ck: &mut Ck<'_, '_>,
    endpoints: [&'a Ident; 2],
    [lo, hi]: [&Expr; 2],
    block: &'a Block,
) -> Res<Distance<'a>> {
    let integer = |e: &Expr| match e.kind {
        ExprKind::Num {
            value,
            float: false,
            ..
        } if (0.0..=4096.0).contains(&value) => Ok(value as u32),
        _ => Err(solver_error(
            e.span,
            "the edge range needs bounded integer literals",
        )),
    };
    let (start, end) = (integer(lo)?, integer(hi)?);
    let unsupported = || {
        solver_error(
            lo.span,
            "constrain expects solve_dist_edge(a, b, im_a, im_b, rest, alpha)",
        )
    };
    let [
        Stmt::Expr(Expr {
            kind: ExprKind::Call { callee, args },
            ..
        }),
    ] = block.stmts.as_slice()
    else {
        return Err(unsupported());
    };
    let [pa, pb, ma, mb, rest, alpha] = args.as_slice() else {
        return Err(unsupported());
    };
    if callee.name != "solve_dist_edge" || args.iter().any(|arg| arg.name.is_some()) {
        return Err(unsupported());
    }
    let field = |p: &'a Expr, binder: &Ident| -> Res<&'a str> {
        if let ExprKind::Index(base, i) = &p.kind
            && let (ExprKind::Name(f), ExprKind::Name(n)) = (&base.kind, &i.kind)
            && *n == binder.name
        {
            return Ok(f);
        }
        Err(solver_error(
            p.span,
            "edge positions must index the authored endpoint binders",
        ))
    };
    let f = field(&pa.value, endpoints[0])?;
    let g = field(&pb.value, endpoints[1])?;
    let Some(Local {
        ty: Ty::Array(ty, count),
        mutable,
        ..
    }) = ck.lookup(f)
    else {
        return Err(solver_error(
            pa.value.span,
            "edge positions require an array field",
        ));
    };
    if !mutable {
        return Err(Diag::new(
            Code::E501,
            pa.value.span,
            "edge writes outside the solver's writes set",
            "declare the point array in writes",
        ));
    }
    if f != g || !matches!(ty.as_ref(), Ty::Geo(Geo::Pt, _)) || start >= end || end >= *count {
        return Err(Diag::new(
            Code::E503,
            lo.span,
            "adjacent edges must fit in one point array",
            "use a nonempty half-open range of left endpoints with a successor in the array",
        ));
    }
    let args = [ma, mb, rest, alpha].map(|arg| &arg.value);
    ck.scopes.push(HashMap::new());
    for binder in endpoints {
        ck.bind(
            &binder.name,
            Local {
                ty: Ty::U32,
                mutable: false,
                map: None,
                span: binder.span,
            },
        );
    }
    for arg in args {
        let ty = ck.expr(arg)?;
        ck.assignable(&ty, &Ty::F32, arg.span)?;
        if ty == Ty::AbstractInt {
            ck.set(arg, Ty::F32);
        }
    }
    ck.scopes.pop();
    Ok(Distance {
        field: f,
        endpoints,
        start,
        end,
        args,
    })
}
