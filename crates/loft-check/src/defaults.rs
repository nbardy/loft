//! Elaborate an omitted render schedule from the authored realtime recipe.
use loft_ir::{Code, Diag, Res, Span};
use loft_parse::ast::*;

pub const ENV_DEFAULTS: [&str; 3] = ["DEFAULT_SUN_COLOR", "DEFAULT_ZENITH", "DEFAULT_HORIZON"];

/// Inline authored schedule calls and fill in a default recipe when needed.
pub fn schedule(modules: &mut Vec<Module>, next: &mut u32, recipe: Option<ScheduleDecl>, library: &[ScheduleDecl]) -> Res<()> {
    compose(modules, next, recipe, library)?;
    let worlds: std::collections::BTreeMap<_, _> = modules.iter().flat_map(|m| &m.items)
        .filter_map(|item| match item {
            Item::World(name, members) => Some((name.name.clone(), members.clone())),
            _ => None,
        }).collect();
    let data_kinds = modules.iter().flat_map(|m| &m.items).filter_map(|item| match item {
        Item::Kind(kind) if kind.queries.is_empty() && kind.name.name != "Mesh" => Some(kind.name.name.clone()),
        _ => None,
    }).collect::<std::collections::BTreeSet<_>>();
    if let Some(s) = modules.iter_mut().flat_map(|m| &mut m.items).find_map(|i| {
        if let Item::Schedule(s) = i { Some(s) } else { None }
    }) {
        // Expand only at kind-list boundaries. A world passed to a lighting function stays a World value.
        let expand_names = |names: &mut Vec<Ident>, geometry: bool| {
            *names = std::mem::take(names).into_iter().flat_map(|n| {
                match worlds.get(&n.name) {
                    Some(members) if geometry && members.iter().any(|k| !data_kinds.contains(&k.name)) => {
                        members.iter().filter(|k| !data_kinds.contains(&k.name)).cloned().collect()
                    }
                    Some(members) => members.clone(),
                    None => vec![n],
                }
            }).collect();
        };
        let mut expand_args = |args: &mut Vec<Arg>| {
            let mut expanded = Vec::new();
            for arg in std::mem::take(args) {
                if arg.name.is_none() && let ExprKind::Name(n) = &arg.value.kind
                    && let Some(members) = worlds.get(n)
                {
                    let filter_data = members.iter().any(|k| !data_kinds.contains(&k.name));
                    for member in members.iter().filter(|k| !filter_data || !data_kinds.contains(&k.name)) {
                        expanded.push(Arg { name: None, value: Expr {
                            id: *next, span: arg.value.span, kind: ExprKind::Name(member.name.clone()),
                        } });
                        *next += 1;
                    }
                } else { expanded.push(arg); }
            }
            *args = expanded;
        };
        for pass in &mut s.passes {
            if let Some(resources) = &mut pass.resources {
                // An indirect draw list contains geometry. Other frame passes may read a world's data kinds.
                let geometry = resources.writes.iter().any(|n| n.name == "IndirectBuf");
                expand_names(&mut resources.reads, geometry);
            }
            match pass.verb.name.as_str() {
                "find_shadow_map" => {
                    expand_args(&mut pass.args);
                    if let Some(BakeClock::Dirty(names)) = &mut pass.clock { expand_names(names, true); }
                }
                "render_bundle" => {
                    for (verb, args) in &mut pass.body {
                        if verb.name == "draw_indirect" { expand_args(args); }
                    }
                }
                _ => {}
            }
        }
    }
    Ok(())
}

fn compose(
    modules: &mut Vec<Module>, next: &mut u32, recipe: Option<ScheduleDecl>, library: &[ScheduleDecl],
) -> Res<()> {
    let mut cameras = modules.iter().flat_map(|m| &m.items)
        .filter_map(|i| if let Item::Camera(c) = i { Some(c) } else { None });
    let camera = cameras.next();
    if let Some(c) = cameras.next() {
        return Err(Diag::new(Code::E520, c.name.span, "multiple default cameras", "declare one camera"));
    }
    let mut authored = modules.iter().flat_map(|m| &m.items)
        .find_map(|i| if let Item::Schedule(s) = i { Some(s.clone()) } else { None });
    let passes = authored.as_mut().map_or_else(Vec::new, |s| std::mem::take(&mut s.passes));
    let mut before = expand(passes, library, 0)?;
    // A called template can already provide the present step without any stock render_bundle.
    // Appending the fallback in that case supplies a second cull and present.
    let stock_bundle = before.iter().any(|p| p.verb.name == "render_bundle");
    let template_present = before.iter().any(|p| p.call_site.is_some()
        && p.resources.as_ref().is_some_and(|r| r.writes.iter().any(|n| n.name == "Swapchain")));
    let has_camera = |p: &PassDecl| matches!(p.verb.name.as_str(), "orbit" | "use_camera");
    if let Some(camera) = camera.filter(|_| !stock_bundle && !before.iter().any(has_camera)) {
        let name = Ident { name: "use_camera".into(), span: camera.name.span };
        let args = vec![Arg { name: None, value: Expr {
            id: *next, span: camera.name.span, kind: ExprKind::Name(camera.name.name.clone()),
        } }];
        *next += 1;
        before.insert(0, PassDecl { label: name.clone(), verb: name, call_site: None, args, body: vec![], resources: None, clock: None });
    }
    let schedule = if stock_bundle || template_present {
        let mut schedule = authored.expect("expanded authored schedule");
        schedule.passes = before;
        schedule
    } else {
        let mut schedule = recipe.ok_or_else(|| Diag::new(Code::E520, Span::NONE,
            "no render schedule was selected", "import an engine or write a complete schedule"))?;
        schedule.implicit = true;
        if let Some(s) = authored {
            schedule.name = s.name;
            if s.radiance.is_some() { schedule.radiance = s.radiance; }
        }
        before.retain(|p| p.verb.name != "realtime" || !p.args.is_empty());
        schedule.passes = expand(schedule.passes, library, 0)?;
        if before.iter().any(has_camera) {
            schedule.passes.retain(|p| p.verb.name != "orbit");
        }
        before.append(&mut schedule.passes);
        schedule.passes = before;
        schedule
    };
    if let Some(slot) = modules.iter_mut().flat_map(|m| &mut m.items).find_map(|i| {
        if let Item::Schedule(s) = i { Some(s) } else { None }
    }) {
        *slot = schedule;
    } else {
        modules.push(Module { items: vec![Item::Schedule(schedule)], ..Module::default() });
    }
    Ok(())
}

/// What a schedule-function parameter is bound to at one call.
enum Bound {
    One(Expr),
    Rest(Vec<Expr>),
}

type Env = Vec<(String, Bound)>;

const MAX_DEPTH: u32 = 8;

fn expand(passes: Vec<PassDecl>, library: &[ScheduleDecl], depth: u32) -> Res<Vec<PassDecl>> {
    let mut out = Vec::with_capacity(passes.len());
    for call in passes {
        let function = library.iter().find(|f| call.resources.is_none() && f.name.name == call.verb.name);
        let Some(function) = function else {
            out.push(call);
            continue;
        };
        if depth == MAX_DEPTH {
            return Err(Diag::new(Code::E520, call.verb.span, format!("schedule functions nest more than {MAX_DEPTH} deep at `{}`", call.verb.name),
                "remove the recursive schedule call"));
        }
        if !call.body.is_empty() {
            return Err(Diag::new(Code::E520, call.verb.span, format!("`{}` is a schedule function and takes no body", call.verb.name),
                "pass its inputs as arguments"));
        }
        let env = bind(function, &call)?;
        let mut steps = Vec::with_capacity(function.passes.len());
        for step in &function.passes {
            let mut step = substitute(step, &env)?;
            step.call_site = Some(call.scheduled_from());
            // `name(...) on load` puts every step of the function at that stage, unless the step names its own.
            if step.clock.is_none() { step.clock.clone_from(&call.clock); }
            steps.push(step);
        }
        out.extend(expand(steps, library, depth + 1)?);
    }
    Ok(out)
}

fn bind(function: &ScheduleDecl, call: &PassDecl) -> Res<Env> {
    let name = &function.name.name;
    let mut positional = call.args.iter().filter(|a| a.name.is_none()).map(|a| a.value.clone()).collect::<std::collections::VecDeque<_>>();
    let mut named = call.args.iter().filter_map(|a| a.name.as_ref().map(|n| (n, &a.value))).collect::<Vec<_>>();
    let mut env = Env::new();
    for param in &function.params {
        match param {
            ScheduleParam::One { name: p, default } => {
                let value = if let Some(i) = named.iter().position(|(n, _)| n.name == p.name) {
                    named.remove(i).1.clone()
                } else if let Some(value) = positional.pop_front() {
                    value
                } else if let Some(value) = default {
                    value.clone()
                } else {
                    return Err(Diag::new(Code::E520, call.verb.span, format!("`{name}` needs `{}`", p.name),
                        format!("write `{name}(..., {}: ...)`", p.name)));
                };
                env.push((p.name.clone(), Bound::One(value)));
            }
            ScheduleParam::Rest(p) => env.push((p.name.clone(), Bound::Rest(positional.drain(..).collect()))),
        }
    }
    if let Some((n, _)) = named.first() {
        return Err(Diag::new(Code::E520, n.span, format!("`{name}` has no parameter `{}`", n.name), "remove the argument"));
    }
    if let Some(extra) = positional.front() {
        return Err(Diag::new(Code::E520, extra.span, format!("`{name}` takes {} positional arguments", function.params.len()),
            "remove the extra argument"));
    }
    Ok(env)
}

fn substitute(step: &PassDecl, env: &Env) -> Res<PassDecl> {
    let mut step = step.clone();
    if step.resources.is_none() && lookup(env, &step.verb.name).is_some() {
        let names = stores(std::slice::from_ref(&step.verb), env)?;
        let [name] = names.as_slice() else {
            return Err(Diag::new(Code::E520, step.verb.span, "expected one schedule function name",
                "pass a single name as the schedule callee"));
        };
        step.verb = name.clone();
    }
    step.args = arguments(&step.args, env)?;
    if let Some(BakeClock::Dirty(names)) = &mut step.clock { *names = stores(names, env)?; }
    for (_, args) in &mut step.body {
        *args = arguments(args, env)?;
    }
    if let Some(resources) = &mut step.resources {
        resources.reads = stores(&resources.reads, env)?;
        resources.writes = stores(&resources.writes, env)?;
        for stmt in &mut resources.body.stmts {
            statement(stmt, env)?;
        }
    }
    Ok(step)
}

fn lookup<'e>(env: &'e Env, name: &str) -> Option<&'e Bound> {
    env.iter().find(|(n, _)| n == name).map(|(_, b)| b)
}

/// Positional arguments splice a rest parameter in place; every other argument substitutes inside its value.
fn arguments(args: &[Arg], env: &Env) -> Res<Vec<Arg>> {
    let mut out = Vec::with_capacity(args.len());
    for arg in args {
        if arg.name.is_none() && let ExprKind::Name(n) = &arg.value.kind && let Some(Bound::Rest(values)) = lookup(env, n) {
            out.extend(values.iter().map(|v| Arg { name: None, value: v.clone() }));
            continue;
        }
        let mut arg = arg.clone();
        expression(&mut arg.value, env)?;
        out.push(arg);
    }
    Ok(out)
}

fn stores(names: &[Ident], env: &Env) -> Res<Vec<Ident>> {
    let mut out = Vec::with_capacity(names.len());
    for n in names {
        let values = match lookup(env, &n.name) {
            None => { out.push(n.clone()); continue; }
            Some(Bound::One(value)) => std::slice::from_ref(value),
            Some(Bound::Rest(values)) => values.as_slice(),
        };
        for value in values {
            let ExprKind::Name(store) = &value.kind else {
                return Err(Diag::new(Code::E520, value.span, format!("`{}` lists a store, but this argument is a value", n.name),
                    "pass a store or kind name"));
            };
            out.push(Ident { name: store.clone(), span: value.span });
        }
    }
    Ok(out)
}

fn statement(stmt: &mut Stmt, env: &Env) -> Res<()> {
    match stmt {
        Stmt::Expr(e) | Stmt::Let { value: e, .. } | Stmt::Return(Some(e), _) => expression(e, env),
        Stmt::Assign { target, value, .. } => { expression(target, env)?; expression(value, env) }
        Stmt::Return(None, _) => Ok(()),
        Stmt::Map { name, .. } | Stmt::Each { var: name, .. } | Stmt::For { var: name, .. } | Stmt::Emit { kind: name, .. }
        | Stmt::ImportTable { kind: name, .. } => Err(Diag::new(Code::E520, name.span,
            "a schedule function's pass body is one dispatch or draw call", "move the logic into the template or a kernel")),
        Stmt::If { cond, .. } => Err(Diag::new(Code::E520, cond.span,
            "a schedule function's pass body is one dispatch or draw call", "choose between template fragments with an `if` expression")),
    }
}

fn expression(e: &mut Expr, env: &Env) -> Res<()> {
    if let ExprKind::Name(n) = &e.kind {
        return match lookup(env, n) {
            None => Ok(()),
            Some(Bound::One(value)) => { *e = value.clone(); Ok(()) }
            Some(Bound::Rest(_)) => Err(Diag::new(Code::E520, e.span, format!("`..{n}` is a list of arguments"),
                "pass it as positional arguments or list it in reads/writes")),
        };
    }
    match &mut e.kind {
        ExprKind::Name(_) => Ok(()),
        ExprKind::If { condition, then, otherwise, cases } => {
            expression(condition, env)?;
            if let Some(cases) = cases {
                for (_, value) in &mut cases.rest {
                    expression(value, env)?;
                }
            }
            for arm in [then, otherwise] {
                for stmt in &mut arm.body.stmts { statement(stmt, env)?; }
                expression(&mut arm.value, env)?;
            }
            Ok(())
        }
        ExprKind::Unproved { step_scale, body } => {
            expression(step_scale, env)?;
            for stmt in &mut body.body.stmts { statement(stmt, env)?; }
            expression(&mut body.value, env)
        }
        ExprKind::FiniteDiff { args, .. } | ExprKind::Call { args, .. } | ExprKind::Ctor { args, .. } => {
            *args = arguments(args, env)?;
            Ok(())
        }
        ExprKind::Member(base, _) | ExprKind::Unary(_, base) => expression(base, env),
        ExprKind::Index(a, b) | ExprKind::Binary(_, a, b) => { expression(a, env)?; expression(b, env) }
        ExprKind::Num { .. } | ExprKind::Bool(_) | ExprKind::String(_) => Ok(()),
    }
}
