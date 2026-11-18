//! Kernels and resources: one model of state (plan §1.4). A `Decl` says what fills which resource and at what clock;
//! `plan` decides once, from rates, where each fill runs:
//!
//! - a Map or Scan whose body is Const (reads no time, param, input, curve or per-frame resource) is run on the tape
//!   at compile and becomes `Init::Bytes`; no kernel is left. When the tape cannot run it yet (a fetch, a loop, a raw
//!   call, `pick`), it runs once on the GPU at load: `When::Load`, the named place for that case;
//! - a fill that reads the frame runs per frame, and its per-frame frontier is lifted into frame-values resources
//!   (src/compile/rates.ts, inherited);
//! - `Steps` and author `Raw` passes keep the clock they declare. Ping-pong, history and checkpoint replay are the
//!   host's job for `Keep` (§1.4), so a stepped resource is one resource whatever its keep.
use crate::prim::Prim as P;
use crate::rate;
use crate::tape::{self, Id, Ssa};
use loft_ir::bundle::{Init, Resource, Shape, When};
use loft_ir::Error;
use loft_ir::graph::{Graph, Op, Prim};
use std::collections::HashMap;

/// A declared resource: made by kernels, or an author curve (f32 samples `rows[at..at + len]`, one vec4 per row),
/// which the host may rewrite each frame, so reading one is per frame.
pub enum Source {
    Made(Shape),
    Curve { at: u32, len: u32 },
}

/// A declared kernel. `When::Frame` means "at most per frame": a Const body moves to compile. `Load` runs once (a
/// recurrence's start, an author load pass); a Const `Load` body also moves to compile.
pub struct Decl {
    pub writes: u32,
    pub fill: Fill,
    pub when: When,
}

pub enum Fill {
    /// Rows `start..start + count`: `f` is a `Fn` decl of one binder (the row index, f32) whose results are the
    /// row's lanes, a vec4 each.
    Map { start: u32, count: u32, f: u32 },
    /// Rows `first..first + steps`, in order: `init` is a `Fn` of no binder giving the state; `step` a `Fn` over
    /// `[state..., i]` giving `[next..., lanes...]`, with `i` the step number.
    Scan { first: u32, steps: u32, init: u32, step: u32 },
    /// An author compute pass: `module`'s raw WGSL, `dispatch` workgroups, reading `reads`.
    Raw { module: u32, dispatch: [u32; 3], reads: Box<[u32]> },
}

pub struct Plan {
    /// One per `Source`, then one per frame-values depth.
    pub resources: Vec<Resource>,
    pub kernels: Vec<Kernel>,
    pub sections: Vec<Box<[u8]>>,
    /// Lifted node -> (frame-values resource, row), sorted by node: emit fetches these instead of computing them.
    pub lifted: Vec<(u32, u32, u32)>,
}

/// A GPU kernel left after planning.
pub struct Kernel {
    pub job: Job,
    pub writes: u32,
    pub reads: Box<[u32]>,
    pub when: When,
}

pub enum Job {
    Decl(u32),
    /// Frame values: one row per lifted node, computed by one invocation that shares their work.
    Lifted(Box<[u32]>),
}

/// The states a stepped resource keeps: the first kept step is the one at time max(from, 0), and with `Every(k)` a
/// state every k steps after it; the host replays between them (src/map/recur.ts checkpoints, unified with compute).
pub fn kept(when: &When) -> u32 {
    match when {
        When::Steps { dt, from, count, keep } => {
            let first = (-f32::from_bits(from.0) / f32::from_bits(dt.0)).round().clamp(0.0, *count as f32) as u32;
            let every = match keep {
                loft_ir::bundle::Keep::Latest => return 1,
                loft_ir::bundle::Keep::Every(k) => (*k).max(1),
            };
            1 + (count - first) / every
        }
        When::Load | When::Frame => 1,
    }
}

pub fn plan(g: &Graph, sources: &[Source], decls: &[Decl]) -> Result<Plan, Error> {
    let fns = |d: &Decl| -> Vec<u32> {
        match d.fill {
            Fill::Map { f, .. } => vec![f],
            Fill::Scan { init, step, .. } => vec![init, step],
            Fill::Raw { .. } => vec![],
        }
    };
    let results = |f: u32| &g.roots_of(&g.decls[f as usize])[rate::binders(g, f).len()..];
    let mut items: Vec<u32> = decls.iter().flat_map(|d| fns(d).into_iter().flat_map(|f| rate::binders(g, f).to_vec())).collect();
    items.sort_unstable();
    let reads: Vec<Box<[u32]>> = decls
        .iter()
        .map(|d| match &d.fill {
            Fill::Raw { reads, .. } => reads.clone(),
            Fill::Map { .. } | Fill::Scan { .. } => fetched(g, &fns(d).iter().flat_map(|&f| results(f).to_vec()).collect::<Vec<_>>()),
        })
        .collect();

    // Classify, refilling the per-frame bits of resources until they settle (a reader of a per-frame resource is
    // per frame). Monotone: at most one pass per resource.
    let curve: Vec<bool> = sources.iter().map(|s| matches!(s, Source::Curve { .. })).collect();
    let mut frame_res = curve.clone();
    let (rates, when) = loop {
        let rates = rate::rates(g, &frame_res, &items);
        let when: Vec<When> = decls
            .iter()
            .map(|d| {
                let body = fns(d).iter().flat_map(|&f| results(f)).fold(false, |a, &x| a | rates[x as usize].frame);
                match (&d.when, &d.fill) {
                    (When::Steps { .. }, _) | (_, Fill::Raw { .. }) | (When::Load, _) => d.when.clone(),
                    (When::Frame, Fill::Map { .. } | Fill::Scan { .. }) => if body { When::Frame } else { When::Load },
                }
            })
            .collect();
        let mut next = curve.clone();
        for (d, w) in decls.iter().zip(&when) {
            next[d.writes as usize] |= matches!(w, When::Frame | When::Steps { .. });
        }
        if next == frame_res {
            break (rates, when);
        }
        frame_res = next;
    };

    // Compile fills: a Const Map/Scan whose resource has no other writer that must stay on the GPU before frames.
    let body_frame = |k: usize| fns(&decls[k]).iter().flat_map(|&f| results(f)).any(|&x| rates[x as usize].frame);
    let gpu_writer = |r: u32, k: usize| {
        decls.iter().enumerate().any(|(j, d)| j != k && d.writes == r && !matches!(when[j], When::Steps { .. }) && (body_frame(j) || matches!(d.fill, Fill::Raw { .. })))
    };
    let mut bytes: Vec<Option<Vec<u8>>> = (0..sources.len()).map(|_| None).collect();
    let mut on_gpu = vec![true; decls.len()];
    for (k, d) in decls.iter().enumerate() {
        if matches!(when[k], When::Load) && !body_frame(k) && !gpu_writer(d.writes, k) {
            let size = match &sources[d.writes as usize] {
                Source::Made(Shape::Rows { rows, lanes }) => *rows as usize * *lanes as usize * 16,
                Source::Made(Shape::Texture { .. } | Shape::ViewSized { .. } | Shape::Frame) | Source::Curve { .. } => continue,
            };
            let out = bytes[d.writes as usize].get_or_insert_with(|| vec![0; size]);
            if fill(g, d, out).is_some() {
                on_gpu[k] = false;
            }
        }
    }
    // Rows no compile fill wrote stay zero; a resource none wrote has no section (its kernels fill it at load).
    for (r, b) in bytes.iter_mut().enumerate() {
        if !decls.iter().enumerate().any(|(k, d)| !on_gpu[k] && d.writes as usize == r) {
            *b = None;
        }
    }

    // Lift the per-frame frontier of every per-frame Map run over more than one row, grouped by the depth of what
    // each value reads, so a value waits only for the resources it reads.
    let gpu = |k: usize| Kernel { job: Job::Decl(k as u32), writes: decls[k].writes, reads: reads[k].clone(), when: when[k].clone() };
    let pre: Vec<Kernel> = (0..decls.len()).filter(|&k| on_gpu[k]).map(gpu).collect();
    let level = crate::schedule::levels(&pre, sources.len())?;
    let mut depth = vec![0u32; sources.len()];
    for (x, l) in pre.iter().zip(&level).filter(|(x, _)| matches!(x.when, When::Frame)) {
        depth[x.writes as usize] = depth[x.writes as usize].max(l + 1);
    }
    let mut lifted: HashMap<u32, (u32, u32)> = HashMap::new();
    let mut groups: Vec<Vec<u32>> = Vec::new();
    let mut extra: Vec<Vec<u32>> = vec![Vec::new(); decls.len()];
    for (k, d) in decls.iter().enumerate() {
        let Fill::Map { f, count: 2.., .. } = d.fill else { continue };
        if !on_gpu[k] || !matches!(when[k], When::Frame) {
            continue;
        }
        for x in rate::frontier(g, &rates, results(f)) {
            let at = fetched(g, &[x]).iter().map(|&r| depth[r as usize]).max().unwrap_or(0) as usize;
            let (group, _) = *lifted.entry(x).or_insert_with(|| {
                groups.resize_with(groups.len().max(at + 1), Vec::new);
                groups[at].push(x);
                (at as u32, groups[at].len() as u32 - 1)
            });
            extra[k].push(group);
        }
    }

    let mut sections = Vec::new();
    let mut section = |b: Box<[u8]>| {
        sections.push(b);
        sections.len() as u32 - 1
    };
    let first_kernel = |r: usize| (0..decls.len()).filter(|&k| on_gpu[k]).position(|k| decls[k].writes as usize == r);
    let mut resources: Vec<Resource> = sources
        .iter()
        .enumerate()
        .map(|(r, s)| match s {
            Source::Curve { at, len } => Resource {
                shape: Shape::Rows { rows: len / 4, lanes: 1 },
                init: Init::Bytes(section(g.rows[*at as usize..(at + len) as usize].iter().flat_map(|b| b.to_le_bytes()).collect())),
            },
            Source::Made(shape) => Resource {
                shape: shape.clone(),
                init: match (bytes[r].take(), first_kernel(r)) {
                    (Some(b), _) => Init::Bytes(section(b.into())),
                    (None, Some(k)) => Init::By(k as u32),
                    (None, None) => Init::Zero,
                },
            },
        })
        .collect();
    // One frame-values resource per non-empty depth group, after the declared ones.
    let mut id = resources.len() as u32;
    let res_of: Vec<u32> = groups.iter().map(|v| if v.is_empty() { u32::MAX } else { id += 1; id - 1 }).collect();
    let mut kernels: Vec<Kernel> = (0..decls.len())
        .filter(|&k| on_gpu[k])
        .map(|k| {
            let mut x = gpu(k);
            let mut r: Vec<u32> = x.reads.iter().copied().chain(extra[k].iter().map(|&g| res_of[g as usize])).collect();
            r.sort_unstable();
            r.dedup();
            x.reads = r.into();
            x
        })
        .collect();
    for values in groups.into_iter().filter(|v| !v.is_empty()) {
        let writes = resources.len() as u32;
        resources.push(Resource { shape: Shape::Rows { rows: values.len() as u32, lanes: 1 }, init: Init::By(kernels.len() as u32) });
        kernels.push(Kernel { reads: fetched(g, &values), job: Job::Lifted(values.into()), writes, when: When::Frame });
    }
    let mut lifted: Vec<(u32, u32, u32)> = lifted.into_iter().map(|(x, (at, lane))| (x, res_of[at as usize], lane)).collect();
    lifted.sort_unstable();
    Ok(Plan { resources, kernels, sections, lifted })
}

/// The resources fetched under `roots`, entering the decls of calls and loops; sorted.
pub fn fetched(g: &Graph, roots: &[u32]) -> Box<[u32]> {
    let (mut seen, mut out, mut stack) = (std::collections::HashSet::new(), Vec::new(), roots.to_vec());
    while let Some(x) = stack.pop() {
        if !seen.insert(x) {
            continue;
        }
        let n = &g.nodes[x as usize];
        stack.extend(g.args_of(n));
        match n.op {
            Op::Fetch { kernel, .. } => out.push(kernel),
            Op::Call(d) | Op::Loop { decl: d, .. } => stack.extend(g.roots_of(&g.decls[d as usize])),
            Op::Lit(_) | Op::Leaf(_) | Op::Prim(_) | Op::Swizzle(_) | Op::Grad | Op::Raw(_) => {}
        }
    }
    out.sort_unstable();
    out.dedup();
    out.into()
}

/// Fills `d`'s rows of `out` on the tape (f32, `libm`); None when the body has what the tape cannot run yet.
fn fill(g: &Graph, d: &Decl, out: &mut [u8]) -> Option<()> {
    let row = |out: &mut [u8], r: usize, lanes: &[f32]| {
        let w = lanes.len() * 4;
        out[r * w..(r + 1) * w].copy_from_slice(&lanes.iter().flat_map(|v| v.to_le_bytes()).collect::<Vec<_>>());
    };
    match d.fill {
        Fill::Map { start, count, f } => {
            let mut l = Lower { g, ssa: Ssa::default() };
            let (bind, res) = split(g, f);
            let i = l.ssa.leaf(0);
            let lanes: Vec<Id> = l.lower(res, &[(bind[0], vec![i])])?.concat();
            let leaves: Vec<f32> = (0..count).map(|i| i as f32).collect();
            let mut regs = Vec::new();
            tape::run(&l.ssa, count as usize, 1, &leaves, &mut regs).ok()?;
            let n = count as usize;
            for r in 0..n {
                row(out, start as usize + r, &lanes.iter().map(|&x| regs[x as usize * n + r]).collect::<Vec<_>>());
            }
        }
        Fill::Scan { first, steps, init, step } => {
            let mut a = Lower { g, ssa: Ssa::default() };
            let init_regs: Vec<Id> = a.lower(split(g, init).1, &[])?.concat();
            let (bind, res) = split(g, step);
            let mut b = Lower { g, ssa: Ssa::default() };
            let mut slot = 0;
            let mut binds = Vec::new();
            for &x in bind {
                let w = g.nodes[x as usize].ty.width()? as u32;
                binds.push((x, (slot..slot + w).map(|s| b.ssa.leaf(s)).collect::<Vec<_>>()));
                slot += w;
            }
            let outs = b.lower(res, &binds)?;
            let n_state = bind.len() - 1;
            let (next, lanes): (Vec<Id>, Vec<Id>) = (outs[..n_state].concat(), outs[n_state..].concat());
            let mut regs = Vec::new();
            tape::run(&a.ssa, 1, 0, &[], &mut regs).ok()?;
            let mut state: Vec<f32> = init_regs.iter().map(|&x| regs[x as usize]).collect();
            for k in 0..steps {
                state.push(k as f32);
                tape::run(&b.ssa, 1, slot as usize, &state, &mut regs).ok()?;
                row(out, (first + k) as usize, &lanes.iter().map(|&x| regs[x as usize]).collect::<Vec<_>>());
                state = next.iter().map(|&x| regs[x as usize]).collect();
            }
        }
        Fill::Raw { .. } => return None,
    }
    Some(())
}

/// A `Fn` decl's binders and results.
fn split(g: &Graph, f: u32) -> (&[u32], &[u32]) {
    let a = rate::binders(g, f).len();
    g.roots_of(&g.decls[f as usize]).split_at(a)
}

/// Graph to tape: vectors become component registers, calls are inlined over their arguments.
struct Lower<'g> {
    g: &'g Graph,
    ssa: Ssa,
}

impl Lower<'_> {
    /// The component registers of each root, with `binds` (binder node -> registers) in scope. Operands precede
    /// their users, so the reached nodes in id order are a valid evaluation order (no recursion on depth).
    fn lower(&mut self, roots: &[u32], binds: &[(u32, Vec<Id>)]) -> Option<Vec<Vec<Id>>> {
        let g = self.g;
        let mut memo: HashMap<u32, Vec<Id>> = binds.iter().cloned().collect();
        let (mut reach, mut stack) = (Vec::new(), roots.to_vec());
        let mut seen = std::collections::HashSet::new();
        while let Some(x) = stack.pop() {
            if seen.insert(x) && !memo.contains_key(&x) {
                reach.push(x);
                stack.extend(g.args_of(&g.nodes[x as usize]));
            }
        }
        reach.sort_unstable();
        for x in reach {
            let n = &g.nodes[x as usize];
            let args: Vec<&[Id]> = g.args_of(n).iter().map(|a| memo[a].as_slice()).collect();
            let v = match n.op {
                Op::Lit(at) => (0..n.ty.width().unwrap_or(1) as usize)
                    .map(|c| self.ssa.lit(f64::from_bits(g.lits[at as usize + c]) as f32))
                    .collect(),
                Op::Prim(Prim::Vec) => match (args.as_slice(), n.ty.width()?) {
                    ([one], w) if one.len() == 1 => vec![one[0]; w as usize],
                    (parts, _) => parts.concat(),
                },
                Op::Prim(p) => {
                    let p = twin(p)?;
                    self.ssa.apply(p, &args)
                }
                Op::Swizzle(s) => (0..s.len).map(|k| args[0][((s.comps >> (2 * k)) & 3) as usize]).collect(),
                Op::Call(d) => {
                    let (bind, res) = split(g, d);
                    let given: Vec<(u32, Vec<Id>)> = bind.iter().copied().zip(args.iter().map(|a| a.to_vec())).collect();
                    match self.lower(res, &given)?.as_slice() {
                        [one] => one.clone(),
                        _ => return None,
                    }
                }
                Op::Leaf(_) | Op::Fetch { .. } | Op::Loop { .. } | Op::Grad | Op::Raw(_) => return None,
            };
            memo.insert(x, v);
        }
        Some(roots.iter().map(|r| memo[r].clone()).collect())
    }
}

/// loft-ir's operator as the tape's (`Vec` is handled by the lowering; `Pick` is not a tape operator yet). The two
/// enums are the same operators in different orders until the wire is reconciled (wave 1 integration note).
#[rustfmt::skip]
fn twin(p: Prim) -> Option<P> {
    Some(match p {
        Prim::Add => P::Add, Prim::Sub => P::Sub, Prim::Mul => P::Mul, Prim::Div => P::Div, Prim::Min => P::Min,
        Prim::Max => P::Max, Prim::Pow => P::Pow, Prim::Mod => P::Mod, Prim::Rem => P::Rem, Prim::Atan2 => P::Atan2,
        Prim::Step => P::Step, Prim::Neg => P::Neg, Prim::Abs => P::Abs, Prim::Sign => P::Sign, Prim::Floor => P::Floor,
        Prim::Ceil => P::Ceil, Prim::Trunc => P::Trunc, Prim::RoundEven => P::RoundEven, Prim::Fract => P::Fract,
        Prim::Sqrt => P::Sqrt, Prim::InverseSqrt => P::InverseSqrt, Prim::Exp => P::Exp, Prim::Exp2 => P::Exp2,
        Prim::Log => P::Log, Prim::Log2 => P::Log2, Prim::Sin => P::Sin, Prim::Cos => P::Cos, Prim::Tan => P::Tan,
        Prim::Asin => P::Asin, Prim::Acos => P::Acos, Prim::Atan => P::Atan, Prim::Sinh => P::Sinh, Prim::Cosh => P::Cosh,
        Prim::Tanh => P::Tanh, Prim::Saturate => P::Saturate, Prim::Normalize => P::Normalize, Prim::Mix => P::Mix,
        Prim::Clamp => P::Clamp, Prim::Smoothstep => P::Smoothstep, Prim::Length => P::Length, Prim::Distance => P::Distance,
        Prim::Dot => P::Dot, Prim::Cross => P::Cross, Prim::Lt => P::Lt, Prim::Gt => P::Gt, Prim::Le => P::Le,
        Prim::Ge => P::Ge, Prim::Eq => P::Eq, Prim::Ne => P::Ne, Prim::And => P::And, Prim::Or => P::Or, Prim::Not => P::Not,
        Prim::Select => P::Select,
        Prim::Vec | Prim::Pick => return None,
    })
}
