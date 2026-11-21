use super::{Prog, RowVal, fn_name_arg, require_schedule_fn, stage0};
use loft_emit::emit::shade_tiles::{Lighting, Visibility};
use loft_ir::{Code, Diag, Res};
use loft_parse::ast::{Arg, Expr, ExprKind, Geo, PassDecl, Space, Ty};
use std::collections::BTreeSet;

pub(crate) fn lighting<'a>(
    p: &Prog, types: &mut Vec<Ty>, pass: &'a PassDecl, overrides: &[(String, f64)],
) -> Res<Lighting<'a>> {
    let mut names = BTreeSet::new();
    for arg in &pass.args {
        if let Some(name) = &arg.name
            && (!["light", "shade", "samples", "sun"].contains(&name.name.as_str())
                || !names.insert(name.name.as_str()))
        {
            return Err(Diag::new(Code::E003, name.span, format!("unknown or duplicate shade_tiles argument {}", name.name),
                "use one shade or light callback, a sun direction and optional samples"));
        }
    }
    let mut positional = pass.args.iter().filter(|arg| arg.name.is_none());
    if !matches!(positional.next(), Some(Arg { value: Expr { kind: ExprKind::Name(name), .. }, .. }) if name == "Vis64")
        || positional.next().is_some()
    {
        return Err(Diag::new(Code::E003, pass.verb.span, "shade_tiles needs one Vis64 input", "pass Vis64 first"));
    }
    let visibility = match pass.args.iter().find(|a| a.name.as_ref().is_some_and(|n| n.name == "samples")) {
        None => Visibility::Center,
        Some(a) => match stage0(p, types, &a.value, overrides)? {
            RowVal::U(1) | RowVal::I(1) => Visibility::Center,
            RowVal::U(4) | RowVal::I(4) => Visibility::SubpixelEdges,
            _ => return Err(Diag::new(Code::E602, a.value.span, "shade_tiles samples must be 1 or 4",
                "use samples: 4 for subpixel edge coverage")),
        },
    };
    match (names.contains("light"), names.contains("shade"), visibility) {
        (true, false, Visibility::Center) => Ok(Lighting::Bsdf(fn_name_arg(p, pass, "light")?)),
        (false, true, visibility) => {
            let callback = fn_name_arg(p, pass, "shade")?;
            let direction = Ty::Geo(Geo::Dir, Space::Named("World".into()));
            require_schedule_fn(p, types, callback, pass.verb.span,
                &[Ty::Struct("World".into()), Ty::Struct("Hit".into()), direction, Ty::Bool, Ty::VecN(2)], Ty::VecN(3),
                "write fn shade(w: World, h: Hit, wo: d3@World, ink: bool, px: v2) -> rgb")?;
            Ok(Lighting::World { callback, visibility })
        }
        _ => Err(Diag::new(Code::E003, pass.verb.span, "shade_tiles needs either light or shade",
            "use shade: callback for World queries and subpixel coverage")),
    }
}

#[cfg(test)]
mod tests {
    use crate::program::Pass;
    use loft_ir::Code;
    use std::path::Path;

    #[test]
    fn tiled_world_callback_keeps_raster_passes() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").canonicalize().unwrap();
        let fixture = include_str!("../../../stdlib/engines/capsules.loft");
        let scene = format!("use stdlib::lighting::world::*
{fixture}");
        let scene = scene.replace(
            "schedule Main {",
            "fn probe(w: World, h: Hit, wo: d3@World, ink: bool, px: v2) -> rgb { visible(w, h.p, h.p) }
schedule Main {",
        ).replace("light: sun_sky", "shade: probe, samples: 4");
        let entry = root.join("stdlib/engines/capsules.loft");
        let src = crate::source::Graph::text(&root, &entry, &scene);
        let (program, _, compiled) = crate::compile(&src, &[("count".into(), 1.0)])
            .unwrap_or_else(|e| panic!("{}", e.render(&src)));
        assert!(matches!(&program.passes[..], [Pass::Cull, Pass::Vis(_), Pass::Classify, Pass::Shade(_), Pass::Present]));
        assert!(program.path.is_none());
        assert!(program.modules[0].wgsl.contains(", hit, wo, false, px)"));
        let bundle = crate::bundle::lower(&program, &compiled, (64, 64), |_, _| vec![0; 256]);
        assert!(bundle.is_ok(), "{:?}", bundle.err());

        let src = crate::source::Graph::text(&root, &entry, &scene.replace("samples: 4", "samples: 2"));
        assert_eq!(crate::compile(&src, &[]).err().unwrap().code, Code::E602);
    }
}
