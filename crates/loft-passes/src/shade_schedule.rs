use super::{Prog, fn_name_arg, require_schedule_fn};
use loft_emit::emit::shade_tiles::{Lighting, Visibility};
use loft_ir::{Code, Diag, Res};
use loft_parse::ast::{Arg, Expr, ExprKind, Geo, PassDecl, Space, Ty};
use std::collections::BTreeSet;

pub(crate) fn lighting<'a>(
    p: &Prog, types: &mut Vec<Ty>, pass: &'a PassDecl, _overrides: &[(String, f64)],
) -> Res<Lighting<'a>> {
    let mut names = BTreeSet::new();
    for arg in &pass.args {
        if let Some(name) = &arg.name
            && (!["light", "shade", "edges", "sun"].contains(&name.name.as_str())
                || !names.insert(name.name.as_str()))
        {
            return Err(Diag::new(Code::E003, name.span, format!("unknown or duplicate shade_tiles argument {}", name.name),
                "use one shade or light callback, a sun direction and optional edges"));
        }
    }
    let mut positional = pass.args.iter().filter(|arg| arg.name.is_none());
    if !matches!(positional.next(), Some(Arg { value: Expr { kind: ExprKind::Name(name), .. }, .. }) if name == "Vis64")
        || positional.next().is_some()
    {
        return Err(Diag::new(Code::E003, pass.verb.span, "shade_tiles needs one Vis64 input", "pass Vis64 first"));
    }
    // Omitting `edges` keeps the centre picture: `coverage` changes every silhouette pixel of scenes approved without
    // it, which is an owner decision (agent_notes/2026-11-23_perf/perf-edges.md).
    let visibility = match pass.args.iter().find(|a| a.name.as_ref().is_some_and(|n| n.name == "edges")) {
        None => Visibility::Center,
        Some(a) => match &a.value.kind {
            ExprKind::Name(n) if n == "none" => Visibility::Center,
            ExprKind::Name(n) if n == "coverage" => Visibility::Coverage,
            ExprKind::Name(n) if n == "taps" => Visibility::SubpixelEdges,
            _ => return Err(Diag::new(Code::E602, a.value.span, "shade_tiles edges must be none, coverage or taps",
                "use edges: coverage for analytic edge antialiasing or edges: taps for four rays per edge pixel")),
        },
    };
    match (names.contains("light"), names.contains("shade"), visibility) {
        (true, false, Visibility::SubpixelEdges) => Err(Diag::new(Code::E003, pass.verb.span,
            "edges: taps needs a shade callback", "use shade: callback, or edges: coverage with light:")),
        (true, false, visibility) => Ok(Lighting::Bsdf(fn_name_arg(p, pass, "light")?, visibility)),
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
        ).replace("light: sun_sky", "shade: probe, edges: taps");
        let entry = root.join("stdlib/engines/capsules.loft");
        let src = crate::source::Graph::text(&root, &entry, &scene);
        let (program, _, compiled) = crate::compile(&src, &[("count".into(), 1.0)])
            .unwrap_or_else(|e| panic!("{}", e.render(&src)));
        assert!(matches!(&program.passes[..], [Pass::Cull, Pass::Vis(_), Pass::Classify, Pass::Shade(_), Pass::Present]));
        assert!(program.path.is_none());
        assert!(program.modules[0].wgsl.contains(", hit, wo, false, px)"));
        let bundle = crate::bundle::lower(&program, &compiled, (64, 64), |_, _| vec![0; 256]);
        assert!(bundle.is_ok(), "{:?}", bundle.err());

        let src = crate::source::Graph::text(&root, &entry, &scene.replace("edges: taps", "edges: four"));
        assert_eq!(crate::compile(&src, &[]).err().unwrap().code, Code::E602);

        // Coverage lowers on both paths: every kind's field in the World module, the own field in a light: module.
        for scene in [scene.replace("edges: taps", "edges: coverage"),
            format!("use stdlib::lighting::world::*\n{fixture}").replace("light: sun_sky", "light: sun_sky, edges: coverage")]
        {
            let src = crate::source::Graph::text(&root, &entry, &scene);
            let (program, _, compiled) = crate::compile(&src, &[("count".into(), 1.0)])
                .unwrap_or_else(|e| panic!("{}", e.render(&src)));
            assert!(program.modules[0].wgsl.contains("loft_coverage_alpha(px, v, near.y != 0u)"));
            assert!(crate::bundle::lower(&program, &compiled, (64, 64), |_, _| vec![0; 256]).is_ok());
        }
    }
}
