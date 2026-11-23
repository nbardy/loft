//! Elaborate an omitted render schedule from the authored realtime recipe.
use loft_ir::{Code, Diag, Res, Span};
use loft_parse::ast::*;

pub const ENV_DEFAULTS: [&str; 3] = ["DEFAULT_SUN_COLOR", "DEFAULT_ZENITH", "DEFAULT_HORIZON"];

/// Compose the render schedule (the scene's own, or the selected recipe around it), then inline every call to a
/// schedule function from `library` (parametrised schedules of every module).
pub fn schedule(modules: &mut Vec<Module>, next: &mut u32, recipe: Option<ScheduleDecl>, library: &[ScheduleDecl]) -> Res<()> {
    compose(modules, next, recipe)?;
    if let Some(s) = modules.iter_mut().flat_map(|m| &mut m.items).find_map(|i| {
        if let Item::Schedule(s) = i { Some(s) } else { None }
    }) {
        s.passes = expand(std::mem::take(&mut s.passes), library, 0)?;
    }
    Ok(())
}

fn compose(modules: &mut Vec<Module>, next: &mut u32, recipe: Option<ScheduleDecl>) -> Res<()> {
    let items = || modules.iter().flat_map(|m| &m.items);
    let mut cameras = items().filter_map(|i| if let Item::Camera(c) = i { Some(c) } else { None });
    let camera = cameras.next();
    if let Some(c) = cameras.next() {
        return Err(Diag::new(Code::E520, c.name.span, "multiple default cameras", "declare one camera"));
    }
    let authored = items().find_map(|i| if let Item::Schedule(s) = i { Some(s) } else { None });
    let woodblock = authored.and_then(|s| s.passes.iter().find(|p| p.verb.name == "woodblock"));
    let complete = authored.is_some_and(|s| s.passes.iter().any(|p| p.verb.name == "render_bundle"));
    if complete { return Ok(()); }
    let mut schedule = recipe.ok_or_else(|| Diag::new(Code::E520, Span::NONE,
        "no render schedule was selected", "import an engine or write a complete schedule"))?;
    if let Some(call) = woodblock {
        let bad = || Diag::new(Code::E520, call.verb.span, "invalid two-world woodblock schedule",
            "write woodblock(PrimaryWorld, SolidShadow, MainCam) with declared worlds and camera");
        let [primary, shadow, cam] = call.args.as_slice() else { return Err(bad()); };
        let names = [primary, shadow, cam].map(|a| match &a.value.kind { ExprKind::Name(n) => Some(n.as_str()), _ => None });
        let [Some(primary), Some(shadow), Some(cam)] = names else { return Err(bad()); };
        if !camera.is_some_and(|c| c.name.name == cam) { return Err(bad()); }
        let index = crate::Prog::index(modules)?;
        let geometry = |name| -> Res<Vec<Arg>> {
            let kinds = index.worlds.get(name).ok_or_else(bad)?;
            Ok(kinds.iter().filter(|k| !index.kinds[k.name.as_str()].queries.is_empty()).map(|k| Arg {
                name: None, value: Expr { id: 0, span: k.span, kind: ExprKind::Name(k.name.clone()) },
            }).collect())
        };
        let primary = geometry(primary)?;
        let shadow = geometry(shadow)?;
        for pass in &mut schedule.passes {
            match pass.verb.name.as_str() {
                "cull_indirect" => pass.args = primary.clone(),
                "render_bundle" => pass.body[0].1 = primary.clone(),
                "find_shadow_map" => {
                    pass.args.retain(|a| a.name.is_some());
                    pass.args.extend(shadow.clone());
                }
                _ => {}
            }
        }
    }
    schedule.implicit = true;
    let mut before = vec![];
    if let Some(camera) = camera.filter(|_| {
        !authored.is_some_and(|s| s.passes.iter().any(|p| matches!(p.verb.name.as_str(), "orbit" | "use_camera")))
    }) {
        let name = Ident { name: "use_camera".into(), span: camera.name.span };
        let args = vec![Arg { name: None, value: Expr {
            id: *next, span: camera.name.span, kind: ExprKind::Name(camera.name.name.clone()),
        } }];
        *next += 1;
        before.push(PassDecl { label: name.clone(), verb: name, args, body: vec![], resources: None, clock: None });
    }
    if let Some(s) = authored {
        schedule.name = s.name.clone();
        if s.radiance.is_some() { schedule.radiance = s.radiance.clone(); }
        before.extend(s.passes.iter().filter(|p| (p.verb.name != "realtime" || !p.args.is_empty()) && p.verb.name != "woodblock").cloned());
    }
    if before.iter().any(|p| matches!(p.verb.name.as_str(), "orbit" | "use_camera")) {
        schedule.passes.retain(|p| p.verb.name != "orbit");
    }
    before.append(&mut schedule.passes);
    schedule.passes = before;
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
    step.args = arguments(&step.args, env)?;
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
        ExprKind::If { condition, then, otherwise } => {
            expression(condition, env)?;
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
