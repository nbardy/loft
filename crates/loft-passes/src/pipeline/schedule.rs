use crate::{AuthoredCompute, CompileOptions, GPU_ORDER, Prog, arg, as_v3, check, emit, kind_args, shade_schedule};
use crate::frame_pass::FramePass;
use crate::eval::Tables;
use crate::program::{Orbit, Pass, StepClock, abi};
use crate::source::Graph;
use emit::paths::{PathTrace, WorldResources};
use emit::shade_tiles::{Lighting, Visibility};
use loft_eval::{as_f32, stage0};
use loft_ir::{Code, Diag, Res, Span};
use loft_parse::ast::*;
use std::collections::{BTreeMap, HashSet};

pub(super) struct Schedule<'a> {
    pub(super) decl: &'a ScheduleDecl,
    pub(super) rows_fns: Vec<&'a FnDecl>,
    pub(super) camera: Orbit,
    pub(super) sun: [f32; 3],
    pub(super) lighting: Lighting<'a>,
    pub(super) sky: &'a str,
    pub(super) steps: Vec<Step<'a>>,
    pub(super) drawn: Vec<&'a str>,
    casters: Vec<&'a str>,
    pub(super) shadow_size: u32,
    pub(super) shadow_clock: StepClock,
    pub(super) path: Option<PathTrace>,
    pub(super) tiled_world: Option<PathTrace>,
    pub(super) path_entry: Option<&'a str>,
    pub(super) paper: Option<&'a str>,
    pub(super) compute: AuthoredCompute,
    pub(super) ids: Vec<u32>,
}

/// A frame step: a compiler pass, or a pass the schedule wrote from a template (lowered once `edges` is known).
pub(super) enum Step<'a> {
    Stock(Pass),
    Frame(FramePass<'a>),
}

#[derive(Default)]
struct Draft<'a> {
    camera: Option<Orbit>,
    sun: Option<[f32; 3]>,
    light: Option<Lighting<'a>>,
    steps: Vec<Step<'a>>,
    drawn: Vec<&'a str>,
    casters: Vec<&'a str>,
    shadow_size: u32,
    shadow_clock: StepClock,
    path: Option<PathTrace>,
    path_entry: Option<&'a str>,
    paper: Option<&'a str>,
}

pub(super) fn resolve<'a>(
    p: &Prog<'a>, modules: &'a [Module], types: &mut Vec<Ty>, src: &Graph, opts: &CompileOptions<'_>,
) -> Res<Schedule<'a>> {
    let CompileOptions { overrides, features, aov, .. } = *opts;
    let sched = p.schedule.ok_or_else(|| Diag::new(Code::E520, Span::NONE, "the program has no `schedule`", "add `schedule Main { ... }` to the scene"))?;
    let mut rows_fns: Vec<&FnDecl> = modules.iter().flat_map(|m| &m.items)
        .filter_map(|item| if let Item::Build(f) = item { Some(f) } else { None }).collect();
    let mut draft = Draft::default();
    let has_shadow_pass = sched.passes.iter().any(|p| p.verb.name == "find_shadow_map");
    let mut emitted_passes = HashSet::new();
    let mut compute = AuthoredCompute { standalone: src.compute_only(), ..AuthoredCompute::default() };
    let mut compute_rows = BTreeMap::new();
    let mut available: HashSet<&str> = ["FrameUniform", "InputsBuf", "ParamsBuf", "RowTables", "LutTables", "BodyTables", "StateBuf", "PoseBuf"].into_iter().collect();
    available.extend(p.bakes.keys().copied());
    let mut labels = HashSet::new();
    for pd in &sched.passes {
        if let Some(resources) = &pd.resources {
            if pd.clock.is_some() {
                return Err(Diag::new(Code::E520, pd.label.span, "`on` is lowered for `find_shadow_map` only in this build",
                    "remove the clause; the pass runs every frame"));
            }
            if let Some(frame) = FramePass::classify(p, types, src, overrides, pd, resources)? {
                if let Some(read) = frame.reads.iter().find(|r| !available.contains(**r)) {
                    return Err(Diag::new(Code::E505, pd.label.span, format!("{read} read before it was written"), "schedule its producer before this pass"));
                }
                if !labels.insert(pd.label.name.as_str()) || p.kinds.contains_key(pd.label.name.as_str()) {
                    return Err(Diag::new(Code::E505, pd.label.span, format!("duplicate render pass {}", pd.label.name),
                        "give each frame pass its own label and schedule one producer per renderer resource"));
                }
                available.extend(frame.writes.iter().copied());
                draft.steps.push(Step::Frame(frame));
                continue;
            }
            if let Some(read) = resources.reads.iter().find(|r| !available.contains(r.name.as_str())) {
                return Err(Diag::new(Code::E505, read.span, format!("{} read before it was written", read.name), "schedule its producer before this pass"));
            }
            compute.lower(p, types, src, overrides, (pd, resources, &compute_rows), features)?;
            available.extend(resources.writes.iter().map(|r| r.name.as_str()));
            continue;
        }
        let verb = pd.verb.name.as_str();
        if GPU_ORDER.contains(&verb) && !emitted_passes.insert(verb) {
            return Err(Diag::new(Code::E505, pd.verb.span, format!("duplicate render pass {verb}"), "schedule one producer per renderer resource"));
        }
        if pd.clock.is_some() && verb != "find_shadow_map" {
            return Err(Diag::new(Code::E520, pd.verb.span, format!("`on` is lowered for `find_shadow_map` only in this build, not `{verb}`"),
                "remove the clause; the step runs every frame"));
        }
        match verb {
            "table" => {
                let (name, row) = compute.declare_table(p, types, overrides, pd)?;
                compute_rows.insert(name, row);
                available.insert(name);
            }
            v if p.fns.get(v).is_some_and(|f| f.comptime) => {
                if !pd.args.is_empty() || !p.fns[v].params.is_empty() {
                    return Err(Diag::new(Code::E405, pd.verb.span, "a rows pass calls a `comptime fn` with no arguments", "read stage-0 constants inside the fn"));
                }
                rows_fns.push(p.fns[v]);
            }
            _ => draft.lower_pass(p, modules, types, overrides, sched, pd)?,
        }
        let (reads, writes) = renderer_effects(verb, has_shadow_pass);
        if let Some(read) = reads.iter().find(|r| !available.contains(**r)) {
            return Err(Diag::new(Code::E505, pd.verb.span, format!("{read} read before it was written"), "schedule its producer before this pass"));
        }
        available.extend(writes);
    }
    let Draft { camera, sun, light, steps, drawn, casters, shadow_size, shadow_clock,
        path, path_entry, paper } = draft;
    let camera = camera.ok_or_else(|| Diag::new(Code::E520, sched.name.span, "no camera", "add `pass camera: orbit(at: ..., radius: ..., height: ..., period: ..., fov: ...);`"))?;
    // The frame's sky is the present pass's `sky` hole: World shading's edge rays call the same function.
    let sky = steps.iter().find_map(|step| match step {
        Step::Frame(frame) => frame.sky(p),
        Step::Stock(_) => None,
    });
    let (Some(light), Some(sky), Some(sun)) = (light, sky, sun) else {
        return Err(Diag::new(Code::E520, sched.name.span, "incomplete raster schedule",
            "schedule shade_tiles and a tonemap pass"));
    };
    if matches!(light, Lighting::World { .. }) && path.is_some()
        || matches!(light, Lighting::World { visibility: Visibility::SubpixelEdges | Visibility::Coverage, .. }
            | Lighting::Bsdf(_, Visibility::Coverage)) && paper.is_some()
    {
        return Err(Diag::new(Code::E520, sched.name.span, "conflicting raster and presentation recipes",
            "use one shade and film recipe for the raster schedule"));
    }
    let lighting = light;
    let tiled_world = (matches!(lighting, Lighting::World { .. }) && aov == emit::Aov::Beauty).then(|| emit::paths::PathTrace {
        integrator: emit::paths::Integrator::Reference, bounces: 0, lens: [0.0, 1.0],
        lamps: vec![], worlds: vec![], media: vec![],
    });
    Ok(Schedule {
        decl: sched, rows_fns, camera, sun, lighting, sky, steps, drawn, casters,
        shadow_size, shadow_clock, path, tiled_world, path_entry, paper, compute, ids: vec![],
    })
}

impl<'a> Draft<'a> {
    fn lower_pass(
        &mut self, p: &Prog<'a>, modules: &'a [Module], types: &mut Vec<Ty>,
        overrides: &[(String, f64)], sched: &ScheduleDecl, pd: &'a PassDecl,
    ) -> Res<()> {
        let verb = pd.verb.name.as_str();
        match verb {
            "run_laws" | "check" => {}, // Lowered to fixed-tick dispatches below.
            "orbit" => self.camera = Some(orbit(p, types, overrides, pd)?),
            "use_camera" => self.camera = Some(named_camera(p, modules, types, overrides, pd)?),
            "render_bundle" => {
                if !matches!(pd.args.first().map(|a| &a.value.kind), Some(ExprKind::Name(n)) if n == "Vis64") {
                    return Err(Diag::new(Code::E405, pd.verb.span, "the bundle draws into `Vis64` only", "write `render_bundle(Vis64) { draw_indirect(K); }`"));
                }
                for (call, args) in &pd.body {
                    if call.name != "draw_indirect" {
                        return Err(Diag::new(Code::E520, call.span, format!("`{}` in a render bundle", call.name), "write `draw_indirect(Kind, ...)`"));
                    }
                    let fake = PassDecl { label: pd.label.clone(), verb: call.clone(), args: args.iter().map(|a| Arg { name: None, value: clone_name(&a.value) }).collect(), body: vec![], resources: None, clock: None };
                    for k in kind_args(p, &fake)? {
                        self.drawn.push(k);
                    }
                }
                self.steps.push(Step::Stock(Pass::Vis((0..self.drawn.len() as u32).collect())));
            }
            "find_shadow_map" => {
                self.casters = kind_args(p, pd)?;
                let e = arg(pd, "size")?;
                self.shadow_size = as_f32(stage0(p, types, e, overrides)?, e.span)? as u32;
                if !(64..=8192).contains(&self.shadow_size) || (!sched.implicit && self.casters.is_empty()) {
                    return Err(Diag::new(Code::E520, pd.verb.span, "a shadow map needs casters and a size of 64 to 8192 texels", "write `find_shadow_map(Kind, ..., size: 1024u)`"));
                }
                self.shadow_clock = shadow_clock(pd, &self.casters)?;
                self.steps.push(Step::Stock(Pass::Shadow(vec![])));
            }
            "shade_tiles" => {
                self.light = Some(shade_schedule::lighting(p, types, pd, overrides)?);
                let s = arg(pd, "sun")?;
                self.sun = Some(as_v3(stage0(p, types, s, overrides)?, s.span)?);
                self.steps.push(Step::Stock(Pass::Shade(vec![])));
            }
            "screen_pass" => {
                let (entry, path) = screen_pass(p, types, overrides, pd)?;
                self.path_entry = Some(entry);
                if self.path.replace(path).is_some() {
                    return Err(Diag::new(Code::E505, pd.verb.span, "duplicate path pass", "schedule one path integrator"));
                }
            }
            // Not a pass: the present template reads it as the frame fact `paper`.
            "outline_and_paper" => { self.paper = Some(p.linked(pd.verb.span, "paper_grain")?); }
            other => {
                return Err(Diag::new(
                    Code::E520,
                    pd.verb.span,
                    format!("unknown pass `{other}`"),
                    format!("passes: a `comptime fn` (rows), orbit, {}", GPU_ORDER.join(", ")),
                ));
            }
        }
        Ok(())
    }
}

fn renderer_effects(verb: &str, has_shadow_pass: bool) -> (&'static [&'static str], &'static [&'static str]) {
    match verb {
        "orbit" | "use_camera" => (&["FrameUniform"], &["CameraSlot"]),
        "run_laws" => (&["StateBuf", "RowTables"], &["PoseBuf"]),
        "check" => (&["RowTables", "PoseBuf"], &["VerdictBuf"]),
        "render_bundle" => (&["IndirectBuf"], &["Vis64", "DepthBuf"]),
        "find_shadow_map" => (&["IndirectBuf"], &["ShadowMap"]),
        "shade_tiles" if has_shadow_pass => (&["Vis64", "DepthBuf", "TileLists", "ShadowMap"], &["ColorHdr"]),
        "shade_tiles" => (&["Vis64", "DepthBuf", "TileLists"], &["ColorHdr"]),
        "outline_and_paper" => (&["ColorHdr", "Vis64"], &["ColorHdr"]),
        "screen_pass" => (&["CameraSlot"], &["ColorHdr"]),
        _ => (&[], &[]),
    }
}

/// `find_shadow_map(...) on load | on dirty(...)`. Dirty names the step's casters (`Casters` is all of them).
fn shadow_clock(pd: &PassDecl, casters: &[&str]) -> Res<StepClock> {
    match &pd.clock {
        None => Ok(StepClock::Dirty),
        Some(BakeClock::Load) => Ok(StepClock::Load),
        Some(BakeClock::Dirty(names)) => {
            if let Some(n) = names.iter().find(|n| n.name != "Casters" && !casters.contains(&n.name.as_str())) {
                return Err(Diag::new(Code::E520, n.span, format!("the shadow map is dirty when its casters move; `{}` is not one", n.name),
                    "write `on dirty(Casters)` or name caster kinds"));
            }
            Ok(StepClock::Dirty)
        }
    }
}

fn orbit(p: &Prog<'_>, types: &mut Vec<Ty>, overrides: &[(String, f64)], pd: &PassDecl) -> Res<Orbit> {
    let f = |n: &str, types: &mut Vec<Ty>| -> Res<f32> {
        let e = arg(pd, n)?;
        as_f32(stage0(p, types, e, overrides)?, e.span)
    };
    let at = arg(pd, "at")?;
    Ok(Orbit {
        at: as_v3(stage0(p, types, at, overrides)?, at.span)?,
        radius: f("radius", types)?,
        height: if pd.args.iter().any(|a|a.name.as_ref().is_some_and(|n|n.name=="height")){f("height",types)?}else{0.5},
        period: if pd.args.iter().any(|a|a.name.as_ref().is_some_and(|n|n.name=="period")){f("period",types)?}else{24.0},
        fov: f("fov", types)?,
        fixed_eye: None,
    })
}

fn named_camera(
    p: &Prog<'_>, modules: &[Module], types: &mut Vec<Ty>, overrides: &[(String, f64)], pd: &PassDecl,
) -> Res<Orbit> {
    let [Arg { value: Expr { kind: ExprKind::Name(name), .. }, .. }] = pd.args.as_slice() else {
        return Err(Diag::new(Code::E520, pd.verb.span, "use_camera needs a declared camera", "write use_camera(CameraName)"));
    };
    let fields = &modules.iter().flat_map(|m| &m.items).find_map(|item| match item {
        Item::Camera(c) if c.name.name == *name => Some(c),
        _ => None,
    }).ok_or_else(|| Diag::new(Code::E520, pd.verb.span, format!("unknown camera {name}"), "declare the camera at module scope"))?.fields;
    let field = |names: &[&str]| fields.iter()
        .find(|(n, _)| names.contains(&n.name.as_str()))
        .map(|(_, e)| e)
        .ok_or_else(|| Diag::new(Code::E520, pd.verb.span, format!("camera needs {}", names[0]), "supply the camera field"));
    let f = |e: &Expr, types: &mut Vec<Ty>| as_f32(stage0(p, types, e, overrides)?, e.span);
    let point = |e: &Expr, types: &mut Vec<Ty>| as_v3(stage0(p, types, e, overrides)?, e.span);
    let at = point(field(&["at", "target"])?, types)?;
    let fov = f(field(&["fov"])?, types)?;
    Ok(match field(&["eye"]) {
        Ok(e) => Orbit { at, fov, fixed_eye: Some(point(e, types)?), radius: 0.0, height: 0.0, period: 1.0 },
        Err(_) => Orbit {
            at, fov, fixed_eye: None,
            radius: f(field(&["radius"])?, types)?,
            height: field(&["height"]).map_or(Ok(0.5), |e| f(e, types))?,
            period: field(&["period"]).map_or(Ok(24.0), |e| f(e, types))?,
        },
    })
}

fn screen_pass<'a>(
    p: &Prog<'a>, types: &mut Vec<Ty>, overrides: &[(String, f64)], pd: &'a PassDecl,
) -> Res<(&'a str, PathTrace)> {
    let Some(ExprKind::Name(entry)) = pd.args.first().map(|a| &a.value.kind) else {
        return Err(Diag::new(Code::E405, pd.verb.span, "screen pass needs a path function", "name a fn with signature (v3, v3, u, u) -> rgb"));
    };
    let f = p.fns.get(entry.as_str()).ok_or_else(|| Diag::new(Code::E011, pd.args[0].value.span,
        format!("unknown path function {entry}"), "name a declared fn"))?;
    let checker = check::Ck::new(p, types);
    let params = f.params.iter().map(|a| a.ty.as_ref().map(|t| checker.resolve(t, true)).transpose())
        .collect::<Res<Vec<_>>>()?;
    let rgb = Some(Ty::VecN(3));
    if params != [rgb.clone(), rgb.clone(), Some(Ty::U32), Some(Ty::U32)]
        || f.ret.as_ref().map(|t| checker.resolve(t, true)).transpose()? != rgb
    {
        return Err(Diag::new(Code::E003, pd.args[0].value.span, "invalid path function signature",
            "write fn sample(origin: v3, direction: v3, seed: u, bounces: u) -> rgb"));
    }
    let integrator = match pd.args.iter().find(|a| a.name.as_ref().is_some_and(|key| key.name == "spectral")).map(|a| &a.value.kind) {
        Some(ExprKind::Bool(true)) => emit::paths::Integrator::Spectral,
        Some(ExprKind::Bool(false)) | None => emit::paths::Integrator::Reference,
        Some(_) => return Err(Diag::new(Code::E003, pd.verb.span, "spectral requires a boolean", "use spectral: true or omit the option")),
    };
    let e = arg(pd, "bounces")?;
    let bounces = as_f32(stage0(p, types, e, overrides)?, e.span)?;
    if bounces.fract() != 0.0 || !(0.0..=64.0).contains(&bounces) {
        return Err(Diag::new(Code::E602, e.span, "invalid path bounce count", "use an integer from 0 to 64"));
    }
    let lens_value = |name: &str, fallback, types: &mut Vec<Ty>| -> Res<f32> {
        let Some(a) = pd.args.iter().find(|a| a.name.as_ref().is_some_and(|key| key.name == name)) else { return Ok(fallback); };
        let e = &a.value;
        as_f32(stage0(p, types, e, overrides)?, e.span)
    };
    let radius = lens_value("lens_radius", 0.0, types)?;
    let focus = lens_value("focus", 1.0, types)?;
    if !radius.is_finite() || radius < 0.0 || !focus.is_finite() || focus <= 0.0 {
        return Err(Diag::new(Code::E602, pd.verb.span, "invalid thin lens", "use a nonnegative lens radius and positive focus distance"));
    }
    let lens = [radius, focus];
    Ok((entry, PathTrace { integrator, bounces: bounces as u32, lens, lamps: vec![], worlds: vec![], media: vec![] }))
}

impl Schedule<'_> {
    pub(super) fn select_drawables(&mut self, p: &Prog<'_>, tables: &Tables) -> Res<()> {
        let sched = self.decl;
        let Self { drawn, casters, shadow_size, steps, path, tiled_world, .. } = self;
        let populated = |k: &&str| !tables[p.kind_index(k)].is_empty();
        // Compact all three lists before assigning GPU kind IDs. An omitted part has no slot.
        drawn.retain(populated);
        casters.retain(populated);
        if casters.is_empty() {
            *shadow_size = 0;
            steps.retain(|step| !matches!(step, Step::Stock(Pass::Shadow(_))));
        }
        let mut b = drawn.clone();
        b.sort_unstable();
        if b.windows(2).any(|pair| pair[0] == pair[1]) || drawn.len() > abi::MAX_KINDS {
            return Err(Diag::new(Code::E520, sched.name.span, format!("the bundle draws {drawn:?}"), "draw each drawable kind once (up to 32)"));
        }
        // The instance table holds every drawn kind's rows, so a frame pass that reads kinds lists them all.
        for step in steps.iter_mut() {
            let Step::Frame(frame) = step else { continue };
            if !frame.reads_kinds { continue; }
            frame.kinds.retain(populated);
            let mut a = frame.kinds.clone();
            a.sort_unstable();
            if a != b {
                return Err(Diag::new(Code::E520, frame.decl.label.span, format!("cull lists {:?} but the bundle draws {drawn:?}", frame.kinds),
                    "cull each drawable kind once (up to 32)"));
            }
        }
        let ids: Vec<u32> = (0..drawn.len() as u32).collect();
        // Shadow-only geometry has row and shader resources but no primary draw.
        for &caster in casters.iter() {
            if !drawn.contains(&caster) { drawn.push(caster); }
        }
        if drawn.len() > abi::MAX_KINDS {
            return Err(Diag::new(Code::E520, sched.name.span, "too many primary and shadow kinds", "use up to 32 kinds"));
        }
        if let Some(path) = path.as_mut().or(tiled_world.as_mut()) {
            path.worlds.push(WorldResources { kinds: (0..drawn.len() as u32).collect(), media: true });
            for members in p.worlds.values() {
                path.worlds.push(WorldResources {
                    kinds: members.iter().filter_map(|m| drawn.iter().position(|k| *k == m.name).map(|k| k as u32)).collect(),
                    media: members.iter().any(|m| m.name == "FieldMedium"),
                });
            }
        }
        let caster_ids = casters.iter().map(|c| {
            drawn.iter().position(|d| d == c).map(|i| i as u32).ok_or_else(|| Diag::new(Code::E520, sched.name.span, format!("shadow caster `{c}` is not drawn"), "cast shadows from drawn kinds"))
        }).collect::<Res<Vec<_>>>()?;
        for step in steps {
            match step {
                Step::Stock(Pass::Vis(k) | Pass::Shade(k)) => *k = ids.clone(),
                Step::Stock(Pass::Shadow(k)) => *k = caster_ids.clone(),
                Step::Stock(Pass::Authored(_) | Pass::Path(_)) | Step::Frame(_) => {}
            }
        }
        self.ids = ids;
        Ok(())
    }
}

/// A name expression, copied (bundle bodies are re-read as kind lists).
fn clone_name(e: &Expr) -> Expr {
    let kind = match &e.kind {
        ExprKind::Name(n) => ExprKind::Name(n.clone()),
        ExprKind::Unproved { .. } | ExprKind::FiniteDiff { .. } | ExprKind::If { .. } | ExprKind::Num { .. } | ExprKind::Bool(_) | ExprKind::String(_) | ExprKind::Member(..) | ExprKind::Index(..) | ExprKind::Call { .. } | ExprKind::Ctor { .. } | ExprKind::Binary(..) | ExprKind::Unary(..) => ExprKind::Bool(false),
    };
    Expr { id: e.id, span: e.span, kind }
}
