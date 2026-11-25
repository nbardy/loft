use super::{Timings, schedule::Schedule};
use crate::{CompileOptions, KindReport, Prog, as_v3, cache, emit, eval, gltf, lip, pose_field, rot};
use crate::program::{Draw, Instance, KindTable, abi};
use crate::source::Graph;
use emit::paths::PathTrace;
use eval::{Eval, Lut, RowVal, Tables, Val};
use loft_check::{defaults, geometry_fields};
use lip::Iv;
use loft_eval::{as_f32, pack, stage0};
use loft_ir::{Code, Diag, Res, Span};
use loft_parse::ast::*;

pub(super) struct Stage0Scene {
    pub(super) tables: Tables,
    pub(super) luts: Vec<Lut>,
    pub(super) lut_words: Vec<f32>,
    pub(super) lut_refs: Vec<emit::LutRef>,
    pub(super) segments: crate::program::SegmentRows,
    pub(super) guard_table: Vec<Option<u32>>,
    pub(super) params: Vec<f32>,
    pub(super) kinds: Vec<KindTable>,
    pub(super) instances: Vec<Instance>,
    pub(super) reports: Vec<KindReport>,
}

pub(super) fn evaluate<'a>(
    p: &Prog<'a>, modules: &'a [Module], types: &mut Vec<Ty>, src: &Graph,
    opts: &CompileOptions<'_>, meshes: &[gltf::Mesh], schedule: &mut Schedule<'_>, timings: &mut Timings,
) -> Res<Stage0Scene> {
    let luts = scans(p, modules, types, opts.overrides)?;
    timings.lap("scans");
    let (tables, params) = evaluate_rows(p, types, src, opts, schedule, &luts)?;
    timings.lap("stage0");

    schedule.select_drawables(p, &tables)?;
    let mut kinds = vec![];
    let mut instances = vec![];
    let mut reports = vec![];
    let mut plans = schedule.drawn.iter().enumerate()
        .map(|(id, name)| plan_kind(p, types, opts, &tables, &luts, meshes, id, name))
        .collect::<Res<Vec<_>>>()?;
    // Every (kind, shape) derivation of the scene runs in one pool. A per-kind pool left cores idle on scenes of
    // several kinds with few shapes each (Hermit: five kinds, eleven shapes).
    let derived = derive_all(p, types, opts, &tables, &luts, &mut plans)?;
    let (mut lut_words, lut_refs) = pack_luts(meshes, &luts, schedule.tiled_world.is_some())?;
    let mut guard_table = vec![None; plans.len()];
    let guard_header = lut_words.len();
    let traced = |plan: &KindPlan<'_, '_>| plan.trace.is_some() && plan.surface.is_none()
        && plan.k.query(QueryKind::Visible).is_none();
    if plans.iter().any(traced) {
        lut_words.resize(guard_header + plans.len(), 0.0);
    }
    for (plan, (derived, derive_s)) in plans.iter().zip(&derived) {
        let first = lut_words.len();
        // A hot edit can change whether this kind admits a guard. Give each
        // traced row its own record so no other kind's LUT addresses move.
        if traced(plan) {
            let address = |n: usize| u32::try_from(n).map_err(|_| Diag::new(
                Code::E602, plan.k.name.span, "visibility intervals exceed the LUT address range", "reduce the number of rows"));
            lut_words[guard_header + plan.id] = f32::from_bits(address(lut_words.len())?);
            if derived.iter().any(|s| s.guard.is_some()) {
                guard_table[plan.id] = Some(address(guard_header)?);
            }
            for &shape in &plan.shape_of {
                let s = &derived[shape];
                let words = match &s.guard {
                    Some(g) => loft_eval::guard::words(&s.proxy, g, pose_field(plan.k).is_some()),
                    None => [0.0; 10],
                };
                lut_words.extend(words);
            }
            address(lut_words.len())?;
        }
        let mesh_asset = |row: &[RowVal]| mesh_asset_of(plan.k, meshes, row);
        let (kind, report) = pack_kind(plan.k, plan.id, &tables[plan.t], plan.mesh_kind, mesh_asset, &plan.shapes,
            &plan.shape_of, derived, plan.step_scale, *derive_s, first..lut_words.len(), &mut instances)?;
        kinds.push(kind);
        reports.push(report);
    }
    let mut ev = Eval::<f32>::new(p, types, tables, opts.overrides);
    let mut segments = crate::program::SegmentRows::new();
    // Surface and mesh geometry take precedence even when the kind also has a visible query.
    for (module, _) in kinds.iter().enumerate().filter(|(_, k)| k.draw == Draw::Analytic && k.n_rows >= 8) {
        let name = &schedule.drawn[module];
        let k = p.kinds[*name];
        let Some(args) = emit::segment_arguments(p, k, src) else { continue };
        let kind = p.kind_index(name);
        let query = k.query(QueryKind::Visible).expect("recognized exact segment");
        let mut balls = Vec::with_capacity(ev.tables[kind].len());
        for row in 0..ev.tables[kind].len() {
            ev.bind(&query.params[0].name.name, Val::Row(kind, row));
            let [a, b, ra, rb] = args.map(|e| ev.expr(e).and_then(|v| eval::concrete(&v, e.span)));
            ev.unbind();
            let pose = pose_field(k).and_then(|index| ev.tables[kind][row].get(index));
            let world = |point: [f32; 3], radius: f32| {
                let p = if let Some(RowVal::X(x)) = pose {
                    let v = rot(x, point);
                    [0, 1, 2].map(|i| v[i] + x.t[i])
                } else { point };
                [p[0], p[1], p[2], radius]
            };
            let a = world(as_v3(a?, args[0].span)?, as_f32(ra?, args[2].span)?);
            let b = world(as_v3(b?, args[1].span)?, as_f32(rb?, args[3].span)?);
            balls.push([a, b]);
        }
        segments.insert(module as u32, balls);
    }
    let tables = ev.tables;
    timings.lap("derive");
    Ok(Stage0Scene { tables, luts, lut_words, lut_refs, segments, guard_table, params, kinds, instances, reports })
}

fn scans<'a>(p: &Prog<'a>, modules: &'a [Module], types: &[Ty], overrides: &[(String, f64)]) -> Res<Vec<Lut>> {
    // Scans, baked in declaration order (a scan may sample one declared before it).
    let mut luts: Vec<eval::Lut> = vec![];
    for item in modules.iter().flat_map(|m| &m.items) {
        let Item::Scan(sc) = item else { continue };
        let lut = {
            let mut ev: Eval<f32> = Eval::new(p, types, vec![], overrides);
            ev.luts = &luts;
            let lo = as_f32(eval::concrete(&ev.expr(&sc.lo)?, sc.lo.span)?, sc.lo.span)?;
            let hi = as_f32(eval::concrete(&ev.expr(&sc.hi)?, sc.hi.span)?, sc.hi.span)?;
            let n = as_f32(eval::concrete(&ev.expr(&sc.steps)?, sc.steps.span)?, sc.steps.span)? as usize;
            if n < 4 || n > 65_536 || hi <= lo {
                return Err(Diag::new(Code::E602, sc.name.span, "a scan needs 4 to 65,536 steps over a range lo < hi", "fix `steps` or the range"));
            }
            let frame = |v: Val<f32>, span: Span| -> Res<[f32; 12]> {
                match v {
                    Val::V(c, _) if c.len() == 12 && c.iter().all(|x| x.is_finite()) => Ok(std::array::from_fn(|i| c[i])),
                    other => Err(Diag::new(Code::E701, span, format!("the scan produced {other:?}"), "keep the step's frame finite (a zero tangent cannot be normalised)")),
                }
            };
            ev.bind(&sc.var.name, Val::S(lo));
            let mut frames = vec![frame(ev.expr(&sc.init)?, sc.init.span)?];
            ev.unbind();
            for i in 1..n {
                let t = lo + (hi - lo) * i as f32 / (n - 1) as f32;
                ev.bind(&sc.prev.name, Val::V(frames[i - 1].to_vec(), 0.0));
                ev.bind(&sc.at.name, Val::S(t));
                frames.push(frame(ev.expr(&sc.step)?, sc.step.span)?);
                ev.unbind();
                ev.unbind();
            }
            eval::Lut { name: sc.name.name.clone(), lo, hi, frames }
        };
        luts.push(lut);
    }
    Ok(luts)
}

fn evaluate_rows(
    p: &Prog<'_>, types: &[Ty], src: &Graph, opts: &CompileOptions<'_>,
    schedule: &mut Schedule<'_>, luts: &[Lut],
) -> Res<(Tables, Vec<f32>)> {
    let CompileOptions { overrides, samples, .. } = *opts;
    let rows_fns = &schedule.rows_fns;
    let mut ev: Eval<f32> = Eval::new(p, types, vec![vec![]; p.kind_order.len()], overrides);
    ev.luts = luts;
    ev.source = Some(src);
    for f in rows_fns {
        ev.call_fn(f, vec![], f.name.span)?;
    }
    for sample in samples{
        let k=p.kinds.get(sample.kind.as_str()).ok_or_else(||Diag::new(Code::E602,Span::NONE,"snapshot names an unknown kind","sample the same scene revision"))?;
        let t=p.kind_index(&sample.kind);
        for (field,values) in &sample.fields{
            let col=k.row.iter().position(|f|f.name.name==*field).ok_or_else(||Diag::new(Code::E602,k.name.span,"snapshot field is absent","sample the current row layout"))?;
            if ev.tables[t].len()!=values.len(){return Err(Diag::new(Code::E602,k.name.span,"motion row count differs from build","sample the same scene and parameter values"))}
            for (i,x) in values.iter().enumerate(){
                if !x.is_finite(){return Err(Diag::new(Code::E602,k.row[col].name.span,"nonfinite motion result","guard the law expression"))}
                if let Some(range)=&k.row[col].range{ev.validate_range(&k.row[col].name,range,*x,k.row[col].name.span)?;}
                ev.tables[t][i][col]=RowVal::F(*x);
            }
        }
    }
    let authored_params = p.continuous_params().map(|(_, c)| as_f32(eval::concrete(&ev.module_expr(&c.value)?, c.name.span)?, c.name.span)).collect::<Res<Vec<_>>>()?;
    let mut environment_defaults = [[0.; 3]; 3];
    for (color, name) in environment_defaults.iter_mut().zip(defaults::ENV_DEFAULTS) {
        let decl = p.consts.get(name).ok_or_else(|| Diag::new(Code::E011, Span::NONE, format!("missing environment default {name}"), "load the light prelude"))?;
        *color = as_v3(eval::concrete(&ev.module_expr(&decl.value)?, decl.name.span)?, decl.name.span)?;
    }
    let tables = ev.tables;
    let environment=|kind:&str|->Res<Option<[[f32;3];2]>>{
        let Some(k)=p.kinds.get(kind) else{return Ok(None)};
        match tables[p.kind_index(kind)].as_slice(){
            []=>Ok(None),
            [row] if row.len()==2=>Ok(Some([as_v3(row[0].clone(),k.name.span)?,as_v3(row[1].clone(),k.name.span)?])),
            _=>Err(Diag::new(Code::E602,k.name.span,format!("expected one {} environment row",kind),"emit one environment for this view")),
        }
    };
    if let Some(path) = schedule.path.as_mut().or(schedule.tiled_world.as_mut()) {
        add_path_resources(p, &tables, path)?;
    }
    let sky_colors=environment("Sky")?;
    let sun_row=environment("Sun")?;
    let [default_sun, default_zenith, default_horizon] = environment_defaults;
    let [sun, sun_color] = sun_row.unwrap_or([schedule.sun, default_sun]);
    schedule.sun = sun;
    let [zenith, horizon] = sky_colors.unwrap_or([default_zenith, default_horizon]);
    let mut params: Vec<f32> = [sun_color, zenith, horizon].into_iter().zip([0., f32::from(sky_colors.is_some()), 0.])
        .flat_map(|([r, g, b], flag)| [r, g, b, flag]).collect();
    params.extend(authored_params.into_iter().flat_map(|x| [x, 0., 0., 0.]));
    Ok((tables, params))
}

fn add_path_resources(p: &Prog<'_>, tables: &Tables, path: &mut PathTrace) -> Res<()> {
    add_media(p, tables, path)?;
    path.lamps.push([[0.0; 4]; 2]); // index zero is the directional sun
    if let Some(lamp) = p.kinds.get("PointLamp") {
        for row in &tables[p.kind_index("PointLamp")] {
            let [pos, color, reach] = row.as_slice() else { return Err(Diag::new(Code::E003, lamp.name.span, "invalid light layout", "use the PointLamp from the light prelude")); };
            let [x,y,z] = as_v3(pos.clone(), lamp.name.span)?;
            let [r,g,b] = as_v3(color.clone(), lamp.name.span)?;
            // Preserve the medium records paired with each authored lamp in the original packing order.
            add_media(p, tables, path)?;
            path.lamps.push([[x, y, z, as_f32(reach.clone(), lamp.name.span)?], [r, g, b, 0.0]]);
        }
    }
    Ok(())
}

fn add_media(p: &Prog<'_>, tables: &Tables, path: &mut PathTrace) -> Res<()> {
    if let Some(medium) = p.kinds.get("FieldMedium") {
        for row in &tables[p.kind_index("FieldMedium")] {
            let [lo, hi, sigma_t, albedo, g, emission] = row.as_slice() else {
                return Err(Diag::new(Code::E003, medium.name.span, "invalid medium layout", "use FieldMedium from the light prelude"));
            };
            let v = |x: &RowVal| as_v3(x.clone(), medium.name.span);
            let (lo, hi, t, albedo, emit) = (v(lo)?, v(hi)?, v(sigma_t)?, v(albedo)?, v(emission)?);
            let g = as_f32(g.clone(), medium.name.span)?;
            if (0..3).any(|i| !(lo[i] < hi[i] && t[i] >= 0.0 && (0.0..=1.0).contains(&albedo[i]) && emit[i] >= 0.0))
                || !(-1.0..1.0).contains(&g) || g == -1.0
            {
                return Err(Diag::new(Code::E602, medium.name.span, "invalid medium coefficients or bounds",
                    "use positive extents, nonnegative extinction and emission, albedo in [0,1] and |g| < 1"));
            }
            path.media.push([[lo[0], lo[1], lo[2], g], [hi[0], hi[1], hi[2], 0.0],
                [t[0], t[1], t[2], 0.0], [t[0] * albedo[0], t[1] * albedo[1], t[2] * albedo[2], 0.0],
                [emit[0], emit[1], emit[2], 0.0]]);
        }
    }
    Ok(())
}

/// What one kind's derivation needs, computed once and in kind order.
struct KindPlan<'a, 't> {
    id: usize,
    t: usize,
    k: &'a KindDecl,
    mesh_kind: bool,
    /// The first row of each distinct shape, and each row's shape.
    shapes: Vec<usize>,
    shape_of: Vec<usize>,
    keys: Vec<Vec<&'t RowVal>>,
    surface: Option<&'a FnDecl>,
    trace: Option<&'a FnDecl>,
    step_scale: Option<f32>,
    domain: blake3::Hasher,
    /// Mesh kinds read their bounds from the asset and derive nothing.
    fixed: Option<Vec<lip::Derived>>,
    guard_key: Option<blake3::Hasher>,
}

fn mesh_asset_of<'m>(k: &KindDecl, meshes: &'m [gltf::Mesh], row: &[RowVal]) -> Res<&'m gltf::Mesh> {
    match row {
        [RowVal::X(_), RowVal::U(id), RowVal::U(0)] if (*id as usize) < meshes.len() => Ok(&meshes[*id as usize]),
        _ => Err(Diag::new(Code::E602, k.name.span, "invalid Mesh asset or LOD", "emit an imported mesh with LOD zero")),
    }
}

fn plan_kind<'a, 't>(
    p: &Prog<'a>, types: &mut Vec<Ty>, opts: &CompileOptions<'_>, tables: &'t Tables,
    luts: &[Lut], meshes: &[gltf::Mesh], id: usize, name: &str,
) -> Res<KindPlan<'a, 't>> {
    let CompileOptions { overrides, .. } = *opts;
    let k = p.kinds[name];
    let t = p.kind_index(name);
    let rows = &tables[t];
    if rows.is_empty() || rows.len() as u32 >= abi::MAX_ROWS {
        return Err(Diag::new(Code::E602, k.name.span, format!("kind `{name}` is drawn with {} rows", rows.len()), "emit 1 to 16,777,215 rows from the rows pass"));
    }
    let mesh_kind = name == "Mesh" && k.queries.is_empty();
    // Rows with the same geometry fields share a derivation across poses.
    let key_fields = if mesh_kind { vec![1] } else { geometry_fields(k) };
    let mut shapes: Vec<usize> = vec![];
    let mut shape_of = vec![0usize; rows.len()];
    let mut keys: Vec<Vec<&RowVal>> = vec![];
    for (r, row) in rows.iter().enumerate() {
        let key: Vec<&RowVal> = key_fields.iter().filter_map(|i| row.get(*i)).collect();
        shape_of[r] = match keys.iter().position(|q| *q == key) {
            Some(s) => s,
            None => {
                keys.push(key);
                shapes.push(r);
                keys.len() - 1
            }
        };
    }
    let surface = k.query(QueryKind::Surface);
    let trace = k.query(QueryKind::Trace);
    let step_scale = if k.proof == GeometryProof::Required {
        None
    } else {
        let mut scale: Option<f32> = None;
        for query in [surface, trace].into_iter().flatten() {
            for expr in emit::References::function(query).step_scales {
                let value = as_f32(stage0(p, types, expr, overrides)?, expr.span)?;
                if !value.is_finite() || value <= 0.0 || !value.recip().is_finite() {
                    return Err(Diag::new(Code::E602, expr.span, "invalid unproved step scale", "choose a positive finite scale with a finite reciprocal"));
                }
                if scale.is_some_and(|old| old != value) {
                    return Err(Diag::new(Code::E003, expr.span, "one geometry query has conflicting step scales", "use the same scale in each unproved block"));
                }
                scale = Some(value);
            }
        }
        scale
    };
    let mut domain = cache::kind_key(p, k, luts)?;
    if !p.bakes.is_empty() {
        let geometry = [surface, trace].into_iter().flatten().collect::<Vec<_>>();
        let (helpers, _) = emit::reachable(p, &geometry, &[]);
        let textures: std::collections::BTreeSet<_> = geometry.into_iter().chain(helpers)
            .flat_map(|f| emit::References::function(f).texture_names).collect();
        for name in textures {
            if let Some(bake) = p.bakes.get(name) {
                // A sampled field's range depends on the texel source and its format.
                domain.update(&cache::fingerprint(bake, bake.texel.name.span)?);
                domain.update(cache::certificate(p, &bake.texel)?.as_bytes());
            }
        }
    }
    let fixed = if mesh_kind {
        for row in rows { mesh_asset_of(k, meshes, row)?; }
        Some(shapes.iter().map(|&row| {
            let mesh = mesh_asset_of(k, meshes, &rows[row])?;
            // A planar triangle still has a nonempty conservative box for the culler.
            Ok(lip::Derived { lo: mesh.lo.map(|x| x.next_down().into()),
                hi: mesh.hi.map(|x| x.next_up().into()), l: 0., evals: 0 })
        }).collect::<Res<_>>()?)
    } else { None };
    // An immutable certificate may depend on this row and pure library functions.
    // Foreign rows, live params, sampled bakes and scheduled writers can change independently.
    let guard_key = trace.filter(|_| surface.is_none() && k.query(QueryKind::Visible).is_none()
        && k.proof == GeometryProof::Required).and_then(|f| {
        let (helpers, constants) = emit::reachable(p, &[f], &[]);
        let mutable = p.schedule.is_some_and(|s| s.passes.iter().any(|pass| pass.verb.name == "run_laws"))
            || constants.iter().any(|n| p.param_slot(n).is_some() || p.runtime_constant(n)
                || !loft_eval::guard::supports_expr(types, &p.consts[n.as_str()].value))
            || std::iter::once(f).chain(helpers).any(|f| {
                let refs = emit::References::function(f);
                f.raw.is_some() || !loft_eval::guard::supports_block(types, &f.body)
                    || !refs.texture_names.is_empty() || !refs.scans.is_empty()
                    || refs.names.iter().any(|&(n,_)| p.kinds.contains_key(n) || n == "time")
                    || refs.calls.iter().any(|c| !p.fns.contains_key(c.name.as_str()) && !matches!(c.name.as_str(),
                        "abs" | "clamp" | "cos" | "sin" | "sqrt" | "min" | "max" | "length" | "distance" | "dist"
                        | "dot" | "cross" | "f32" | "select" | "mix" | "step" | "smoothstep" | "coords" | "inv"
                        | "vec2" | "vec3" | "vec4" | "vec2f" | "vec3f" | "vec4f" | "rgb" | "saturate" | "normalize"))
            });
        if mutable { return None; }
        let mut key = domain.clone();
        key.update(include_bytes!("../../../loft-eval/src/guard.rs"));
        key.update(include_bytes!("../../../loft-eval/src/guard_range.rs"));
        for (name, value) in overrides {
            key.update(&(name.len() as u64).to_le_bytes());
            key.update(name.as_bytes());
            key.update(&value.to_le_bytes());
        }
        Some(key)
    });
    Ok(KindPlan { id, t, k, mesh_kind, shapes, shape_of, keys, surface, trace, step_scale, domain, fixed, guard_key })
}

struct ShapeBounds {
    proxy: lip::Derived,
    guard: Option<lip::Derived>,
}

fn derive_all(
    p: &Prog<'_>, types: &[Ty], opts: &CompileOptions<'_>, tables: &Tables, luts: &[Lut], plans: &mut [KindPlan<'_, '_>],
) -> Res<Vec<(Vec<ShapeBounds>, f64)>> {
    let shared: &[KindPlan<'_, '_>] = plans;
    let CompileOptions { overrides, cache, .. } = *opts;
    let jobs: Vec<(usize, usize)> = shared.iter().enumerate().filter(|(_, plan)| plan.fixed.is_none())
        .flat_map(|(i, plan)| (0..plan.shapes.len()).map(move |s| (i, s))).collect();
    let next = std::sync::atomic::AtomicUsize::new(0);
    let workers = std::thread::available_parallelism().map_or(1, |n| n.get()).min(jobs.len());
    // Workers claim jobs one at a time: derivation cost varies several-fold between shapes.
    let done: Vec<Vec<(usize, Res<ShapeBounds>, f64)>> = std::thread::scope(|sc| {
        let handles: Vec<_> = (0..workers).map(|_| {
            let (jobs, next) = (&jobs, &next);
            sc.spawn(move || {
                let mut iv: Eval<Iv> = Eval::new(p, types, (*tables).clone(), overrides);
                iv.luts = luts;
                let mut out = vec![];
                loop {
                    let j = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    let Some(&(i, s)) = jobs.get(j) else { break };
                    let plan = &shared[i];
                    let r = plan.shapes[s];
                    let began = std::time::Instant::now();
                    let derived = cache::derive(cache, &plan.domain, &plan.keys[s], plan.k.name.span, || match (plan.surface, plan.trace) {
                        (Some(f), _) => lip::derive_surface(&mut iv, f, plan.t, r, plan.k.name.span),
                        (None, Some(f)) if plan.step_scale.is_some() => iv.unproved_bounds(f, plan.t, r, plan.k.name.span),
                        (None, Some(f)) => lip::derive(&mut iv, f, plan.t, r, plan.k.name.span),
                        (None, None) => unreachable!("checked: trace or surface"),
                    });
                    let result = derived.and_then(|proxy| {
                        let guard = match (&plan.guard_key, plan.trace) {
                            (Some(key), Some(f)) if loft_eval::guard::stored_transforms_exact(&plan.keys[s]) => Some(cache::derive(cache, key, &plan.keys[s], plan.k.name.span, ||
                                loft_eval::guard::derive(&mut iv, f, plan.t, r, plan.k.name.span, &proxy, pose_field(plan.k).is_some()))?),
                            _ => None,
                        };
                        Ok(ShapeBounds { proxy, guard })
                    });
                    out.push((j, result, began.elapsed().as_secs_f64()));
                }
                out
            })
        }).collect();
        handles.into_iter().map(|h| h.join().expect("derive thread")).collect()
    });
    let mut results: Vec<Option<(Res<ShapeBounds>, f64)>> = (0..jobs.len()).map(|_| None).collect();
    for (j, r, seconds) in done.into_iter().flatten() { results[j] = Some((r, seconds)); }
    let mut results = results.into_iter();
    // The first failing job in kind and shape order reports, whatever order the workers finished in.
    // F4: one pool derives every kind's shapes, so no kind has a wall time of its own. Each job is timed, and a
    // kind's figure is its share of the wait: its jobs' seconds over the worker count. Cached shapes cost near 0.
    plans.iter_mut().map(|plan| match plan.fixed.take() {
        Some(fixed) => Ok((fixed.into_iter().map(|proxy| ShapeBounds { proxy, guard: None }).collect(), 0.)),
        None => {
            let mut seconds = 0.;
            let derived: Res<Vec<ShapeBounds>> = (0..plan.shapes.len()).map(|_| {
                let (r, s) = results.next().expect("one result per job").expect("job ran");
                seconds += s;
                r
            }).collect();
            derived.map(|d| (d, seconds / workers.max(1) as f64))
        }
    }).collect()
}

fn pack_kind<'a>(
    k: &KindDecl, id: usize, rows: &[Vec<RowVal>], mesh_kind: bool,
    mesh_asset: impl Fn(&[RowVal]) -> Res<&'a gltf::Mesh>, shapes: &[usize], shape_of: &[usize],
    derived: &[ShapeBounds], step_scale: Option<f32>, derive_s: f64, guard_words: std::ops::Range<usize>,
    instances: &mut Vec<Instance>,
) -> Res<(KindTable, KindReport)> {
    let name = &k.name.name;
    let surface = k.query(QueryKind::Surface);
    let pose = pose_field(k);
    let (mut words, mut lips, mut evals) = (vec![], vec![], 0u64);
    for d in derived {
        evals += u64::from(d.proxy.evals) + d.guard.as_ref().map_or(0, |g| u64::from(g.evals));
    }
    for (r, row) in rows.iter().enumerate() {
        let d = &derived[shape_of[r]].proxy;
        // World bound: the local bound's eight corners through the row's rigid pose.
        let (mut lo, mut hi) = ([f32::INFINITY; 3], [f32::NEG_INFINITY; 3]);
        let x = match pose.map(|i| &row[i]) {
            Some(RowVal::X(x)) => *x,
            _ => eval::Rigid::yaw([0.0; 3], 0.0),
        };
        for c in 0..8 {
            let q = [0, 1, 2].map(|i| if c >> i & 1 == 0 { d.lo[i] as f32 } else { d.hi[i] as f32 });
            let w = rot(&x, q);
            for i in 0..3 {
                lo[i] = lo[i].min(w[i] + x.t[i]);
                hi[i] = hi[i].max(w[i] + x.t[i]);
            }
        }
        pack(row, &mut words);
        words.extend(lo);
        words.extend(hi);
        // The marcher stores the reciprocal of its step multiplier in the row ABI.
        words.push(step_scale.map_or(d.l as f32, f32::recip));
        lips.push(d.l as f32);
        instances.push(Instance { lo, hi, kind: id as u32, row: r as u32 });
    }
    let draw = if mesh_kind {
        let mut vertices = 0;
        for row in rows {
            let n = u32::try_from(mesh_asset(row)?.triangles.len()).ok().and_then(|n| n.checked_mul(3))
                .ok_or_else(|| Diag::new(Code::E602, k.name.span, "mesh vertex count exceeds u32", "split the mesh"))?;
            vertices = vertices.max(n);
        }
        Draw::Mesh { vertices }
    } else { match (surface, k.query(QueryKind::Visible)) {
        (Some(s), _) => {
            let g = s.attrs.iter().find(|a| a.name.name == "grid").expect("checked grid");
            let n = |e: &Expr| match e.kind {
                ExprKind::Num { value, .. } => value as u32,
                _ => unreachable!("checked literal"),
            };
            Draw::Surface { nu: n(&g.args[0]), nv: n(&g.args[1]) }
        }
        (None, Some(_)) => Draw::Analytic,
        (None, None) => Draw::Traced,
    } };
    let report = KindReport {
        name: name.to_string(),
        proof: k.proof,
        rows: rows.len(),
        shapes: shapes.len(),
        l_min: lips.iter().copied().fold(f32::INFINITY, f32::min).into(),
        l_max: lips.iter().copied().fold(0.0, f32::max).into(),
        evals,
        cache_hits: derived.iter().filter(|d| d.proxy.evals == 0 && d.guard.as_ref().is_none_or(|g| g.evals == 0)).count(),
        guard_words,
        derive_s,
        draw,
    };
    let stride = (words.len() / rows.len()) as u32;
    let kind = KindTable { name: name.to_string(), module: id, stride, rows: words, n_rows: rows.len() as u32, lipschitz: lips, draw, reads: vec![] };
    Ok((kind, report))
}

fn pack_luts(
    meshes: &[gltf::Mesh], luts: &[Lut], tiled_world: bool,
) -> Res<(Vec<f32>, Vec<emit::LutRef>)> {
    let mut lut_refs = vec![];
    let asset_words = gltf::words(meshes);
    let mut lut_words = match tiled_world {
        false => asset_words,
        true => {
            let mut words = vec![0.0; 12];
            words.extend(asset_words);
            words
        }
    };
    let mut off = lut_words.len().div_ceil(12) as u32;
    for l in luts {
        lut_refs.push(emit::LutRef { name: l.name.clone(), offset: off, n: l.frames.len() as u32, lo: l.lo, hi: l.hi });
        off += l.frames.len() as u32;
    }
    lut_words.resize(lut_words.len().next_multiple_of(12), 0.);
    lut_words.extend(luts.iter().flat_map(|l| l.frames.iter().flatten().copied()));
    Ok((lut_words, lut_refs))
}
