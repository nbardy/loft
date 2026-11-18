//! Loft's compiler core, and its program driver (lean Rust plan §1.10, §8.2: the integrator's file).
//!
//! `run` takes one job (wire §3.2) and answers with a report, plus the bundle for a Scene. A Scene goes
//! decode → kernel plan → schedule → layout → emit (Naga, f32 and f16) → `Bundle` V1 → bytes and hash.
//!
//! What a Scene job can say today bounds what its bundle holds (driver note, `agent_notes/2026-11-19_lean_rust_driver.md`):
//! - the program roots are the expressions the TS emitter handed to `emitWgsl`, with their free variables (u, v, p).
//!   Each root becomes one exported function `loft_root{k}` whose parameters are those variables, in name order;
//! - a `Kernel` decl names a table but carries no fill body, so a table resource is declared with `Init::Zero` and the
//!   report counts it as `unfilled`;
//! - Draw, Pass, Medium, Camera, Check and Solve decls have no pinned layout (§3.2), so the bundle has no draws or
//!   passes and cannot be drawn yet. The report says `draws: 0`.
//!
//! Solve, Evaluate and Check jobs are `Internal::Unbuilt`: the wire cannot carry their decls yet.
pub mod cache;
pub mod check;
pub mod draw;
pub mod emit;
pub mod grad;
pub mod interval;
pub mod kernel;
pub mod layout;
pub mod prim;
pub mod rate;
pub mod schedule;
pub mod tape;

use loft_ir::bundle::{self, Bundle, F32, Format, InputDecl, Module, Origin, ParamDecl, Shape};
use loft_ir::error::{Internal, Limits, Meter, SceneFault};
use loft_ir::graph::{Graph, GraphFn, Kernel, Op, Program};
use loft_ir::{Error, wire};
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
    /// Table resources the job names but gives no fill for (wire §3.2 carries no Kernel body): `Init::Zero`.
    pub unfilled: usize,
    pub storage_buffers: usize,
    pub dispatches: usize,
    /// Always 0 until the wire pins Draw/Pass decls: the bundle cannot be drawn.
    pub draws: usize,
    pub bundle_bytes: usize,
    /// sha256 of the bundle file, hex: `Identity::build`.
    pub build: String,
    /// Whether the module also validates under `alias lf = f16;`. A build is f32 by default; a `Mixed` load of a
    /// module that does not is refused by the host's own Naga check (loft-gpu `validate`), so here it is reported.
    pub mixed: Mixed,
}

#[derive(Serialize)]
pub enum Mixed {
    Valid,
    /// Naga's refusal under f16, as `Error`'s text (a literal past f16's 65504 is the case met in the corpus).
    Refused(String),
}

#[derive(Serialize, Default)]
pub struct Phases {
    pub decode: u64,
    pub plan: u64,
    pub emit: u64,
    /// Naga under `alias lf = f16;` (lane 2.2's both-ways check), apart from emit's own f32 validation.
    pub f16: u64,
    pub bundle: u64,
}

/// One job. For a Scene, the bundle file's bytes come back beside the report; the caller writes them.
pub fn run(job: &[u8], limits: Limits, compiler: &str) -> Result<(Report, Option<Vec<u8>>), Error> {
    let mut phases = Phases::default();
    let t = Instant::now();
    let job = wire::decode(job)?;
    phases.decode = ns(t);
    let g = job.graph;
    limits.spend(Meter::Nodes, g.nodes.len() as u64)?;
    let (nodes, decls) =
        (g.nodes.len(), g.fns.len() + g.raws.len() + g.loops.len() + g.kernels.len() + g.params.len() + g.inputs.len());
    match job.program {
        Program::Scene => {
            let (scene, bytes) = scene(g, compiler, &mut phases)?;
            Ok((Report { program: job.program, nodes, decls, scene: Some(scene), phases }, Some(bytes)))
        }
        Program::Solve | Program::Evaluate | Program::Check => Err(Error::Internal(Internal::Unbuilt(job.program))),
    }
}

fn ns(t: Instant) -> u64 {
    t.elapsed().as_nanos() as u64
}

fn unsupported(what: String) -> Error {
    Error::Scene { site: None, fault: SceneFault::Unsupported { what: what.into() } }
}

fn scene(mut g: Graph, compiler: &str, phases: &mut Phases) -> Result<(Scene, Vec<u8>), Error> {
    // Plan: one resource per Kernel decl, in decl order, so resource k is kernel k (emit's Slots, Fetch's binding).
    let t = Instant::now();
    let sources = g
        .kernels
        .iter()
        .map(|k| match *k {
            Kernel::Table { lanes, entries, .. } => {
                let lanes = u16::try_from(lanes)
                    .map_err(|_| unsupported(format!("a table of {lanes} lanes; a row holds at most {}", u16::MAX)))?;
                Ok(kernel::Source::Made(Shape::Rows { rows: entries, lanes }))
            }
            Kernel::Rows { at, len, .. } => Ok(kernel::Source::Curve { at, len }),
        })
        .collect::<Result<Vec<_>, Error>>()?;
    // No fill decls: the wire carries none yet (module docs).
    let plan = kernel::plan(&g, &sources, &[])?;
    assert!(plan.kernels.is_empty(), "a plan with no fill decls has no kernels");
    let dispatches = schedule::schedule(&plan, &[])?.len();
    let lay = layout::layout(&plan)?;
    phases.plan = ns(t);

    // Emit: each distinct root as `r{k}(free variables in name order)`, as a graph function appended to the job's.
    let t = Instant::now();
    let mut roots: Vec<u32> = Vec::with_capacity(g.roots.len());
    for &r in g.roots.iter() {
        if !roots.contains(&r) {
            roots.push(r);
        }
    }
    let first = g.fns.len() as u32;
    let wrappers: Vec<GraphFn> = roots
        .iter()
        .map(|&r| GraphFn { name: 0, ret: g.nodes[r as usize].ty, binders: free_vars(&g, r), body: r })
        .collect();
    g.fns = g.fns.iter().cloned().chain(wrappers).collect();
    // `loft_` is the reserved prefix (draw templates); `r{k}` would meet emit's storage bindings `r{res}`.
    let names: Vec<String> = (0..roots.len()).map(|k| format!("loft_root{k}")).collect();
    let exports: Vec<(&str, u32)> = names.iter().enumerate().map(|(k, n)| (n.as_str(), first + k as u32)).collect();
    let slots = emit::Slots {
        params: (0..g.params.len() as u32).collect(),
        inputs: (0..g.inputs.len() as u32).collect(),
        kernels: g
            .kernels
            .iter()
            .enumerate()
            .map(|(k, x)| match *x {
                Kernel::Table { lanes, .. } => [k as u32, lanes],
                Kernel::Rows { .. } => [k as u32, 1],
            })
            .collect(),
    };
    let m = emit::module(&g, &exports, &slots)?;
    phases.emit = ns(t);
    let t = Instant::now();
    let mixed = emit::validate(&g, &m, true).map_or_else(|e| Mixed::Refused(e.to_string()), |()| Mixed::Valid);
    phases.f16 = ns(t);

    let t = Instant::now();
    let unfilled = plan
        .resources
        .iter()
        .zip(&sources)
        .filter(|(r, s)| matches!((&r.init, s), (bundle::Init::Zero, kernel::Source::Made(_))))
        .count();
    let functions = m.text.lines().filter(|l| l.starts_with("fn f")).count();
    let wgsl_bytes = m.text.len();
    let mut sections = plan.sections;
    sections.push(m.text.into_bytes().into());
    let mut strings: Vec<Box<str>> = Vec::new();
    let mut name = |s: &str| {
        strings.push(s.into());
        strings.len() as u32 - 1
    };
    let lit = |i: u32| F32((f64::from_bits(g.lits[i as usize]) as f32).to_bits());
    let params = g.params.iter().map(|p| ParamDecl {
        name: name(g.sym(p.name)),
        def: lit(p.def),
        min: lit(p.min),
        max: lit(p.max),
    });
    let params: Box<[ParamDecl]> = params.collect();
    let inputs: Box<[InputDecl]> = g
        .inputs
        .iter()
        .map(|i| InputDecl {
            name: name(g.sym(i.name)),
            ty: i.ty,
            def: (0..u32::from(i.ty.lits())).map(|k| lit(i.def + k)).collect(),
        })
        .collect();
    names.iter().for_each(|n| _ = name(n));
    let b = Bundle {
        format: Format::V1,
        compiler: compiler.into(),
        names: strings.into(),
        resources: plan.resources.into(),
        kernels: Box::new([]),
        modules: Box::new([Module {
            wgsl: sections.len() as u32 - 1,
            origin: Origin::Author,
            precision_alias: true,
            reads: m.reads.iter().copied().collect(),
        }]),
        draws: Box::new([]),
        passes: Box::new([]),
        media: Box::new([]),
        cameras: Box::new([]),
        params,
        inputs,
        sites: Box::new([]),
        sections: sections.into(),
    };
    let resources = b.resources.len();
    let bytes = b.write()?;
    let build = Sha256::digest(&bytes).iter().map(|x| format!("{x:02x}")).collect();
    phases.bundle = ns(t);
    let report = Scene {
        roots: roots.len(),
        functions,
        wgsl_bytes,
        resources,
        unfilled,
        storage_buffers: lay.buffers.len(),
        dispatches,
        draws: 0,
        bundle_bytes: bytes.len(),
        build,
        mixed,
    };
    Ok((report, bytes))
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
