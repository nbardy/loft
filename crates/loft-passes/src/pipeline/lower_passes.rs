use super::{Timings, schedule::{Schedule, Step}, stage0::Stage0Scene};
use crate::{CompileOptions, Prog, bake_tex, emit, shader::naga_module};
use crate::frame_pass::{Facts, FramePass};
use crate::program::{Dispatch, Draw, Pass, ShaderModule, abi};
use crate::source::Graph;
use loft_eval::pack;
use loft_eval::eval::{RowVal, Tables};
use loft_ir::{Code, Diag, Res};
use loft_parse::ast::*;
use std::collections::BTreeMap;
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
    for ((name, kt), guard) in drawn.iter().zip(kinds).zip(&scene.guard_table) {
        let shader = emit::emit_kind_module(p, types, p.kinds[*name], light, lut_refs, kt.draw, aov, &fixed, guard.map(|base| (base, loft_eval::guard::EYE_REACH)), src)?;
        texts.push((name.to_string(), shader));
    }
    // Rows a kind's queries read (`K[i]`) bind to the read kind's table in the vis, shadow and shade passes. The tiled
    // World and the path tracer compile every kind into one module, which has no such binding.
    for (i, name) in drawn.iter().enumerate() {
        let k = p.kinds[*name];
        let mut reads = vec![];
        for read in emit::kind_reads(p, k) {
            let index = drawn.iter().position(|d| *d == read).ok_or_else(|| Diag::new(Code::E405, k.name.span,
                format!("`{}` reads rows of `{read}`, which is not drawn", k.name.name), format!("draw `{read}`, or copy the column into `{}`", k.name.name)))?;
            if light.is_none() || schedule.path.is_some() {
                return Err(Diag::new(Code::E405, k.name.span, format!("`{}` reads another kind's rows under World shading", k.name.name),
                    "shade with a BSDF light (`shade_tiles(Vis64, light: ...)`), or copy the column into the kind"));
            }
            reads.push(index as u32);
        }
        scene.kinds[i].reads = reads;
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

/// Rows read rows (DESIGN §5.3): a read of kind S sees S after the last earlier law in schedule order that writes
/// S, else as the previous tick left it. Dispatches that read a kind written by another dispatch, are read by one,
/// or read their own kind run in lockstep, one tick at a time, ordered by their kind's first scheduled law or check.
/// That order delivers the rule only if each reader sees S on one side of all of S's laws; anything else is E505.
fn order_reads(p: &Prog<'_>, sched: &ScheduleDecl, drawn: &[&str], dispatches: &mut [Dispatch]) -> Res<()> {
    use crate::program::DispatchRows;
    if dispatches.iter().all(|d| d.reads.is_empty()) { return Ok(()); }
    // Schedule position of each law and check, with the kind it runs on.
    let mut steps: Vec<(usize, &str, &FnDecl, bool)> = vec![];
    for pass in sched.passes.iter().filter(|pass| matches!(pass.verb.name.as_str(), "run_laws" | "check")) {
        let law = pass.verb.name == "run_laws";
        for arg in &pass.args {
            let ExprKind::Name(n) = &arg.value.kind else { continue };
            let f = if law { p.laws.get(n.as_str()).map(|l| &l.f) } else { p.checks.get(n.as_str()).copied() };
            let Some(f) = f else { continue };
            let Some(TyExpr::Name(k)) = f.params.first().and_then(|prm| prm.ty.as_ref()) else { continue };
            steps.push((steps.len(), k.name.as_str(), f, law));
        }
    }
    let position = |kind: &str| steps.iter().find(|s| s.1 == kind).map(|s| s.0);
    let writers = |kind: &str| steps.iter().filter(|s| s.1 == kind && s.3).map(|s| s.0).collect::<Vec<_>>();
    let mut lockstep: Vec<&str> = vec![];
    for &(r, kind, f, _) in &steps {
        for prm in &f.params {
            let Some(TyExpr::Table(read)) = &prm.ty else { continue };
            let read = read.name.as_str();
            if read == kind {
                lockstep.push(kind);
                continue;
            }
            // A kind no law writes is constant: any order reads the same rows.
            let laws = writers(read);
            if laws.is_empty() { continue; }
            lockstep.extend([kind, read]);
            let before = laws.iter().any(|&w| w < r);
            let first = position(read) < position(kind);
            if before != first || (before && laws.iter().any(|&w| w > r)) {
                return Err(Diag::new(Code::E505, f.name.span,
                    format!("`{}` reads `{read}`, whose laws are scheduled on both sides of it or of `{kind}`'s first law", f.name.name),
                    format!("schedule every law that writes `{read}` together, all before or all after the laws of `{kind}`")));
            }
        }
    }
    let kind_of = |d: &Dispatch| match &d.input {
        DispatchRows::Table { name, .. } => name.clone(),
        DispatchRows::Drawable(i) => drawn[*i].to_owned(),
    };
    let kinds: Vec<String> = dispatches.iter().map(kind_of).collect();
    let slots: Vec<usize> = (0..dispatches.len()).filter(|&i| lockstep.contains(&kinds[i].as_str())).collect();
    let mut sorted = slots.clone();
    sorted.sort_by_key(|&i| position(&kinds[i]));
    let moved: Vec<Dispatch> = sorted.iter().map(|&i| Dispatch { lockstep: true, ..dispatches[i].clone() }).collect();
    for (slot, d) in slots.into_iter().zip(moved) { dispatches[slot] = d; }
    Ok(())
}

fn lower_dispatches(
    p: &Prog<'_>, types: &mut Vec<Ty>, opts: &CompileOptions<'_>, schedule: &Schedule<'_>,
    scene: &Stage0Scene, fixed: &BTreeMap<(String, String), String>, texts: &mut Vec<(String, String)>,
) -> Res<Vec<Dispatch>> {
    let CompileOptions { samples, .. } = *opts;
    // Merge (cut-unused dropped `decl` with the run_solver loop; probe-rows' order_reads still needs it).
    let Schedule { drawn, decl: sched, .. } = schedule;
    let Stage0Scene { tables, .. } = scene;
    let mut dispatches = vec![];
    if samples.is_empty() {
        use crate::program::DispatchRows;
        let packed = |name: &str| -> Res<DispatchRows> {
            let table = &tables[p.kind_index(name)];
            let stride = p.kinds[name].row.iter().try_fold(abi::HIDDEN_WORDS, |stride, field| {
                let ty = crate::check::Ck::new(p, &mut vec![]).resolve(&field.ty, false)?;
                Ok::<_, Diag>(stride + ty.words().expect("checked storable column"))
            })?;
            let mut words = vec![];
            for row in table {
                pack(row, &mut words);
                words.extend([0.; abi::HIDDEN_WORDS as usize]);
            }
            Ok(DispatchRows::Table { name: name.into(), n_rows: table.len() as u32, stride, words })
        };
        let rows_of = |name: &str, _| match drawn.iter().position(|&n| n == name) {
            Some(draw) => Ok(DispatchRows::Drawable(draw)),
            None => packed(name),
        };
        for &name in &p.kind_order {
            if tables[p.kind_index(name)].is_empty() { continue; }
            let kind = p.kinds[name];
            let draw = drawn.iter().position(|&n| n == name);
            let output = || {
                if let Some(draw) = draw {
                    return Ok((texts.len(), DispatchRows::Drawable(draw)));
                }
                Ok((texts.len(), packed(name)?))
            };
            if let Some((entry, dispatch)) = emit::motion_module(p, types, kind, fixed, output, &rows_of)? {
                texts.push((format!("{name}.advance"), entry));
                dispatches.push(dispatch);
            }
        }
        order_reads(p, sched, drawn, &mut dispatches)?;
    }
    Ok(dispatches)
}
