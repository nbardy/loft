//! Elaborate an omitted render schedule from the authored realtime recipe.
use loft_ir::{Code, Diag, Res, Span};
use loft_parse::ast::*;

pub const ENV_DEFAULTS: [&str; 3] = ["DEFAULT_SUN_COLOR", "DEFAULT_ZENITH", "DEFAULT_HORIZON"];

pub fn schedule(modules: &mut Vec<Module>, next: &mut u32, recipe: Option<ScheduleDecl>) -> Res<()> {
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
        before.push(PassDecl { label: name.clone(), verb: name, args, body: vec![], resources: None });
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
