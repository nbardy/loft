//! The driver's Check and Solve programs (lean Rust plan §1.7): a job's Check and Solve decls (wire 2) lowered onto the
//! tape over leaf slots, then `check` and `solve`. Time is slot 0; a solve's unknown k reads its d-th time derivative
//! at slot 1 + 4k + d; params and inputs follow, held at their defaults; a check's ranged free variables follow those.
//!
//! The default build asks for Solve: it solves and reports every hold `Proof::Unproved`. `loft check` asks for Check:
//! it decides every check and solves and proves every hold (`solve::prove`). An unproved hold is never a pass.
use crate::check::{self, Slot, Strategy, Verdict, Work};
use crate::kernel::Lower;
use crate::prim::Atom;
use crate::solve::{self, Proof};
use crate::tape::{Id, Ssa};
use loft_ir::error::{Error, Limits, SceneFault};
use loft_ir::graph::{self as ir, Graph, HoldKind, Op, Site, Times};
use serde::Serialize;

#[derive(Serialize)]
pub struct CheckReport {
    pub label: String,
    pub verdict: Verdict,
}

#[derive(Serialize)]
pub struct SolveReport {
    pub label: String,
    /// Each unknown's name and its f32 numbers.
    pub curves: Vec<(String, Vec<f32>)>,
    /// One per hold, in decl order: where it was written and what is known of it on the returned curves.
    pub holds: Vec<(Site, Proof)>,
    pub rounds: u32,
    pub steps: u32,
    pub tape_ops: u64,
}

/// Whether a solve's holds are proved on its curves (`loft check`) or left `Unproved` (the build).
#[derive(Clone, Copy)]
pub enum Holds {
    Unproved,
    Prove,
}

fn refuse(site: Option<Site>, what: String) -> Error {
    Error::Scene { site, fault: SceneFault::Unsupported { what: what.into() } }
}

/// `file:line` or `file:line:col`, as the recorder writes a hold's site; anything else is a file of that name.
fn site_of(s: &str) -> Site {
    let mut parts = s.rsplitn(3, ':');
    let (last, mid, rest) = (parts.next(), parts.next(), parts.next());
    match (rest, mid.map(str::parse::<u32>), last.map(str::parse::<u32>)) {
        (Some(file), Some(Ok(line)), Some(Ok(col))) => Site { file: file.into(), line, col },
        (_, _, Some(Ok(line))) if mid.is_some() => {
            let file = s.rsplit_once(':').map_or(s, |x| x.0);
            Site { file: file.into(), line, col: 0 }
        }
        _ => Site { file: s.into(), line: 0, col: 0 },
    }
}

/// Leaf slots for a program over `roots`: time at 0, then `fixed` (node, slot) as given, then each param and input
/// read, held at its default. Any other free variable is refused: only a range or a solve's jets give it a value.
/// A ranged param or input reads its range's slot inside the range; a range end reading it is refused by `check`.
struct Frame {
    held: Vec<f32>,
    binds: Vec<(u32, Vec<Id>)>,
}
fn frame(g: &Graph, ssa: &mut Ssa, roots: &[u32], fixed: &[(u32, Slot)], first: Slot) -> Result<Frame, Error> {
    let lit = |i: u32| f64::from_bits(g.lits[i as usize]) as f32;
    let mut held = vec![0.0f32; first as usize];
    let (mut binds, mut seen, mut stack) = (vec![], std::collections::HashSet::new(), roots.to_vec());
    let mut params: Vec<(u32, Slot)> = vec![];
    let mut inputs: Vec<(u32, Slot)> = vec![];
    while let Some(x) = stack.pop() {
        if !seen.insert(x) {
            continue;
        }
        let n = &g.nodes[x as usize];
        stack.extend(g.args_of(n));
        // A ranged leaf or a solve's jet (a binder on the wire) reads its own slot.
        if let Some(&(_, slot)) = fixed.iter().find(|f| f.0 == x) {
            binds.push((x, vec![ssa.leaf(slot)]));
            continue;
        }
        let slots: Vec<Slot> = match n.op {
            Op::Time => vec![0],
            Op::Var(s) => {
                let what = format!(
                    "the free variable `{}` outside a range of it: range it with always or eventually",
                    g.sym(s)
                );
                return Err(refuse(g.site(x).cloned(), what));
            }
            Op::Param(p) => {
                let at = params.iter().find(|q| q.0 == p).map(|q| q.1).unwrap_or_else(|| {
                    held.push(lit(g.params[p as usize].def));
                    params.push((p, held.len() as Slot - 1));
                    held.len() as Slot - 1
                });
                vec![at]
            }
            Op::Input(i) => {
                let d = g.inputs[i as usize];
                let at = inputs.iter().find(|q| q.0 == i).map(|q| q.1).unwrap_or_else(|| {
                    let at = held.len() as Slot;
                    held.extend((0..u32::from(d.ty.lits())).map(|k| lit(d.def + k)));
                    inputs.push((i, at));
                    at
                });
                (at..at + u32::from(d.ty.lits())).collect()
            }
            Op::Lit(_)
            | Op::Bind(_)
            | Op::Call(_)
            | Op::Fetch { .. }
            | Op::Grad
            | Op::Raw(_)
            | Op::Swizzle(_)
            | Op::Field(_)
            | Op::Make(_)
            | Op::Open(_)
            | Op::Vec
            | Op::Prim(_)
            | Op::Loop { .. } => continue,
        };
        binds.push((x, slots.iter().map(|&k| ssa.leaf(k)).collect()));
    }
    Ok(Frame { held, binds })
}

/// Lowers `roots` with `f`'s leaves bound; refused by name where the tape cannot run a node yet.
fn lower(g: &Graph, ssa: Ssa, roots: &[u32], f: &Frame, what: &str) -> Result<(Ssa, Vec<Vec<Id>>), Error> {
    let mut l = Lower { g, ssa };
    match l.lower(roots, &f.binds) {
        Some(regs) => Ok((l.ssa, regs)),
        None => Err(refuse(
            None,
            format!("{what} reading a table, a loop, a raw function or a gradient: the CPU tape cannot run it yet"),
        )),
    }
}

/// Every check decl of the job, decided.
pub fn checks(g: &Graph, limits: Limits) -> Result<Vec<CheckReport>, Error> {
    let mut out = vec![];
    for c in g.checks.iter() {
        let label = g.sym(c.label).to_string();
        // Every quantified leaf gets its own slot, after time (a time range binds slot 0 itself).
        let (mut roots, mut fixed, mut next) = (vec![], vec![], 1);
        walk(&c.body, &mut |f| match f {
            ir::Formula::Atom { margin, test } => roots.extend([*margin, *test]),
            ir::Formula::Always(q) | ir::Formula::Eventually(q) => {
                roots.extend(q.lo.iter().chain(q.hi.iter()));
                for &leaf in q.leaves.iter() {
                    if g.nodes[leaf as usize].op != Op::Time && !fixed.iter().any(|x: &(u32, Slot)| x.0 == leaf) {
                        fixed.push((leaf, next));
                        next += 1;
                    }
                }
            }
            ir::Formula::And(_) | ir::Formula::Or(_) | ir::Formula::Not(_) => {}
        });
        let mut ssa = Ssa::default();
        let fr = frame(g, &mut ssa, &roots, &fixed, next)?;
        let (ssa, regs) = lower(g, ssa, &roots, &fr, &format!("check `{label}`"))?;
        let reg = |node: u32| regs[roots.iter().position(|&r| r == node).expect("lowered with the formula")][0];
        let slot = |leaf: u32| fixed.iter().find(|x| x.0 == leaf).map_or(0, |x| x.1);
        let body = formula(&c.body, &reg, &slot);
        let how = match c.how {
            ir::Strategy::Prove { boxes } => {
                Strategy::Prove { budget: Work { boxes: boxes.min(cap(limits)) }, precision: PRECISION }
            }
            ir::Strategy::Sample { n } => Strategy::Sample { n },
        };
        let verdict = check::check(&ssa, &fr.held, &check::Check { site: None, body, how })?;
        out.push(CheckReport { label, verdict });
    }
    Ok(out)
}

/// `src/ranged.ts`'s default: a proof stops once the margin's bounds are this close.
const PRECISION: f64 = 1e-4;
fn cap(limits: Limits) -> u32 {
    u32::try_from(limits.boxes).unwrap_or(u32::MAX)
}

fn walk(f: &ir::Formula, visit: &mut impl FnMut(&ir::Formula)) {
    visit(f);
    match f {
        ir::Formula::Atom { .. } => {}
        ir::Formula::Always(q) | ir::Formula::Eventually(q) => walk(&q.body, visit),
        ir::Formula::And(fs) | ir::Formula::Or(fs) => fs.iter().for_each(|f| walk(f, visit)),
        ir::Formula::Not(f) => walk(f, visit),
    }
}

/// The wire's formula over registers; a range over several leaves is nested ranges, one leaf each, in order.
fn formula(f: &ir::Formula, reg: &impl Fn(u32) -> Id, slot: &impl Fn(u32) -> Slot) -> check::Formula {
    let quant = |q: &ir::Quant, every: bool| {
        let mut body = formula(&q.body, reg, slot);
        for k in (0..q.leaves.len()).rev() {
            let q = check::Quant { leaf: slot(q.leaves[k]), lo: reg(q.lo[k]), hi: reg(q.hi[k]), body: Box::new(body) };
            body = if every { check::Formula::Always(q) } else { check::Formula::Eventually(q) };
        }
        body
    };
    match f {
        ir::Formula::Atom { margin, test } => check::Formula::Atom { margin: reg(*margin), test: reg(*test) },
        ir::Formula::Always(q) => quant(q, true),
        ir::Formula::Eventually(q) => quant(q, false),
        ir::Formula::And(fs) => check::Formula::And(fs.iter().map(|f| formula(f, reg, slot)).collect()),
        ir::Formula::Or(fs) => check::Formula::Or(fs.iter().map(|f| formula(f, reg, slot)).collect()),
        ir::Formula::Not(f) => check::Formula::Not(Box::new(formula(f, reg, slot))),
    }
}

/// Every solve decl of the job: solved, and its holds proved when `holds` asks.
pub fn solves(g: &Graph, limits: Limits, holds: Holds, phases: &mut crate::Phases) -> Result<Vec<SolveReport>, Error> {
    let lit = |i: u32| f64::from_bits(g.lits[i as usize]);
    let mut out = vec![];
    for s in g.solves.iter() {
        let label = g.sym(s.label).to_string();
        let fixed: Vec<(u32, Slot)> = s
            .unknowns
            .iter()
            .enumerate()
            .flat_map(|(k, u)| u.jets.iter().enumerate().map(move |(d, &j)| (j, 1 + 4 * k as Slot + d as Slot)))
            .collect();
        let first = 1 + 4 * s.unknowns.len() as Slot;
        let margins = s.holds.iter().map(|h| match h.kind {
            HoldKind::Equal { margin, .. } | HoldKind::Bound { margin } => margin,
        });
        let roots: Vec<u32> =
            margins.chain(s.unknowns.iter().map(|u| u.from)).chain(s.near.iter().map(|z| z.to)).collect();
        let mut ssa = Ssa::default();
        let fr = frame(g, &mut ssa, &roots, &fixed, first)?;
        let (mut ssa, regs) = lower(g, ssa, &roots, &fr, &format!("solve `{label}`"))?;
        let over = [lit(s.numbers), lit(s.numbers + 1)];
        let mut lines = vec![];
        for (h, margin) in s.holds.iter().zip(&regs) {
            let site = site_of(g.sym(h.site));
            let said = g.sym(h.said);
            // The condition as written and its robustness margin, as `src/geom/solve.ts` `holdsOf` builds them: a
            // bound's margin is already its amount over the limit; an equal holds where max |a - b| <= within.
            let (kind, test, robust) = match (h.kind, said) {
                (HoldKind::Bound { .. }, "ge" | "le" | "gt" | "lt") => {
                    let zero = ssa.lit(0.0);
                    let cmp = if matches!(said, "gt" | "lt") { Atom::Gt } else { Atom::Ge };
                    (solve::Kind::Bound, ssa.op(cmp, [margin[0], zero, 0]), margin[0])
                }
                (HoldKind::Equal { within, .. }, "equal") => {
                    let mut largest = ssa.op(Atom::Abs, [margin[0], 0, 0]);
                    for &c in &margin[1..] {
                        let a = ssa.op(Atom::Abs, [c, 0, 0]);
                        largest = ssa.op(Atom::Max, [largest, a, 0]);
                    }
                    let w = ssa.lit(lit(within) as f32);
                    let (test, robust) = (ssa.op(Atom::Le, [largest, w, 0]), ssa.op(Atom::Sub, [w, largest, 0]));
                    (solve::Kind::Equal { within: lit(within) }, test, robust)
                }
                (HoldKind::Bound { .. } | HoldKind::Equal { .. }, said) => {
                    return Err(refuse(Some(site), format!("a hold said as `{said}`: equal, ge, le, gt or lt")));
                }
            };
            let times = match h.times {
                Times::At { t } => [lit(t); 2],
                Times::During { lits } => [lit(lits), lit(lits + 1)],
                Times::Span => over,
            };
            lines.push(solve::Hold { site, kind, margin: margin.clone(), test, robust, times });
        }
        let at = s.holds.len();
        let problem = solve::Solve {
            ssa,
            held: fr.held,
            time: 0,
            jets: 1,
            over,
            vary: s
                .unknowns
                .iter()
                .zip(&regs[at..])
                .map(|(u, r)| solve::Family { n: u.numbers as usize, from: r[0] })
                .collect(),
            holds: lines,
            prefer: solve::Prefer {
                jerk: lit(s.numbers + 3),
                acceleration: lit(s.numbers + 4),
                near_weight: lit(s.numbers + 5),
                near: s
                    .near
                    .iter()
                    .zip(&regs[at + s.unknowns.len()..])
                    .map(|(z, r)| (z.unknown as usize, r[0]))
                    .collect(),
            },
            delta: lit(s.numbers + 2),
            tape_ops: limits.tape_ops,
            boxes: Work { boxes: cap(limits) },
            precision: PRECISION,
        };
        let t = std::time::Instant::now();
        let mut solved = solve::solve(&problem)?;
        phases.solve += crate::ns(t);
        let t = std::time::Instant::now();
        match holds {
            Holds::Unproved => {}
            Holds::Prove => solve::prove(&problem, &mut solved)?,
        }
        phases.prove += crate::ns(t);
        out.push(SolveReport {
            label,
            curves: s.unknowns.iter().map(|u| g.sym(u.name).to_string()).zip(solved.controls).collect(),
            holds: problem.holds.iter().map(|h| h.site.clone()).zip(solved.holds).collect(),
            rounds: solved.rounds,
            steps: solved.steps,
            tape_ops: solved.tape_ops,
        });
    }
    Ok(out)
}
