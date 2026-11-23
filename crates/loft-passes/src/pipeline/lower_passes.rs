use super::{Timings, schedule::{Schedule, Step}, stage0::Stage0Scene};
use crate::{CompileOptions, Prog, bake_tex, emit, shader::naga_module};
use crate::frame_pass::{Facts, FramePass};
use crate::program::{Dispatch, Draw, Pass, ShaderModule, abi};
use crate::source::Graph;
use loft_eval::pack;
use loft_eval::eval::{RowVal, Tables};
use loft_ir::{Code, Diag, Res};
use loft_parse::ast::*;
use std::collections::{BTreeMap, HashSet};
use emit::shade_tiles::{Lighting, Visibility};

pub(super) struct LoweredPasses {
    pub(super) bakes: Vec<bake_tex::Bake>,
    pub(super) dispatches: Vec<Dispatch>,
    pub(super) shader_modules: Vec<ShaderModule>,
    pub(super) passes: Vec<Pass>,
    pub(super) store_energy: bool,
}

/// A frame step once its module index is known; frame passes become `Pass::Authored` after naga reflects them.
enum Lowered<'s, 'a> {
    Stock(Pass),
    Frame(&'s FramePass<'a>, usize),
}

pub(super) fn lower(
    p: &Prog<'_>, modules: &[Module], types: &mut Vec<Ty>, src: &Graph,
    opts: &CompileOptions<'_>, schedule: &mut Schedule<'_>, scene: &mut Stage0Scene, timings: &mut Timings,
) -> Res<LoweredPasses> {
    let CompileOptions { aov, features, .. } = *opts;
    let Schedule { drawn, lighting, .. } = schedule;
    let Stage0Scene { kinds, lut_refs, .. } = &*scene;
    let light = match *lighting {
        Lighting::Bsdf(name, _) => Some(name),
        Lighting::World { .. } => None,
    };
    let fixed = fixed_parameters(p, &scene.tables)?;
    let mut texts = Vec::new();
    for (name, kt) in drawn.iter().zip(kinds) {
        let shader = emit::emit_kind_module(p, types, p.kinds[*name], light, lut_refs, kt.draw, aov, None, &fixed)?;
        texts.push((name.to_string(), shader));
    }
    let visibility = lower_world(p, types, opts, schedule, scene, &mut texts)?;
    let dispatches = lower_dispatches(p, types, opts, schedule, scene, &fixed, &mut texts)?;
    let Schedule { path, steps, shadow_size, path_entry, paper, ids, .. } = schedule;
    let Stage0Scene { kinds, lut_refs, .. } = scene;
    let mut order = vec![];
    if let Some(path) = path.as_ref() {
        // The path integrator draws the whole frame; the raster steps and the present template are not lowered.
        *shadow_size = 0;
        order.push(Lowered::Stock(Pass::Path(texts.len())));
        texts.push(("present".into(), emit::paths::module(p, types, kinds, &texts, lut_refs, path, path_entry.expect("checked path entry"))?));
    } else {
        let facts = Facts { drawn: ids.len() as u32, edges: visibility, aov, paper: *paper };
        for step in steps.iter() {
            match step {
                Step::Stock(pass) => order.push(Lowered::Stock(pass.clone())),
                Step::Frame(frame) => {
                    order.push(Lowered::Frame(frame, texts.len()));
                    texts.push((frame.decl.label.name.clone(), frame.module(p, types, src, &facts)?));
                }
            }
        }
    }
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
    let passes = order.into_iter().map(|step| match step {
        Lowered::Stock(pass) => Ok(pass),
        Lowered::Frame(frame, index) => frame.lowered(&shader_modules[index], index).map(Pass::Authored),
    }).collect::<Res<Vec<_>>>()?;
    Ok(LoweredPasses { bakes, dispatches, shader_modules, passes, store_energy })
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

/// Each kind parameter as a WGSL literal: the one value every row of its kind was emitted with.
/// Rows still carry the value, so CPU evaluation and live edits read it as a column.
fn fixed_parameters(p: &Prog<'_>, tables: &Tables) -> Res<BTreeMap<(String, String), String>> {
    let mut fixed = BTreeMap::new();
    for &name in &p.kind_order {
        let kind = p.kinds[name];
        let rows = &tables[p.kind_index(name)];
        for prm in &kind.params {
            let column = kind.row.iter().position(|f| f.name.name == prm.name.name).expect("parsed: a parameter is a column");
            let mut values: Vec<String> = vec![];
            for row in rows {
                let (literal, number) = match &row[column] {
                    RowVal::U(v) => (format!("{v}u"), f64::from(*v)),
                    RowVal::I(v) => (format!("{v}i"), f64::from(*v)),
                    RowVal::B(v) => (v.to_string(), 0.0),
                    other @ (RowVal::F(_) | RowVal::V(_) | RowVal::X(_)) => {
                        unreachable!("checked: a kind parameter is bool or an integer, not {other:?}")
                    }
                };
                if let Some((ExprKind::Num { value: lo, .. }, ExprKind::Num { value: hi, .. })) =
                    prm.range.as_ref().map(|(lo, hi)| (&lo.kind, &hi.kind)) && !(*lo..*hi).contains(&number) {
                    return Err(Diag::new(Code::E602, prm.span, format!("kind parameter `{}` = {number} lies outside {lo}..{hi}", prm.name.name),
                        "emit a value in the parameter's domain (the upper bound is excluded)"));
                }
                if !values.contains(&literal) { values.push(literal); }
            }
            match values.as_slice() {
                [] => {}
                [one] => { fixed.insert((name.to_string(), prm.name.name.clone()), one.clone()); }
                many => return Err(Diag::new(Code::E402, prm.span,
                    format!("kind parameter `{}` takes {} values in this scene ({})", prm.name.name, many.len(), many.join(", ")),
                    format!("a scene compiles one variant per kind: emit one value, or move `{}` into the column list", prm.name.name))),
            }
        }
    }
    Ok(fixed)
}

fn lower_dispatches(
    p: &Prog<'_>, types: &mut Vec<Ty>, opts: &CompileOptions<'_>, schedule: &Schedule<'_>,
    scene: &Stage0Scene, fixed: &BTreeMap<(String, String), String>, texts: &mut Vec<(String, String)>,
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
            if let Some((entry, dispatch)) = emit::motion_module(p, types, kind, priors, fixed, output)? {
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
