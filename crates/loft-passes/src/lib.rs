//! Layer 2 of the meta engine (plan 2026-11-20 §1): every pass from source to `Program`, one module per pass.
//!
//!   parse -> index -> check -> lower the schedule -> stage 0 (rows) -> derive (bounds, L) -> emit -> naga validate
//!
//! `compile` runs them all; `Compiled` keeps what the gates need afterwards (the CPU twin of every query).

pub mod check;
pub mod emit;
pub mod eval;
pub mod lip;
pub mod parse;

use eval::{Eval, RowVal, Tables, Val};
use lip::Iv;
use loft_ir::ast::*;
use loft_ir::program::{abi, Draw, Instance, KindTable, Orbit, Pass, Program, ShaderModule};
use loft_ir::{Code, Diag, Res, Sources, Span};
use std::collections::{BTreeMap, HashSet};
use std::time::Instant;

/// The program's items by name. Built from parsed modules; cheap to rebuild.
pub struct Prog<'a> {
    pub fns: BTreeMap<&'a str, &'a FnDecl>,
    pub kinds: BTreeMap<&'a str, &'a KindDecl>,
    /// Declaration order; a kind's index here is its stage-0 table.
    pub kind_order: Vec<&'a str>,
    pub consts: BTreeMap<&'a str, &'a ConstDecl>,
    pub spaces: HashSet<&'a str>,
    pub units: HashSet<&'a str>,
    pub schedule: Option<&'a ScheduleDecl>,
}

impl<'a> Prog<'a> {
    pub fn index(modules: &'a [Module]) -> Res<Prog<'a>> {
        let mut p = Prog {
            fns: BTreeMap::new(),
            kinds: BTreeMap::new(),
            kind_order: vec![],
            consts: BTreeMap::new(),
            spaces: HashSet::new(),
            units: HashSet::new(),
            schedule: None,
        };
        let mut seen: BTreeMap<&str, Span> = BTreeMap::new();
        let mut name = |n: &'a Ident| -> Res<()> {
            if let Some(prev) = seen.insert(&n.name, n.span) {
                return Err(Diag::new(Code::E002, n.span, format!("`{}` is declared twice", n.name), "rename one")
                    .note(prev, "first declared here"));
            }
            Ok(())
        };
        for item in modules.iter().flat_map(|m| &m.items) {
            match item {
                // Spaces are their own namespace: `space Blob;` beside `kind Blob` is the natural spelling.
                Item::Space(names) => {
                    for n in names {
                        if !p.spaces.insert(&n.name) {
                            return Err(Diag::new(Code::E002, n.span, format!("space `{}` is declared twice", n.name), "declare it once"));
                        }
                    }
                }
                Item::Unit(n) => {
                    name(n)?;
                    p.units.insert(&n.name);
                }
                Item::Const(c) => {
                    name(&c.name)?;
                    p.consts.insert(&c.name.name, c);
                }
                Item::Fn(f) => {
                    name(&f.name)?;
                    p.fns.insert(&f.name.name, f);
                }
                Item::Kind(k) => {
                    name(&k.name)?;
                    p.kinds.insert(&k.name.name, k);
                    p.kind_order.push(&k.name.name);
                }
                Item::Schedule(s) => {
                    if let Some(prev) = p.schedule {
                        return Err(Diag::new(Code::E501, s.name.span, "a program has one schedule", "remove one")
                            .note(prev.name.span, "first schedule here"));
                    }
                    p.schedule = Some(s);
                }
            }
        }
        Ok(p)
    }

    pub fn kind_index(&self, name: &str) -> usize {
        self.kind_order.iter().position(|k| *k == name).expect("checked kind")
    }
}

pub fn parse_all(src: &Sources) -> Res<Vec<Module>> {
    let mut next = 0;
    src.files.iter().enumerate().map(|(i, (_, text))| parse::parse(i as u16, text, &mut next)).collect()
}

/// Per drawn kind: rows and the derived L range; the evaluation count of the bound search.
pub struct KindReport {
    pub name: String,
    pub rows: usize,
    pub l_min: f64,
    pub l_max: f64,
    pub evals: u64,
    pub draw: Draw,
}

pub struct Report {
    pub kinds: Vec<KindReport>,
    /// Wall milliseconds per pass, in order.
    pub ms: Vec<(&'static str, f64)>,
}

/// What survives compilation for the gates: the source, types and stage-0 tables, so any query can run on the CPU.
pub struct Compiled {
    pub modules: Vec<Module>,
    pub types: Vec<Ty>,
    pub tables: Tables,
    /// Drawn kinds by kind id: (stage-0 table index, name).
    pub drawn: Vec<(usize, String)>,
}

/// One query evaluated on the CPU in f32 (the twin), at a point and a direction: (trace, visible or 0, material).
pub struct CpuQuery {
    pub trace: f32,
    pub visible: f32,
    pub material: [f32; 3],
}

impl Compiled {
    pub fn prog(&self) -> Prog<'_> {
        Prog::index(&self.modules).expect("indexed once already")
    }

    pub fn cpu_query(&self, kind_id: usize, row: usize, p: [f32; 3], rd: [f32; 3]) -> Res<CpuQuery> {
        let prog = self.prog();
        let (table, name) = &self.drawn[kind_id];
        let k = prog.kinds[name.as_str()];
        let mut ev: Eval<f32> = Eval::new(&prog, &self.types, self.tables.clone(), &[]);
        let pt = Val::V(p.to_vec(), 0.0);
        let len = (rd[0] * rd[0] + rd[1] * rd[1] + rd[2] * rd[2]).sqrt();
        let dir = Val::V(rd.iter().map(|x| x / len).collect(), 0.0);
        let q = |w| k.queries.iter().find(|(x, _)| *x == w).map(|(_, f)| f);
        let f32_of = |v: Val<f32>| match v {
            Val::S(s) => s,
            other => unreachable!("query returns f32, got {other:?}"),
        };
        let trace = f32_of(ev.call_fn(q(QueryKind::Trace).expect("checked"), vec![Val::Row(*table, row), pt.clone()], Span::default())?);
        let visible = match q(QueryKind::Visible) {
            Some(f) => f32_of(ev.call_fn(f, vec![Val::Row(*table, row), pt.clone(), dir.clone()], Span::default())?),
            None => 0.0,
        };
        let material = match ev.call_fn(q(QueryKind::Material).expect("checked"), vec![Val::Row(*table, row), pt, dir], Span::default())? {
            Val::V(c, _) => [c[0], c[1], c[2]],
            other => unreachable!("material returns vec3, got {other:?}"),
        };
        Ok(CpuQuery { trace, visible, material })
    }
}

fn arg<'e>(pd: &'e PassDecl, name: &str) -> Res<&'e Expr> {
    pd.args.iter().find(|a| a.name.as_ref().is_some_and(|n| n.name == name)).map(|a| &a.value).ok_or_else(|| {
        Diag::new(Code::E501, pd.verb.span, format!("`{}` needs `{name}: ...`", pd.verb.name), format!("add `{name}: ...`"))
    })
}

fn positional_name<'e>(pd: &'e PassDecl, i: usize) -> Res<&'e str> {
    match pd.args.iter().filter(|a| a.name.is_none()).nth(i).map(|a| &a.value.kind) {
        Some(ExprKind::Name(n)) => Ok(n),
        _ => Err(Diag::new(Code::E501, pd.verb.span, format!("`{}` needs a name as argument {}", pd.verb.name, i + 1), "see the Phase 0 schedule verbs")),
    }
}

fn fn_name_arg<'e>(p: &Prog, pd: &'e PassDecl, name: &str) -> Res<&'e str> {
    let e = arg(pd, name)?;
    match &e.kind {
        ExprKind::Name(n) if p.fns.contains_key(n.as_str()) => Ok(n),
        _ => Err(Diag::new(Code::E002, e.span, format!("`{name}:` names a library fn"), "write the fn's name, e.g. `sky: sky_color`")),
    }
}

/// Stage-0 value of a schedule argument.
fn stage0(p: &Prog, types: &mut Vec<Ty>, e: &Expr, overrides: &[(String, f64)]) -> Res<RowVal> {
    check::check_stage0_expr(p, types, e)?;
    let mut ev: Eval<f32> = Eval::new(p, types, vec![], overrides);
    eval::concrete(&ev.expr(e)?, e.span)
}

fn as_f32(v: RowVal, span: Span) -> Res<f32> {
    match v {
        RowVal::F(x) => Ok(x),
        other => Err(Diag::new(Code::E003, span, format!("expected a number, found {other:?}"), "pass an f32")),
    }
}
fn as_v3(v: RowVal, span: Span) -> Res<[f32; 3]> {
    match v {
        RowVal::V(c) if c.len() == 3 => Ok([c[0], c[1], c[2]]),
        other => Err(Diag::new(Code::E003, span, format!("expected a vec3, found {other:?}"), "pass vec3(x, y, z) or Pt[S](...)")),
    }
}

/// Words of one row in the GPU table: the author's fields in order (Xform: rotation column-major, then t).
fn pack(row: &[RowVal], out: &mut Vec<f32>) {
    for v in row {
        match v {
            RowVal::F(x) => out.push(*x),
            RowVal::V(c) => out.extend(c),
            RowVal::U(u) => out.push(f32::from_bits(*u)),
            RowVal::I(i) => out.push(f32::from_bits(*i as u32)),
            RowVal::B(b) => out.push(if *b { 1.0 } else { 0.0 }),
            RowVal::X(x) => {
                for j in 0..3 {
                    for i in 0..3 {
                        out.push(x.r[i][j]);
                    }
                }
                out.extend(x.t);
            }
        }
    }
}

fn naga_module(name: &str, wgsl: String) -> Res<ShaderModule> {
    let internal = |e: String| {
        Diag::new(Code::E701, Span::NONE, format!("generated WGSL for `{name}` is invalid:\n{e}"), "run `loft show` to read the module; fix the library fn or the emitter")
    };
    let module = naga::front::wgsl::parse_str(&wgsl).map_err(|e| internal(e.emit_to_string(&wgsl)))?;
    naga::valid::Validator::new(naga::valid::ValidationFlags::all(), naga::valid::Capabilities::default())
        .validate(&module)
        .map_err(|e| internal(e.emit_to_string(&wgsl)))?;
    Ok(ShaderModule { name: name.into(), wgsl, naga: module })
}

pub const GPU_ORDER: [&str; 5] = ["build_indirect", "render_bundle", "tile_bin_kinds", "dispatch_material_tiles", "tonemap_aces"];

/// Front end only: parse, index, check. For `loft check` and the error tests.
pub fn front(src: &Sources) -> Res<(Vec<Module>, Vec<Ty>)> {
    let modules = parse_all(src)?;
    let mut types = vec![];
    {
        let p = Prog::index(&modules)?;
        check::check(&p, &mut types)?;
    }
    Ok((modules, types))
}

pub fn compile(src: &Sources, overrides: &[(String, f64)]) -> Res<(Program, Report, Compiled)> {
    let mut ms = vec![];
    let mut clock = Instant::now();
    let mut lap = |name: &'static str, ms: &mut Vec<(&'static str, f64)>| {
        ms.push((name, clock.elapsed().as_secs_f64() * 1e3));
        clock = Instant::now();
    };
    let modules = parse_all(src)?;
    lap("parse", &mut ms);
    let p = Prog::index(&modules)?;
    let mut types = vec![];
    check::check(&p, &mut types)?;
    for (name, _) in overrides {
        match p.consts.get(name.as_str()) {
            Some(c) if c.comptime => {}
            _ => {
                return Err(Diag::new(Code::E002, Span::NONE, format!("`--set {name}`: no `comptime const {name}`"), "override a `comptime const` declared in the scene"));
            }
        }
    }
    lap("check", &mut ms);

    // The schedule: stage-0 passes, the camera, and the five GPU passes in dependency order.
    let sched = p.schedule.ok_or_else(|| Diag::new(Code::E501, Span::NONE, "the program has no `schedule`", "add `schedule Main { ... }` to the scene"))?;
    let mut rows_fns = vec![];
    let (mut camera, mut sun, mut light, mut sky) = (None, None, None, None);
    let (mut passes, mut verbs) = (vec![], vec![]);
    let mut drawn: Vec<&str> = vec![];
    let mut culled: Vec<&str> = vec![];
    for pd in &sched.passes {
        let verb = pd.verb.name.as_str();
        match verb {
            v if p.fns.get(v).is_some_and(|f| f.comptime) => {
                if !pd.args.is_empty() || !p.fns[v].params.is_empty() {
                    return Err(Diag::new(Code::E405, pd.verb.span, "a rows pass calls a `comptime fn` with no arguments", "read stage-0 constants inside the fn"));
                }
                rows_fns.push(p.fns[v]);
            }
            "orbit" => {
                let f = |n: &str, types: &mut Vec<Ty>| -> Res<f32> {
                    let e = arg(pd, n)?;
                    as_f32(stage0(&p, types, e, overrides)?, e.span)
                };
                let at = arg(pd, "at")?;
                camera = Some(Orbit {
                    at: as_v3(stage0(&p, &mut types, at, overrides)?, at.span)?,
                    radius: f("radius", &mut types)?,
                    height: f("height", &mut types)?,
                    period: f("period", &mut types)?,
                    fov: f("fov", &mut types)?,
                });
            }
            "build_indirect" => {
                culled = (0..pd.args.len()).map(|i| positional_name(pd, i)).collect::<Res<_>>()?;
                passes.push(Pass::Cull);
                verbs.push(pd);
            }
            "render_bundle" => {
                if positional_name(pd, 0)? != "Vis64" {
                    return Err(Diag::new(Code::E405, pd.verb.span, "Phase 0 draws into `Vis64` only", "write `render_bundle(Vis64) { draw_indirect(K); }`"));
                }
                for (call, args) in &pd.body {
                    if call.name != "draw_indirect" {
                        return Err(Diag::new(Code::E501, call.span, format!("`{}` in a render bundle", call.name), "write `draw_indirect(Kind, ...)`"));
                    }
                    for a in args {
                        let ExprKind::Name(n) = &a.value.kind else {
                            return Err(Diag::new(Code::E501, a.value.span, "draw_indirect takes kind names", "write the kind's name"));
                        };
                        let Some(k) = p.kinds.get(n.as_str()) else {
                            return Err(Diag::new(Code::E002, a.value.span, format!("unknown kind `{n}`"), "declare the kind"));
                        };
                        if let Some(prm) = k.params.first() {
                            return Err(Diag::new(Code::E405, prm.span, "Phase 0 does not lower kind variants", "move the parameter into the row for now"));
                        }
                        drawn.push(&k.name.name);
                    }
                }
                passes.push(Pass::Vis((0..drawn.len() as u32).collect()));
                verbs.push(pd);
            }
            "tile_bin_kinds" => {
                let tile = arg(pd, "tile")?;
                if as_f32(stage0(&p, &mut types, tile, overrides).map(|v| match v {
                    RowVal::U(u) => RowVal::F(u as f32),
                    RowVal::I(i) => RowVal::F(i as f32),
                    other => other,
                })?, tile.span)? != abi::TILE as f32
                {
                    return Err(Diag::new(Code::E405, tile.span, "Phase 0 classifies 8x8 tiles", "write `tile: 8u`"));
                }
                passes.push(Pass::Classify);
                verbs.push(pd);
            }
            "dispatch_material_tiles" => {
                light = Some(fn_name_arg(&p, pd, "light")?);
                let s = arg(pd, "sun")?;
                sun = Some(as_v3(stage0(&p, &mut types, s, overrides)?, s.span)?);
                passes.push(Pass::Shade(vec![]));
                verbs.push(pd);
            }
            "tonemap_aces" => {
                sky = Some(fn_name_arg(&p, pd, "sky")?);
                passes.push(Pass::Present);
                verbs.push(pd);
            }
            other => {
                return Err(Diag::new(
                    Code::E501,
                    pd.verb.span,
                    format!("unknown pass `{other}`"),
                    format!("Phase 0 passes: a `comptime fn` (rows), orbit, {}", GPU_ORDER.join(", ")),
                ));
            }
        }
    }
    let order: Vec<&str> = verbs.iter().map(|v| v.verb.name.as_str()).collect();
    if order != GPU_ORDER {
        return Err(Diag::new(Code::E501, sched.name.span, format!("GPU passes {order:?} out of dependency order"), format!("write each once, in order: {}", GPU_ORDER.join(", "))));
    }
    let mut a = culled.clone();
    let mut b = drawn.clone();
    a.sort_unstable();
    b.sort_unstable();
    if a != b || drawn.is_empty() || drawn.len() > abi::MAX_KINDS {
        return Err(Diag::new(Code::E501, sched.name.span, format!("cull lists {culled:?} but the bundle draws {drawn:?}"), "cull exactly the kinds the bundle draws (1 to 32)"));
    }
    let ids: Vec<u32> = (0..drawn.len() as u32).collect();
    for pass in &mut passes {
        if let Pass::Shade(k) = pass {
            *k = ids.clone();
        }
    }
    let camera = camera.ok_or_else(|| Diag::new(Code::E501, sched.name.span, "no camera", "add `pass camera: orbit(at: ..., radius: ..., height: ..., period: ..., fov: ...);`"))?;
    let (light, sky, sun) = (light.expect("checked order"), sky.expect("checked order"), sun.expect("checked order"));
    lap("schedule", &mut ms);

    // Stage 0.
    let mut ev: Eval<f32> = Eval::new(&p, &types, vec![vec![]; p.kind_order.len()], overrides);
    for f in &rows_fns {
        ev.call_fn(f, vec![], f.name.span)?;
    }
    let tables = ev.tables;
    lap("stage0", &mut ms);

    // Derived bounds and L, per row of every drawn kind.
    let mut kinds = vec![];
    let mut instances = vec![];
    let mut reports = vec![];
    for (id, name) in drawn.iter().enumerate() {
        let k = p.kinds[*name];
        let t = p.kind_index(name);
        let trace = k.queries.iter().find(|(q, _)| *q == QueryKind::Trace).map(|(_, f)| f).expect("checked");
        let rows = &tables[t];
        if rows.is_empty() || rows.len() as u32 >= abi::MAX_ROWS {
            return Err(Diag::new(Code::E601, k.name.span, format!("kind `{name}` is drawn with {} rows", rows.len()), "emit 1 to 16,777,215 rows from the rows pass"));
        }
        // Rows are independent: derive them on every core. Results are per row, so the order of work cannot matter.
        let threads = std::thread::available_parallelism().map_or(1, |n| n.get()).min(rows.len());
        let chunk = rows.len().div_ceil(threads);
        let derived: Vec<lip::Derived> = std::thread::scope(|sc| {
            let jobs: Vec<_> = (0..threads)
                .map(|j| {
                    let (p, types, tables) = (&p, &types, &tables);
                    sc.spawn(move || {
                        let mut iv: Eval<Iv> = Eval::new(p, types, tables.clone(), overrides);
                        (j * chunk..((j + 1) * chunk).min(tables[t].len()))
                            .map(|r| lip::derive(&mut iv, trace, t, r, k.name.span))
                            .collect::<Res<Vec<_>>>()
                    })
                })
                .collect();
            jobs.into_iter().map(|h| h.join().expect("derive thread")).collect::<Res<Vec<Vec<_>>>>().map(|v| v.into_iter().flatten().collect())
        })?;
        let (mut words, mut lips, mut evals) = (vec![], vec![], 0u64);
        for (r, (row, d)) in rows.iter().zip(&derived).enumerate() {
            pack(row, &mut words);
            let (lo, hi) = (d.lo.map(|x| x as f32), d.hi.map(|x| x as f32));
            words.extend(lo);
            words.extend(hi);
            words.push(d.l as f32);
            lips.push(d.l as f32);
            evals += u64::from(d.evals);
            instances.push(Instance { lo, hi, kind: id as u32, row: r as u32 });
        }
        let draw = if k.queries.iter().any(|(q, _)| *q == QueryKind::Visible) { Draw::Analytic } else { Draw::Traced };
        reports.push(KindReport {
            name: name.to_string(),
            rows: rows.len(),
            l_min: lips.iter().copied().fold(f32::INFINITY, f32::min).into(),
            l_max: lips.iter().copied().fold(0.0, f32::max).into(),
            evals,
            draw,
        });
        let stride = (words.len() / rows.len()) as u32;
        kinds.push(KindTable { name: name.to_string(), module: id, stride, rows: words, n_rows: rows.len() as u32, lipschitz: lips, draw });
    }
    lap("derive", &mut ms);

    let mut texts = vec![];
    for name in &drawn {
        texts.push((name.to_string(), emit::kind_module(&p, &types, p.kinds[*name], light)));
    }
    texts.push(("cull".into(), emit::cull_module()));
    texts.push(("classify".into(), emit::classify_module()));
    texts.push(("present".into(), emit::present_module(&p, &types, sky)));
    lap("emit", &mut ms);
    let shader_modules = texts.into_iter().map(|(n, w)| naga_module(&n, w)).collect::<Res<Vec<_>>>()?;
    lap("naga", &mut ms);

    let n = drawn.len();
    let program = Program {
        modules: shader_modules,
        kinds,
        instances,
        camera,
        sun,
        passes,
        cull: n,
        classify: n + 1,
        present: n + 2,
    };
    let drawn_owned = drawn.iter().map(|d| (p.kind_index(d), d.to_string())).collect();
    drop(p);
    Ok((program, Report { kinds: reports, ms }, Compiled { modules, types, tables, drawn: drawn_owned }))
}
