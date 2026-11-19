//! Elaborate an omitted render schedule from the authored realtime recipe.
use loft_ir::{Code, Diag, Res, Span};
use loft_parse::ast::*;

pub(crate) const ENV_DEFAULTS: [&str; 3] = ["DEFAULT_SUN_COLOR", "DEFAULT_ZENITH", "DEFAULT_HORIZON"];

pub fn schedule(modules: &mut Vec<Module>, next: &mut u32) -> Res<()> {
    let items = || modules.iter().flat_map(|m| &m.items);
    let mut cameras = items().filter_map(|i| if let Item::Camera(c) = i { Some(c) } else { None });
    let camera = cameras.next();
    if let Some(c) = cameras.next() {
        return Err(Diag::new(Code::E520, c.name.span, "multiple default cameras", "declare one orbit camera"));
    }
    let authored = items().find_map(|i| if let Item::Schedule(s) = i { Some(s) } else { None });
    if authored.is_some_and(|s| s.passes.iter().any(|p| p.verb.name == "render_bundle")) {
        return Ok(());
    }
    let recipe = crate::parse::parse(Span::NONE.file, include_str!("../../../engines/realtime.loft"), next)?;
    let [Item::Schedule(schedule)] = recipe.items.as_slice() else { unreachable!("one default schedule") };
    let mut schedule = schedule.clone();
    schedule.implicit = true;
    let mut before = vec![];
    if let Some(camera) = camera {
        let name = Ident { name: "orbit".into(), span: camera.name.span };
        let args = camera.fields.iter().map(|(n, e)| Arg { name: Some(n.clone()), value: e.clone() }).collect();
        before.push(PassDecl { label: name.clone(), verb: name, args, body: vec![], resources: None });
    }
    if let Some(s) = authored {
        schedule.name = s.name.clone();
        schedule.waivers = s.waivers.clone();
        before.extend(s.passes.iter().filter(|p| p.verb.name != "realtime" || !p.args.is_empty()).cloned());
    }
    if before.iter().any(|p| p.verb.name == "orbit") {
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
