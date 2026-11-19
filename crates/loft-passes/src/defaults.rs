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
    let selection = items().filter_map(|item| match item {
        Item::Use(path) if matches!(path.as_slice(), [a, _] if a.name == "engines") => path.get(1),
        _ => None,
    }).next_back();
    let authored = items().find_map(|i| if let Item::Schedule(s) = i { Some(s) } else { None });
    if authored.is_some_and(|s| s.passes.iter().any(|p| p.verb.name == "render_bundle")) {
        return Ok(());
    }
    let text = match selection.map(|n| n.name.as_str()) {
        Some("reference") => include_str!("../../../engines/reference.loft"),
        Some("woodblock") => include_str!("../../../engines/woodblock.loft"),
        Some("spectral_caustic") => include_str!("../../../engines/spectral_caustic.loft"),
        _ => include_str!("../../../engines/realtime.loft"),
    };
    let mut recipe = crate::parse::parse(Span::NONE.file, text, next)?;
    let index = recipe.items.iter().position(|i| matches!(i, Item::Schedule(_))).expect("one default schedule");
    let Item::Schedule(mut schedule) = recipe.items.remove(index) else { unreachable!("found schedule") };
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
        if s.radiance.is_some() { schedule.radiance = s.radiance.clone(); }
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
    modules.push(recipe);
    Ok(())
}
