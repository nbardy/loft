//! Expand the default Phase 1 renderer to the same passes as an explicit schedule.
use loft_ir::{Code, Diag, Res, Span, ast::*};

pub fn schedule(modules: &mut Vec<Module>, next: &mut u32) -> Res<()> {
    let items = || modules.iter().flat_map(|m| &m.items);
    let mut cameras = items().filter_map(|i| {
        if let Item::Camera(c) = i {
            Some(c)
        } else {
            None
        }
    });
    let camera = cameras.next();
    if let Some(c) = cameras.next() {
        return Err(Diag::new(
            Code::E520,
            c.name.span,
            "multiple default cameras",
            "declare one orbit camera",
        ));
    }
    let sched = items().find_map(|i| {
        if let Item::Schedule(s) = i {
            Some(s)
        } else {
            None
        }
    });
    if sched.is_some_and(|s| s.passes.iter().any(|p| p.verb.name == "render_bundle")) {
        return Ok(());
    }
    let kinds: Vec<_> = items()
        .filter_map(|i| match i {
            Item::Kind(k) if !k.queries.is_empty() => Some(k),
            _ => None,
        })
        .collect();
    let span = sched
        .map(|s| s.name.span)
        .or(camera.map(|c| c.name.span))
        .unwrap_or(Span::NONE);
    let id = |s: &str| Ident {
        name: s.into(),
        span,
    };
    let mut expr = |kind| {
        let i = *next;
        *next += 1;
        Expr { id: i, kind, span }
    };
    let mut named = |s: &str| expr(ExprKind::Name(s.into()));
    let arg = |name: Option<&str>, value| Arg {
        name: name.map(id),
        value,
    };
    let pass = |s: &str, args, body| PassDecl {
        label: id(s),
        verb: id(s),
        args,
        body,
    };
    let drawn: Vec<_> = kinds
        .iter()
        .map(|k| arg(None, named(&k.name.name)))
        .collect();
    let mut casters: Vec<_> = kinds
        .iter()
        .filter(|k| crate::pose_field(k).is_some())
        .map(|k| arg(None, named(&k.name.name)))
        .collect();
    let vis = arg(None, named("Vis64"));
    let light = arg(Some("light"), named("sun_sky"));
    let sky = arg(Some("sky"), named("sky_color"));
    let hdr = arg(None, named("ColorHdr"));
    let mut num = |v: f64| {
        expr(ExprKind::Num {
            value: v,
            suffix: NumSuffix::None,
            text: v.to_string(),
            float: true,
        })
    };
    let mut passes = vec![];
    if let Some(c) = camera {
        passes.push(pass(
            "orbit",
            c.fields
                .iter()
                .map(|(n, e)| Arg {
                    name: Some(n.clone()),
                    value: e.clone(),
                })
                .collect(),
            vec![],
        ))
    }
    if let Some(s) = sched {
        for p in &s.passes {
            match p.verb.name.as_str() {
                "orbit" | "run_laws" | "check" => passes.push(p.clone()),
                "realtime" if p.args.is_empty() => {}
                _ => {
                    return Err(Diag::new(
                        Code::E520,
                        p.verb.span,
                        format!("unsupported schedule call {}", p.verb.name),
                        "call realtime() or provide an explicit render bundle",
                    ));
                }
            }
        }
    }
    let has_cam = passes.iter().any(|p| p.verb.name == "orbit");
    let orbit_values = [0., 0., 0., 4., 0.5, 24., 40f64.to_radians()].map(&mut num);
    let side = num(1024.);
    let tile = num(8.);
    let default_sun = [0.5, 0.75, 0.35].map(&mut num);
    let v3 = |values: [Expr; 3]| ExprKind::Ctor {
        ty: TyExpr::Name(id("vec3f")),
        args: values.into_iter().map(|e| arg(None, e)).collect(),
    };
    if !has_cam {
        let [x, y, z, r, h, t, f] = orbit_values;
        passes.push(pass(
            "orbit",
            vec![
                arg(Some("at"), expr(v3([x, y, z]))),
                arg(Some("radius"), r),
                arg(Some("height"), h),
                arg(Some("period"), t),
                arg(Some("fov"), f),
            ],
            vec![],
        ));
    }
    passes.push(pass("cull_indirect", drawn.clone(), vec![]));
    passes.push(pass(
        "render_bundle",
        vec![vis.clone()],
        vec![(id("draw_indirect"), drawn)],
    ));
    if !casters.is_empty() {
        casters.push(arg(Some("size"), side));
        passes.push(pass("find_shadow_map", casters, vec![]))
    }
    passes.push(pass(
        "tile_classify",
        vec![vis.clone(), arg(Some("tile"), tile)],
        vec![],
    ));
    passes.push(pass(
        "shade_tiles",
        vec![vis, light, arg(Some("sun"), expr(v3(default_sun)))],
        vec![],
    ));
    passes.push(pass("tonemap_aces", vec![hdr, sky], vec![]));
    let main = ScheduleDecl {
        name: id("Main"),
        passes,
    };
    if let Some(s) = modules.iter_mut().flat_map(|m| &mut m.items).find_map(|i| {
        if let Item::Schedule(s) = i {
            Some(s)
        } else {
            None
        }
    }) {
        *s = main;
    } else {
        modules.push(Module {
            items: vec![Item::Schedule(main)],
            ..Module::default()
        });
    }
    Ok(())
}
