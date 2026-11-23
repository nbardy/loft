use super::{Timings, schedule::Schedule, stage0::Stage0Scene};
use crate::{CompileOptions, Prog, bake_tex, emit, shader::naga_module};
use crate::program::{Dispatch, Draw, Pass, ShaderModule, abi};
use loft_eval::pack;
use loft_ir::{Code, Diag, Res, Sources};
use loft_parse::ast::*;
use std::collections::HashSet;
use emit::shade_tiles::{Lighting, Visibility};

pub(super) struct LoweredPasses {
    pub(super) bakes: Vec<bake_tex::Bake>,
    pub(super) dispatches: Vec<Dispatch>,
    pub(super) shader_modules: Vec<ShaderModule>,
    pub(super) shared: usize,
    pub(super) store_energy: bool,
}

pub(super) fn lower(
    p: &Prog<'_>, modules: &[Module], types: &mut Vec<Ty>, src: &Sources,
    opts: &CompileOptions<'_>, schedule: &mut Schedule<'_>, scene: &mut Stage0Scene, timings: &mut Timings,
) -> Res<LoweredPasses> {
    let CompileOptions { aov, features, .. } = *opts;
    let Schedule { drawn, lighting, .. } = schedule;
    let Stage0Scene { kinds, lut_refs, .. } = &*scene;
    let light = match *lighting {
        Lighting::Bsdf(name, _) => Some(name),
        Lighting::World { .. } => None,
    };
    let mut texts = Vec::new();
    for (name, kt) in drawn.iter().zip(kinds) {
        let shader = emit::emit_kind_module(p, types, p.kinds[*name], light, lut_refs, kt.draw, aov, None)?;
        texts.push((name.to_string(), shader));
    }
    let visibility = lower_world(p, types, opts, schedule, scene, &mut texts)?;
    let dispatches = lower_dispatches(p, types, opts, schedule, scene, &mut texts)?;
    let Schedule { path, passes, shadow_size, path_entry, sky, paper, film, ids, .. } = schedule;
    let Stage0Scene { kinds, lut_refs, .. } = scene;
    let present = if let Some(path) = path.as_ref() {
        passes.retain(|pass| matches!(pass, Pass::Present));
        *shadow_size = 0;
        emit::paths::module(p, types, kinds, &texts, lut_refs, path, path_entry.expect("checked path entry"))?
    } else {
        emit::shade_tiles::present_module(p, types, sky, aov, *paper, *film, visibility)
    };
    let shared = texts.len();
    texts.push(("cull".into(), emit::cull_module(ids.len() == 1)));
    texts.push(("classify".into(), emit::shade_tiles::classify_module(visibility)));
    texts.push(("present".into(), present));
    let mut bakes = Vec::new();
    for item in modules.iter().flat_map(|m| &m.items) {
        if let Item::Bake(decl) = item {
            let (bake, wgsl) = bake_tex::bake_module(p, types, decl, texts.len())?;
            texts.push((bake.name.clone(), wgsl));
            bakes.push(bake);
        }
    }
    timings.lap("emit");
    let store_energy = texts.iter().any(|(_, w)| w.contains("loft_ggx_energy(")
        && (w.contains("var<storage, read> luts:") || w.contains("var<storage, read> world_luts:")));
    let shader_modules = texts.into_iter().map(|(n,w)| {
        let mut m=naga_module(&n, w, src, features)?;
        if let Some(k)=p.kinds.get(n.as_str()) {let (file,line,col,_,_)=src.locate(k.name.span); m.span=Some((file,line,col));}
        Ok(m)
    }).collect::<Res<Vec<_>>>()?;
    timings.lap("naga");
    Ok(LoweredPasses { bakes, dispatches, shader_modules, shared, store_energy })
}

fn lower_world(
    p: &Prog<'_>, types: &[Ty], opts: &CompileOptions<'_>, schedule: &mut Schedule<'_>,
    scene: &mut Stage0Scene, texts: &mut [(String, String)],
) -> Res<Visibility> {
    let Schedule { lighting, tiled_world, drawn, ids, sky, decl: sched, .. } = schedule;
    let Stage0Scene { kinds, instances, lut_words, lut_refs, .. } = scene;
    let samples = opts.samples;
    let visibility = match (*lighting, tiled_world.as_mut()) {
        (Lighting::World { callback, visibility }, Some(world)) => {
            // A World view needs the same bounds/rows as the primary raster.
            if sched.passes.iter().any(|pass| pass.verb.name == "run_laws") && samples.is_empty() {
                return Err(Diag::new(Code::E405, sched.name.span, "tiled World queries need a geometry snapshot",
                    "snapshot motion before shading with World queries"));
            }
            let primary_world = if drawn.len() == ids.len() {
                0
            } else {
                let index = world.worlds.len() as u32;
                world.worlds.push(emit::paths::WorldResources { kinds: ids.clone(), media: false });
                index
            };
            let span = p.fns[callback].name.span;
            loft_eval::bvh::pack_world(kinds, instances, world, lut_words)?;
            let wgsl = emit::shade_tiles::world_source(p, types, kinds, texts, span)?;
            for (_, source) in texts {
                *source = emit::shade_tiles::module(p, types, lut_refs, source, &wgsl, callback, sky, visibility, primary_world);
            }
            visibility
        }
        // Surface grids and meshes have no field to measure a closest approach, so their modules keep the centre path.
        (Lighting::Bsdf(_, Visibility::Coverage), _) if opts.aov == emit::Aov::Beauty => {
            for ((_, source), kind) in texts.iter_mut().zip(kinds.iter()) {
                if matches!(kind.draw, Draw::Analytic | Draw::Traced) {
                    *source = emit::shade_tiles::bsdf_coverage(source);
                }
            }
            Visibility::Coverage
        }
        _ => Visibility::Center,
    };
    Ok(visibility)
}

fn lower_dispatches(
    p: &Prog<'_>, types: &mut Vec<Ty>, opts: &CompileOptions<'_>, schedule: &Schedule<'_>,
    scene: &Stage0Scene, texts: &mut Vec<(String, String)>,
) -> Res<Vec<Dispatch>> {
    let CompileOptions { samples, overrides, .. } = *opts;
    let Schedule { drawn, decl: sched, .. } = schedule;
    let Stage0Scene { tables, priors, .. } = scene;
    let mut dispatches = vec![];
    if samples.is_empty() {
        for &name in &p.kind_order {
            use crate::program::DispatchRows;
            let kind = p.kinds[name];
            let draw = drawn.iter().position(|&n| n == name);
            let table = &tables[p.kind_index(name)];
            let output = || {
                if let Some(draw) = draw {
                    return Ok((texts.len(), DispatchRows::Drawable(draw)));
                }
                if table.is_empty() {
                    return Err(Diag::new(Code::E602, kind.name.span, "scheduled table has no rows", "emit a row or remove its scheduled operation"));
                }
                let mut words = vec![];
                for row in table {
                    pack(row, &mut words);
                    words.extend([0.; abi::HIDDEN_WORDS as usize]);
                }
                Ok((texts.len(), DispatchRows::Table {
                    name: name.into(), n_rows: table.len() as u32, stride: (words.len() / table.len()) as u32, words,
                }))
            };
            if let Some((entry, dispatch)) = emit::motion_module(p, types, kind, priors, output)? {
                texts.push((format!("{name}.advance"), entry));
                dispatches.push(dispatch);
            }
        }
        let mut solver_tables = HashSet::new();
        for pass in sched.passes.iter().filter(|p| p.verb.name == "run_solver") {
            let ExprKind::Name(name) = &pass.args[0].value.kind else { unreachable!("checked solver name"); };
            let solver = p.solvers[name.as_str()];
            let table = &solver.table[0];
            if p.kinds.contains_key(table.name.as_str()) || !solver_tables.insert(table.name.as_str()) {
                return Err(Diag::new(Code::E502, table.span, "the scheduled solver state table has multiple owners",
                    "use one scheduled solver per dedicated state table"));
            }
            let (entry, dispatch) = emit::solver::lower(p, types, texts.len(), solver,
                |p, types, e| loft_eval::solver::initial_words(p, types, e, overrides))?;
            texts.push((format!("{name}.advance"), entry));
            dispatches.push(dispatch);
        }
    }
    Ok(dispatches)
}
