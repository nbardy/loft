//! Loft's compiler core, and its program driver (lean Rust plan §1.10, §8.2: the integrator's file).
//!
//! `run` takes one job (wire §3.2) and how to build it, and answers with a report, plus the bundle for a Scene. A Scene
//! goes decode → kernel plan → schedule → layout → emit (Naga only when `Build` asks) → `Bundle` V1 → bytes and hash.
//!
//! What a Scene job can say today bounds what its bundle holds (driver note, `agent_notes/2026-11-19_lean_rust_driver.md`):
//! - the program roots are the expressions the TS emitter handed to `emitWgsl`, with their free variables (u, v, p).
//!   Each root becomes one exported function `loft_root{k}` whose parameters are those variables, in name order;
//! - a `Kernel` decl carries its table's fill (wire 2): a Map or Scan body is planned (a Const one runs on the tape at
//!   compile, the rest become one compute module each, `fill`), supplied numbers become section bytes, and a
//!   recurrence's tables, which the wire does not carry yet, are `Init::Zero`, counted as `unfilled`;
//! - Draw, Pass, Medium and Camera decls (wire 2) are lowered by `draw` into the bundle's draws, media and passes;
//!   each camera's parts are exported from the root module as `loft_camera{k}_{part}`, and its lens entry
//!   `loft_lens{k}` (bundle `Camera`) writes them for the host, which builds the matrices on the GPU.
//!   No corpus scene records them yet (the TS library's hit, place and paint functions are lane 3.1's TS half).
//!
//! Check and Solve jobs run their decls (`analyses`): Solve solves every Solve decl and reports each hold
//! `Proof::Unproved` (the default build: proofs take seconds); Check decides every Check decl and solves and proves
//! every Solve decl (`loft check`). An Evaluate job is `Internal::Unbuilt`.
mod analyses;
pub mod cache;
pub mod check;
pub mod draw;
pub mod emit;
pub mod fill;
pub mod grad;
pub mod interval;
pub mod kernel;
pub mod layout;
pub mod prim;
pub mod rate;
pub mod schedule;
pub mod solve;
pub mod tape;

pub use analyses::{CheckReport, SolveReport};
use loft_ir::bundle::{self, Bundle, F32, Format, InputDecl, Module, Origin, ParamDecl, Shape};
use loft_ir::error::{Internal, Limits, Meter, SceneFault};
use loft_ir::graph::{self as ir, Fill, Graph, GraphFn, Kernel, Op, Program, Role, Ty};
pub use loft_ir::identity::Precision;
use loft_ir::{Error, frame, wire};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::time::Instant;

/// What a job did, for the caller's report (src/core/native.ts `Report`).
#[derive(Serialize)]
pub struct Report {
    pub program: Program,
    pub nodes: usize,
    pub decls: usize,
    pub scene: Option<Scene>,
    /// A Check job's checks, in decl order (empty for other programs).
    pub checks: Vec<CheckReport>,
    /// A Solve or Check job's solves, in decl order (empty for a Scene).
    pub solves: Vec<SolveReport>,
    /// Wall time per phase, ns: Rust's own time, so the caller can tell the crossing from the work.
    pub phases: Phases,
}

#[derive(Serialize)]
pub struct Scene {
    /// Distinct program roots, each one exported function.
    pub roots: usize,
    /// Graph functions written once (shared by text hash) across the module.
    pub functions: usize,
    pub wgsl_bytes: usize,
    pub resources: usize,
    /// Table resources nothing fills (a recurrence's tables: the wire does not carry recurrences yet): `Init::Zero`.
    pub unfilled: usize,
    /// GPU fill kernels (each its own module), after Const fills moved to compile.
    pub kernels: usize,
    pub storage_buffers: usize,
    pub dispatches: usize,
    /// Always 0 until the wire pins Draw/Pass decls: the bundle cannot be drawn.
    pub draws: usize,
    pub bundle_bytes: usize,
    /// sha256 of the bundle file, hex: `Identity::build`.
    pub build: String,
    /// Whether the module also validates under `alias lf = f16;`, when `Build::precision` asked for `Mixed`. A `Mixed`
    /// load of a module that does not is refused by the host's own Naga check (loft-gpu `validate`).
    pub mixed: Mixed,
}

#[derive(Serialize)]
pub enum Mixed {
    Valid,
    /// Naga's refusal under f16, as `Error`'s text (a literal past f16's 65504 is the case met in the corpus).
    Refused(String),
    /// The build asked for f32, so no f16 check ran (`--precision mixed` asks for it).
    NotAsked,
}

/// How to build a job: what the caller asked for, classified once at the boundary (loft-node).
#[derive(Clone, Copy, Debug)]
pub struct Build {
    pub naga: Naga,
    /// `Mixed` adds the f16 check (`Scene::mixed`); the module text is the same either way (one `lf` alias).
    pub precision: Precision,
}

/// Where a Scene's WGSL is validated. Owl 2026-11-19: Naga at emit was 33 of 58 ms of Rust, and the host validates the
/// same text again when it creates the module, so a build validates once, at load, unless it asks for more.
#[derive(Clone, Copy, Debug)]
pub enum Naga {
    /// Naga at emit (`emit::check`): a refusal is `Scene { Wgsl }` at the author's line. `loft check` and the lab tools.
    Emit,
    /// No Naga at compile: the host's module creation at load is the one validation (loft-gpu `validate`, then wgpu).
    /// A bad module is then refused at load, without the author's line. The default build.
    Load,
}

#[derive(Serialize, Default)]
pub struct Phases {
    pub decode: u64,
    pub plan: u64,
    /// Emission, plus Naga as f32 under `Naga::Emit`.
    pub emit: u64,
    /// Naga under `alias lf = f16;` (lane 2.2's both-ways check), when `Precision::Mixed` asked for it; else 0.
    pub f16: u64,
    pub bundle: u64,
    /// Solving (Solve and Check jobs), and proving (checks, and a Check job's solve holds).
    pub solve: u64,
    pub prove: u64,
}

/// One job. For a Scene, the bundle file's bytes come back beside the report; the caller writes them.
pub fn run(job: &[u8], limits: Limits, build: Build, compiler: &str) -> Result<(Report, Option<Vec<u8>>), Error> {
    let mut phases = Phases::default();
    let t = Instant::now();
    let job = wire::decode(job)?;
    phases.decode = ns(t);
    let g = job.graph;
    limits.spend(Meter::Nodes, g.nodes.len() as u64)?;
    let decls = [
        g.fns.len(),
        g.raws.len(),
        g.loops.len(),
        g.kernels.len(),
        g.params.len(),
        g.inputs.len(),
        g.draws.len(),
        g.passes.len(),
        g.media.len(),
        g.cameras.len(),
        g.checks.len(),
        g.solves.len(),
    ];
    let (nodes, decls) = (g.nodes.len(), decls.iter().sum());
    match job.program {
        Program::Scene => {
            let (scene, bytes) = scene(g, build, compiler, &mut phases)?;
            let report = Report {
                program: job.program,
                nodes,
                decls,
                scene: Some(scene),
                checks: vec![],
                solves: vec![],
                phases,
            };
            Ok((report, Some(bytes)))
        }
        Program::Solve => {
            let solves = analyses::solves(&g, limits, analyses::Holds::Unproved, &mut phases)?;
            Ok((Report { program: job.program, nodes, decls, scene: None, checks: vec![], solves, phases }, None))
        }
        Program::Check => {
            let t = Instant::now();
            let checks = analyses::checks(&g, limits)?;
            phases.prove = ns(t);
            let solves = analyses::solves(&g, limits, analyses::Holds::Prove, &mut phases)?;
            Ok((Report { program: job.program, nodes, decls, scene: None, checks, solves, phases }, None))
        }
        Program::Evaluate => Err(Error::Internal(Internal::Unbuilt(job.program))),
    }
}

fn ns(t: Instant) -> u64 {
    t.elapsed().as_nanos() as u64
}

fn unsupported(what: String) -> Error {
    Error::Scene { site: None, fault: SceneFault::Unsupported { what: what.into() } }
}

/// What the wire carries and no driver phase lowers yet (wire 3, agent_notes/2026-11-19_lean_rust_wire3.md): an author
/// compute pass, an offscreen pass and its table camera. Refused here, by name, before any phase reads them.
fn unlowered(g: &Graph) -> Result<(), Error> {
    if let Some(Kernel::Compute(c)) = g.kernels.iter().find(|k| matches!(k, Kernel::Compute(_))) {
        return Err(unsupported(format!("the author compute pass `{}`: on the wire, not yet lowered", g.sym(c.name))));
    }
    if let Some(ir::Target::Texture { name, .. }) =
        g.passes.iter().map(|p| p.target).find(|t| matches!(t, ir::Target::Texture { .. }))
    {
        return Err(unsupported(format!("the offscreen pass `{}`: on the wire, not yet lowered", g.sym(name))));
    }
    match g.cameras.iter().find(|c| matches!(c.lens, ir::Lens::Table { .. })) {
        Some(c) => Err(unsupported(format!("the table camera `{}`: on the wire, not yet lowered", g.sym(c.name)))),
        None => Ok(()),
    }
}

fn scene(mut g: Graph, build: Build, compiler: &str, phases: &mut Phases) -> Result<(Scene, Vec<u8>), Error> {
    unlowered(&g)?;
    // Plan: one resource per Kernel decl, in decl order, so resource k is kernel k (emit's Slots, Fetch's binding).
    let t = Instant::now();
    let sources = g
        .kernels
        .iter()
        .map(|k| match k {
            Kernel::Table { lanes, entries, fill, .. } => {
                let lanes = u16::try_from(*lanes)
                    .map_err(|_| unsupported(format!("a table of {lanes} lanes; a row holds at most {}", u16::MAX)))?;
                Ok(match fill {
                    Fill::Data { at } => kernel::Source::Data { rows: *entries, lanes, at: *at },
                    Fill::Map(_) | Fill::Scan(_) | Fill::Recurrence(_) => {
                        kernel::Source::Made(Shape::Rows { rows: *entries, lanes })
                    }
                })
            }
            Kernel::Rows { at, len, .. } => Ok(kernel::Source::Curve { at: *at, len: *len }),
            Kernel::Compute(_) => unreachable!("`unlowered` refused compute kernels"),
        })
        .collect::<Result<Vec<_>, Error>>()?;
    let decls = fill_decls(&g);
    let mut plan = kernel::plan(&g, &sources, &decls)?;
    drop_lifted(&mut plan, sources.len());
    let dispatch_list = schedule::schedule(&plan, &decls)?;
    let dispatches = dispatch_list.len();
    let order: Vec<usize> = dispatch_list.iter().flat_map(|d| d.iter().map(|&k| k as usize)).collect();
    let lay = layout::with_outputs(&plan, &g.row_steps.iter().map(|(s, _)| s.output).collect::<Vec<_>>())?;
    phases.plan = ns(t);

    // Emit: each distinct root as `r{k}(free variables in name order)`, as a graph function appended to the job's.
    let t = Instant::now();
    let mut roots: Vec<u32> = Vec::with_capacity(g.roots.len());
    for &r in g.roots.iter() {
        if !roots.contains(&r) {
            roots.push(r);
        }
    }
    // `loft_` is the reserved prefix (draw templates); `r{k}` would meet emit's storage bindings `r{res}`.
    let mut names: Vec<String> = (0..roots.len()).map(|k| format!("loft_root{k}")).collect();
    let program_roots = roots.len();
    // Each camera's parts, exported beside the roots, and its lens entry: the host evaluates it every frame and builds
    // the matrices from the lens on the GPU (plan §1.5, one camera builder; record in loft_ir::frame::LENS).
    let mut lens_glue = String::new();
    for (k, c) in g.cameras.iter().enumerate() {
        // The lens record: row 0 (kind, fovy, 0, 0), then eye, target, up for a look, or the four columns of a matrix.
        let (head, parts): (String, [(&str, u32); 4]) = match c.lens {
            ir::Lens::Look { eye, target, up, fovy } => (
                format!("vec4<f32>({:?}, f32(loft_camera{k}_fovy()), 0.0, 0.0)", frame::LOOK),
                [("fovy", fovy), ("eye", eye), ("target", target), ("up", up)],
            ),
            ir::Lens::Matrix { cols } => (
                format!("vec4<f32>({:?}, 0.0, 0.0, 0.0)", frame::MATRIX),
                [("c0", cols[0]), ("c1", cols[1]), ("c2", cols[2]), ("c3", cols[3])],
            ),
            ir::Lens::Table { .. } => unreachable!("`unlowered` refused table cameras"),
        };
        let (mut body, mut at) = (format!("  loft_lens[0] = {head};\n"), 1);
        for (part, node) in parts {
            if let Some(&v) = free_vars(&g, node).first() {
                let Op::Var(var) = g.nodes[v as usize].op else { unreachable!("free_vars returns Var nodes") };
                let var = g.sym(var);
                return Err(unsupported(format!(
                    "camera \"{}\"'s {part} reads the free variable {var}; a camera reads only time, params and inputs",
                    g.sym(c.name)
                )));
            }
            roots.push(node);
            names.push(format!("loft_camera{k}_{part}"));
            let row = match g.nodes[node as usize].ty {
                Ty::F32 => continue, // fovy: in row 0
                Ty::V3 => format!("vec4<f32>(vec3<f32>(loft_camera{k}_{part}()), 0.0)"),
                Ty::V4 => format!("vec4<f32>(loft_camera{k}_{part}())"),
                Ty::V2 | Ty::Bool => unreachable!("the wire types a camera's parts (loft-ir decode)"),
            };
            body.push_str(&format!("  loft_lens[{at}] = {row};\n"));
            at += 1;
        }
        lens_glue.push_str(&format!("@compute @workgroup_size(1) fn loft_lens{k}() {{\n{body}}}\n"));
    }
    if !lens_glue.is_empty() {
        lens_glue.insert_str(
            0,
            &format!("@group(2) @binding(0) var<storage, read_write> loft_lens: array<vec4<f32>, {}>;\n", frame::LENS),
        );
    }
    let first = g.fns.len() as u32;
    let wrappers: Vec<GraphFn> = roots
        .iter()
        .map(|&r| GraphFn { name: 0, ret: g.nodes[r as usize].ty, binders: free_vars(&g, r), body: r })
        .collect();
    g.fns = g.fns.iter().cloned().chain(wrappers).collect();
    let exports: Vec<(&str, u32)> = names.iter().enumerate().map(|(k, n)| (n.as_str(), first + k as u32)).collect();
    // Resource k is kernel decl k (plan above); layout put each in a pack.
    let row = |k: usize| match (lay.at[k], &plan.resources[k].shape) {
        (layout::Place::Rows { pack, offset }, Shape::Rows { rows, lanes }) => {
            emit::Row { res: k as u32, pack, offset, rows: *rows, lanes: u32::from(*lanes) }
        }
        (layout::Place::Rows { .. } | layout::Place::Bound, _) => {
            unreachable!("kernel::plan makes every kernel decl's resource rows, and layout packs every rows resource")
        }
    };
    let slots = emit::Slots {
        params: (0..g.params.len() as u32).collect(),
        inputs: (0..g.inputs.len() as u32).collect(),
        kernels: (0..g.kernels.len()).map(row).collect(),
        packs: lay.packs.iter().map(|p| p.access).collect(),
    };
    // Globals outside emit's prelude are refused by name whether or not Naga runs at emit.
    emit::prelude::globals(&g)?;
    let m = emit::module(&g, &exports, &slots, emit::Stage::Render)?;
    match build.naga {
        Naga::Emit => emit::check(&g, &m)?,
        Naga::Load => {}
    }
    phases.emit = ns(t);
    let t = Instant::now();
    let mixed = match build.precision {
        Precision::Mixed => {
            emit::validate(&g, &m, true).map_or_else(|e| Mixed::Refused(e.to_string()), |()| Mixed::Valid)
        }
        Precision::F32 => Mixed::NotAsked,
    };
    phases.f16 = ns(t);

    let t = Instant::now();
    let functions = m.text.lines().filter(|l| l.starts_with("fn f")).count();
    let text = m.text + "\n" + &lens_glue;
    let wgsl_bytes = text.len();
    let mut sections = plan.sections;
    sections.push(text.into_bytes().into());
    let mut modules = vec![Module { wgsl: sections.len() as u32 - 1, origin: Origin::Author, precision_alias: true }];
    let mut strings: Vec<Box<str>> = Vec::new();
    let mut name = |s: &str| {
        strings.push(s.into());
        strings.len() as u32 - 1
    };
    // Fill kernels in schedule order (the host runs them in bundle order); `Init::By` follows the new numbering.
    let mut renumber = vec![0u32; plan.kernels.len()];
    let mut kernels: Vec<bundle::Kernel> = Vec::with_capacity(order.len());
    let fill_entry = name("loft_fill");
    for &k in &order {
        let pk = &plan.kernels[k];
        let kernel::Job::Decl(d) = pk.job else { unreachable!("drop_lifted left only decl kernels") };
        let fm = fill::module(&mut g, &slots, &decls[d as usize], build.naga)?;
        sections.push(fm.text.into_bytes().into());
        modules.push(Module { wgsl: sections.len() as u32 - 1, origin: Origin::Author, precision_alias: true });
        let module = modules.len() as u32 - 1;
        let body = match decls[d as usize].fill {
            kernel::Fill::Map { .. } => bundle::Body::Map { module, entry: fill_entry, rows: fm.rows },
            kernel::Fill::Scan { .. } => bundle::Body::Scan { module, entry: fill_entry, rows: fm.rows },
            kernel::Fill::Raw { .. } => unreachable!("fill_decls makes no raw kernel"),
        };
        renumber[k] = kernels.len() as u32;
        kernels.push(bundle::Kernel {
            writes: Box::new([pk.writes]),
            reads: pk.reads.clone(),
            body,
            when: pk.when.clone(),
        });
    }
    let mut resources = plan.resources;
    for r in &mut resources {
        if let bundle::Init::By(k) = r.init {
            let k = renumber[k as usize];
            // The host runs an init kernel once at load (loft-gpu `validate`). A table a frame kernel fills is
            // written every frame before it is read, so it starts as zero, not as that kernel's output.
            r.init = match kernels[k as usize].when {
                bundle::When::Load => bundle::Init::By(k),
                bundle::When::Frame | bundle::When::Steps { .. } => bundle::Init::Zero,
            };
        }
    }
    let written: Vec<u32> = kernels.iter().flat_map(|k| k.writes.iter().copied()).collect();
    let unfilled = resources
        .iter()
        .zip(&sources)
        .enumerate()
        .filter(|(r, (x, s))| {
            matches!((&x.init, s), (bundle::Init::Zero, kernel::Source::Made(_))) && !written.contains(&(*r as u32))
        })
        .count();
    let lit = |i: u32| F32((f64::from_bits(g.lits[i as usize]) as f32).to_bits());
    let params = g.params.iter().map(|p| ParamDecl {
        name: name(g.sym(p.name)),
        def: lit(p.def),
        min: lit(p.min),
        max: lit(p.max),
    });
    let params: Box<[ParamDecl]> = params.collect();
    let mut inputs: Box<[InputDecl]> = g
        .inputs
        .iter()
        .map(|i| InputDecl {
            kind: bundle::InputKind::Level,
            name: name(g.sym(i.name)),
            ty: i.ty,
            def: (0..u32::from(i.ty.lits())).map(|k| lit(i.def + k)).collect(),
        })
        .collect();
    let cameras: Box<[bundle::Camera]> = (g.cameras.iter().enumerate())
        .map(|(k, c)| bundle::Camera { name: name(g.sym(c.name)), lens: (0, name(&format!("loft_lens{k}"))) })
        .collect();
    // Draws, media and passes: lowered by `draw` into modules of their own; their module and name ids move past ours.
    let (draws_ir, media_ir, passes_ir) = draw_decls(&g);
    let low = draw::lower(&g, &slots, &draws_ir, &media_ir, &passes_ir)?;
    let mut row_steps = Vec::new();
    for (spec, kind) in &g.row_steps {
        let authored = g.sym(spec.module);
        let (source, packet) = match kind {
            bundle::InputKind::Level | bundle::InputKind::Script { .. } => (authored.to_owned(), "vec4f"),
            bundle::InputKind::Events { slots } => {
                (format!("struct TickEvents {{ count:u32, values:array<vec4f,{slots}> }}\n{authored}"), "TickEvents")
            }
        };
        // Module-private variables also reset at a dispatch boundary. Keeping them across fused ticks
        // would change a recurrence when a player changes presentation cadence.
        let parsed = naga::front::wgsl::parse_str(&source).map_err(|e| unsupported(e.emit_to_string(&source)))?;
        if !parsed.entry_points.is_empty() || !parsed.global_variables.is_empty() {
            return Err(unsupported("row-local laws must keep mutable values in State or function locals: use a shared-row Steps kernel in a Rust bundle for global or shared storage".into()));
        }
        let info =
            naga::valid::Validator::new(naga::valid::ValidationFlags::all(), naga::valid::Capabilities::default())
                .validate(&parsed)
                .map_err(|e| unsupported(e.emit_to_string(&source)))?;
        if parsed.functions.iter().any(|(id, _)| !info[id].uniformity.requirements.is_empty()) {
            return Err(unsupported(
                "row-local laws cannot synchronize invocations: use a shared-row Steps kernel in a Rust bundle for a barrier or collective".into(),
            ));
        }
        let text = format!("{source}\n{}", include_str!("../wgsl/advance.wgsl").replace("PACKET_TYPE", packet));
        if matches!(build.naga, Naga::Emit) {
            let module = naga::front::wgsl::parse_str(&text).map_err(|e| unsupported(e.emit_to_string(&text)))?;
            naga::valid::Validator::new(naga::valid::ValidationFlags::all(), naga::valid::Capabilities::default())
                .validate(&module)
                .map_err(|e| unsupported(e.emit_to_string(&text)))?;
        }
        let module = modules.len() as u32;
        sections.push(text.into_bytes().into());
        modules.push(Module { wgsl: sections.len() as u32 - 1, origin: Origin::Author, precision_alias: false });
        row_steps.push(bundle::RowStep { module, ..spec.clone() });
        let declaration = &mut inputs[spec.input as usize];
        if g.row_steps.iter().any(|(other, k)| other.input == spec.input && k != kind) {
            return Err(unsupported(
                "a shared stepped input has conflicting sources: choose one source or give each input its own name"
                    .into(),
            ));
        }
        declaration.kind = kind.clone();
    }
    let (module0, name0) = (modules.len() as u32, strings.len() as u32);
    for text in low.modules {
        sections.push(text.into_bytes().into());
        modules.push(Module { wgsl: sections.len() as u32 - 1, origin: Origin::Author, precision_alias: true });
    }
    strings.extend(low.names);
    let at = |(m, n): (u32, u32)| (m + module0, n + name0);
    let draws: Box<[bundle::Draw]> =
        low.draws.into_iter().map(|d| bundle::Draw { name: d.name + name0, shade: at(d.shade), ..d }).collect();
    let media: Box<[bundle::Medium]> =
        low.media.into_iter().map(|m| bundle::Medium { transmittance: at(m.transmittance), ..m }).collect();
    let passes: Box<[bundle::Pass]> = low
        .passes
        .into_iter()
        .map(|p| {
            let compose = bundle::Compose { outline: p.compose.outline.map(at), ground: p.compose.ground.map(at) };
            bundle::Pass { compose, ..p }
        })
        .collect();
    let b = Bundle {
        format: Format::V1,
        compiler: compiler.into(),
        names: strings.into(),
        resources: resources.into(),
        packs: lay.packs.into(),
        kernels: kernels.into(),
        row_steps,
        modules: modules.into(),
        draws,
        passes,
        media,
        cameras,
        params,
        inputs,
        sites: Box::new([]),
        sections: sections.into(),
    };
    let resources = b.resources.len();
    let (n_kernels, n_draws, n_packs) = (b.kernels.len() + b.row_steps.len(), b.draws.len(), b.packs.len());
    let bytes = b.write()?;
    let build = Sha256::digest(&bytes).iter().map(|x| format!("{x:02x}")).collect();
    phases.bundle = ns(t);
    let report = Scene {
        roots: program_roots,
        functions,
        wgsl_bytes,
        resources,
        unfilled,
        kernels: n_kernels,
        storage_buffers: n_packs,
        dispatches,
        draws: n_draws,
        bundle_bytes: bytes.len(),
        build,
        mixed,
    };
    Ok((report, bytes))
}

/// The job's draws, media and passes as `draw` lowers them: references stay decl ids (fns, kernels, media), numbers
/// become f32 bits. Every pass draws to the view (the wire has no texture resource decl yet).
fn draw_decls(g: &Graph) -> (Vec<draw::Draw>, Vec<draw::Medium>, Vec<draw::Pass>) {
    let f = |i: u32| F32((f64::from_bits(g.lits[i as usize]) as f32).to_bits());
    let v3 = |at: u32| [f(at), f(at + 1), f(at + 2)];
    let box6 = |at: u32| [v3(at), v3(at + 3)];
    let draws = g
        .draws
        .iter()
        .map(|d| {
            let cover = match &d.cover {
                ir::Cover::Grid { u, v, place, normal, cells, count } => draw::Cover::Raster {
                    prim: draw::Prim::Grid { u: *u, v: *v, place: *place, normal: *normal, cells: *cells },
                    count: *count,
                },
                ir::Cover::Mesh { kernel, count } => {
                    draw::Cover::Raster { prim: draw::Prim::Mesh { res: *kernel }, count: *count }
                }
                ir::Cover::Quads { place, count } => {
                    draw::Cover::Raster { prim: draw::Prim::Quads { place: *place }, count: *count }
                }
                ir::Cover::Ray { proxy, find } => draw::Cover::Ray {
                    proxy: match proxy {
                        ir::Proxy::Box { lits } => {
                            let [min, max] = box6(*lits);
                            bundle::Proxy::Box { min, max }
                        }
                        ir::Proxy::Instances { kernel, local } => {
                            bundle::Proxy::Instances { res: *kernel, local: box6(*local) }
                        }
                    },
                    find: match find {
                        ir::Find::Hit(h) => draw::Find::Hit(*h),
                        ir::Find::Medium(m) => draw::Find::Medium(*m),
                    },
                },
                ir::Cover::Screen => draw::Cover::Screen,
            };
            let layer = match d.layer {
                ir::Layer::Subject => bundle::Layer::Subject,
                ir::Layer::Surroundings => bundle::Layer::Surroundings,
            };
            let blend = match d.raster.blend {
                ir::Blend::Opaque => bundle::Blend::Opaque,
                ir::Blend::Alpha => bundle::Blend::Alpha,
                ir::Blend::Add => bundle::Blend::Add,
            };
            let raster = bundle::Raster { cull: d.raster.cull, depth_write: d.raster.depth_write, blend };
            draw::Draw { name: d.name, layer, cover, paint: d.paint, raster }
        })
        .collect();
    let media = g
        .media
        .iter()
        .map(|m| match *m {
            ir::Medium::March { density, within, steps, shadow_steps } => {
                draw::Medium::March { density, within: box6(within), steps, shadow_steps }
            }
            ir::Medium::Gaussians { kernel } => draw::Medium::Gaussians { res: kernel },
        })
        .collect();
    let passes = g
        .passes
        .iter()
        .map(|p| draw::Pass {
            target: bundle::Target::View,
            camera: p.camera,
            draws: p.draws,
            clear: match p.clear {
                ir::Clear::Load => bundle::Clear::Load,
                ir::Clear::Colour(at) => bundle::Clear::Color([f(at), f(at + 1), f(at + 2), f(at + 3)]),
            },
            ground: match p.ground {
                ir::Ground::None => None,
                ir::Ground::Traced { hit, paint } => Some([hit, paint]),
            },
            outline: match p.outline {
                ir::Outline::None => None,
                ir::Outline::Ink(f) => Some(f),
            },
        })
        .collect();
    (draws, media, passes)
}

/// One kernel decl per Map run and per Scan, writing the resource of its Kernel decl (resource k is kernel k). A
/// static table fills once at load; any other at most per frame (the plan moves a Const body to compile).
fn fill_decls(g: &Graph) -> Vec<kernel::Decl> {
    let mut out = vec![];
    for (k, x) in g.kernels.iter().enumerate() {
        let Kernel::Table { fill, role, .. } = x else { continue };
        let when = match role {
            Role::Static => bundle::When::Load,
            Role::Stations | Role::Data => bundle::When::Frame,
        };
        match fill {
            Fill::Map(runs) => {
                let mut start = 0;
                for r in runs.iter() {
                    let fill = kernel::Fill::Map { start, count: r.count, index: r.index, lanes: r.lanes.clone() };
                    out.push(kernel::Decl { writes: k as u32, fill, when: when.clone() });
                    start += r.count;
                }
            }
            Fill::Scan(s) => {
                let fill = kernel::Fill::Scan {
                    first: s.first,
                    steps: s.steps,
                    init: s.init.clone(),
                    state: s.state.clone(),
                    index: s.index,
                    next: s.next.clone(),
                    lanes: s.lanes.clone(),
                };
                out.push(kernel::Decl { writes: k as u32, fill, when });
            }
            Fill::Data { .. } | Fill::Recurrence(_) => {}
        }
    }
    out
}

/// Emit does not yet read lifted frame values (each root computes its values whole), so the plan's frame-values
/// kernels and resources would be written and never read. They are dropped with the reads that name them. They come
/// last in both lists, so no other index moves.
fn drop_lifted(plan: &mut kernel::Plan, declared: usize) {
    plan.kernels.retain(|k| matches!(k.job, kernel::Job::Decl(_)));
    plan.resources.truncate(declared);
    for k in &mut plan.kernels {
        k.reads = k.reads.iter().copied().filter(|&r| (r as usize) < declared).collect();
    }
    plan.lifted.clear();
}

/// The free variables (`Op::Var`) root `r` reads, one node per name, in name order: the exported function's
/// parameters. Loops are walked (their bodies are inline); graph function bodies are not (they read only binders).
fn free_vars(g: &Graph, r: u32) -> Box<[u32]> {
    let mut seen = std::collections::HashSet::new();
    let mut stack = vec![r];
    let mut vars: Vec<(&str, u32)> = vec![];
    while let Some(x) = stack.pop() {
        if !seen.insert(x) {
            continue;
        }
        let n = &g.nodes[x as usize];
        stack.extend(g.args_of(n));
        match n.op {
            Op::Var(s) => vars.push((g.sym(s), x)),
            Op::Loop { decl, .. } => stack.extend(g.loops[decl as usize].nodes()),
            Op::Lit(_)
            | Op::Time
            | Op::Param(_)
            | Op::Input(_)
            | Op::Bind(_)
            | Op::Call(_)
            | Op::Fetch { .. }
            | Op::Grad
            | Op::Raw(_)
            | Op::Swizzle(_)
            | Op::Vec
            | Op::Prim(_) => {}
        }
    }
    // Two nodes of one name would be two parameters of one name: the recorder interns a free variable per name, so
    // keep the first and let emit refuse the other as an unbound variable if it ever happens.
    vars.sort();
    vars.dedup_by_key(|v| v.0);
    vars.into_iter().map(|(_, x)| x).collect()
}
