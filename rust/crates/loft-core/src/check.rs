//! One check, one verdict (lean Rust plan §1.7): `always`/`eventually` over ranges, combined by and/or/not, decided by
//! branch and bound over the f32 twin (`tape`) with f32-padded enclosures (`interval`), or sampled when the formula
//! says so. The search is `src/ranged.ts`'s (best first, point middles and corners, pattern polish, nested searches
//! started from the enclosing search's witness, inner witnesses that move along a split leaf as a line).
//!
//! New here: the correlation the TS kept with affine forms comes from a centred form instead. Over a box X,
//! twin(x) lies in real(m) + sum_j D_j (X_j - m_j) +- E, where D_j encloses the derivative (`grad`, enclosed over X),
//! real(m) the padded point enclosure at the middle, and E the twin's rounding over X (a forward running-error bound:
//! e_r = pad_r + sum_k |d op / d arg_k| e_k, with the partials enclosed by each atom's own derivative rule). The result
//! is intersected with the plain enclosure. DERIVATION: the mean value theorem for Lipschitz programs (Clarke: `abs`,
//! `min`, `max` have subgradient enclosures through `sign` and `select` over intervals), so it is used only where every
//! jump (floor, comparisons, select, rem, atan2's cut) is decided over X. A moving inner witness (`Track`) is the chain
//! rule: x_k = c + sum_j s_kj (y_j - m_j) adds s_kj D_k to D_j.
use crate::grad::grad;
use crate::interval::{self, I, Unbounded};
use crate::prim::{self, Atom};
use crate::tape::{self, Id, Ins, Ssa};
use loft_ir::error::{Error, SceneFault};
use loft_ir::graph::Site;
use serde::{Serialize, Serializer};
use std::collections::{BTreeMap as Map, BTreeSet};
use std::sync::OnceLock;

/// A leaf slot of the tape (time, a param, an input component, a free variable).
pub type Slot = u32;

pub struct Check {
    pub site: Option<Site>,
    pub body: Formula,
    pub how: Strategy,
}
pub enum Formula {
    /// `margin` > 0 holds, < 0 fails; at 0 the comparison `test` (0 or 1) decides.
    Atom {
        margin: Id,
        test: Id,
    },
    Always(Quant),
    Eventually(Quant),
    And(Box<[Formula]>),
    Or(Box<[Formula]>),
    Not(Box<Formula>),
}
/// `leaf` ranges over [lo, hi]; the ends are registers read at the held values (they may not read a bound leaf).
pub struct Quant {
    pub leaf: Slot,
    pub lo: Id,
    pub hi: Id,
    pub body: Box<Formula>,
}
pub enum Strategy {
    /// Bound the margin within `precision` with at most `budget.boxes` enclosures (work units: deterministic).
    Prove { budget: Work, precision: f64 },
    /// n + 1 evenly spaced points over the one range of a one-quantifier formula; never a proof.
    Sample { n: u32 },
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Verdict {
    pub truth: Truth,
    /// The twin's margin at the witness (NaN with no witness).
    pub margin: f32,
    /// Proven bounds on the quantified margin (sampled: the extreme seen, twice).
    pub bounds: [f64; 2],
    pub witness: Box<[(Slot, f32)]>,
    pub work: Work,
    pub scope: Scope,
}
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub enum Truth {
    Holds,
    Fails,
    Undecided(Why),
}
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub enum Why {
    Budget(Work),
    /// An operator with no finite enclosure over the boxes that could hold the extreme (the register and why).
    Codomain(#[serde(serialize_with = "unbounded")] Unbounded),
    /// The boxes that could hold the extreme are one f32 apart and still straddle the answer.
    WithinPrecision,
}
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub enum Scope {
    Proved,
    Sampled { n: u32 },
    Observed { ticks: u32, inputs: [u8; 32], law: [u8; 32], by: Evaluator },
}
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub enum Evaluator {
    Gpu,
    Twin,
}

/// Each element is the signed margin of the observed site at that tick. Refuse missing or nonfinite evidence.
pub fn observed(margins: &[Option<f32>], inputs: [u8; 32], law: [u8; 32], by: Evaluator) -> Result<Verdict, Error> {
    if margins.is_empty() || margins.iter().flatten().any(|m| !m.is_finite()) {
        return Err(Error::Usage(
            "an observed check needs a finite margin at every sampled tick: supply the missing site".into(),
        ));
    }
    let (tick, margin) = margins
        .iter()
        .enumerate()
        .filter_map(|(k, v)| v.map(|m| (k, m)))
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .ok_or_else(|| {
            Error::Usage("the predicate selected no observed ticks: choose a take containing the hold".into())
        })?;
    Ok(Verdict {
        truth: if margin >= 0.0 { Truth::Holds } else { Truth::Fails },
        margin,
        bounds: [f64::from(margin); 2],
        witness: Box::new([(0, tick as f32)]),
        work: Work::default(),
        scope: Scope::Observed { ticks: margins.len() as u32, inputs, law, by },
    })
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
pub struct Work {
    pub boxes: u32,
}
/// The register and the reason, as text: `interval` keeps its types free of serde.
fn unbounded<S: Serializer>(u: &Unbounded, s: S) -> Result<S::Ok, S::Error> {
    s.serialize_str(&format!("register {}: {:?}", u.at, u.why))
}

/// Decides `c` over `ssa` with every leaf no quantifier binds held at `held[slot]`.
pub fn check(ssa: &Ssa, held: &[f32], c: &Check) -> Result<Verdict, Error> {
    let refuse =
        |what: String| Error::Scene { site: c.site.clone(), fault: SceneFault::Unsupported { what: what.into() } };
    if held.len() < ssa.slots() {
        return Err(refuse(format!("a check reading leaf slot {} with {} held", ssa.slots() - 1, held.len())));
    }
    let (mut bound, mut atoms) = (BTreeSet::new(), vec![]);
    scan(&c.body, &mut vec![], &mut bound, &mut atoms)
        .map_err(|s| refuse(format!("leaf slot {s} bound twice on one path: drop one of the ranges")))?;
    let mut regs = vec![];
    tape::run(ssa, 1, held.len(), held, &mut regs).map_err(|u| refuse(format!("{u:?}")))?;
    let mut err = None;
    walk(&c.body, &mut |q: &Quant| {
        let end = |r: Id| match reads(ssa, r).into_iter().find(|s| bound.contains(s)) {
            Some(s) => Err(refuse(format!(
                "a range end that reads the bound leaf slot {s}; use ends fixed throughout the search"
            ))),
            None => Ok(regs[r as usize] as f64),
        };
        match (end(q.lo), end(q.hi)) {
            (Ok(lo), Ok(hi)) if lo.is_finite() && hi.is_finite() && lo <= hi => {}
            (Ok(lo), Ok(hi)) => {
                err = err
                    .take()
                    .or(Some(refuse(format!("the range of slot {} is [{lo}, {hi}]: finite ends, lo <= hi", q.leaf))))
            }
            (Err(e), _) | (_, Err(e)) => err = err.take().or(Some(e)),
        }
    });
    if let Some(e) = err {
        return Err(e);
    }
    match c.how {
        Strategy::Sample { n } => sample(ssa, held, &c.body, &regs, n)
            .ok_or_else(|| refuse("a sampled check that is not one range over a comparison".into())),
        Strategy::Prove { budget, precision } => {
            let mut s = Search::new(ssa, &regs, held.len(), bound, atoms, budget.boxes, precision);
            let env: Vec<Arg> = held.iter().map(|&v| Arg::Box([v as f64; 2])).collect();
            let f = s
                .bound(&c.body, &env, true, Effort::Point, &Hint::default(), true)
                .unwrap_or(Found::unknown(Spent::Boxes));
            let work = Work { boxes: s.boxes };
            let truth = match (f.truth, f.spent) {
                (Tri::True, _) => Truth::Holds,
                (Tri::False, _) => Truth::Fails,
                (Tri::Either, Spent::Boxes) => Truth::Undecided(Why::Budget(work)),
                (Tri::Either, Spent::Width(Some(u))) => Truth::Undecided(Why::Codomain(u)),
                (Tri::Either, Spent::Width(None) | Spent::Finished) => Truth::Undecided(Why::WithinPrecision),
            };
            let witness =
                if f.margin.is_nan() { Box::default() } else { f.at.iter().map(|&(k, v)| (k, v as f32)).collect() };
            Ok(Verdict { truth, margin: f.margin as f32, bounds: [f.lo, f.hi], witness, work, scope: Scope::Proved })
        }
    }
}

fn walk(f: &Formula, each: &mut impl FnMut(&Quant)) {
    match f {
        Formula::Atom { .. } => {}
        Formula::Always(q) | Formula::Eventually(q) => {
            each(q);
            walk(&q.body, each)
        }
        Formula::And(fs) | Formula::Or(fs) => fs.iter().for_each(|f| walk(f, each)),
        Formula::Not(f) => walk(f, each),
    }
}
/// Every bound slot (refusing one bound twice on one path) and every atom's margin.
fn scan(f: &Formula, path: &mut Vec<Slot>, bound: &mut BTreeSet<Slot>, atoms: &mut Vec<Id>) -> Result<(), Slot> {
    match f {
        Formula::Atom { margin, .. } => {
            atoms.push(*margin);
            Ok(())
        }
        Formula::Always(q) | Formula::Eventually(q) => {
            if path.contains(&q.leaf) {
                return Err(q.leaf);
            }
            path.push(q.leaf);
            bound.insert(q.leaf);
            let r = scan(&q.body, path, bound, atoms);
            path.pop();
            r
        }
        Formula::And(fs) | Formula::Or(fs) => fs.iter().try_for_each(|f| scan(f, path, bound, atoms)),
        Formula::Not(f) => scan(f, path, bound, atoms),
    }
}
/// The registers `r` reads, itself included.
fn cone(ssa: &Ssa, r: Id) -> Vec<bool> {
    let mut seen = vec![false; r as usize + 1];
    seen[r as usize] = true;
    for i in (0..=r as usize).rev() {
        if let (true, Ins::Op(a, x)) = (seen[i], ssa.ins[i]) {
            x[..prim::atom(a).arity as usize].iter().for_each(|&k| seen[k as usize] = true);
        }
    }
    seen
}
/// The leaf slots register `r` reads.
fn reads(ssa: &Ssa, r: Id) -> Vec<Slot> {
    let (mut seen, mut todo, mut out) = (vec![false; r as usize + 1], vec![r], vec![]);
    while let Some(i) = todo.pop() {
        if std::mem::replace(&mut seen[i as usize], true) {
            continue;
        }
        match ssa.ins[i as usize] {
            Ins::Lit(_) => {}
            Ins::Leaf(s) => out.push(s),
            Ins::Op(a, x) => todo.extend(&x[..prim::atom(a).arity as usize]),
        }
    }
    out
}

/// A one-range comparison at n + 1 points (`loft check`'s sampled scope).
fn sample(ssa: &Ssa, held: &[f32], f: &Formula, at: &[f32], n: u32) -> Option<Verdict> {
    let (q, sign) = match f {
        Formula::Always(q) => (q, 1.0),
        Formula::Eventually(q) => (q, -1.0),
        Formula::Atom { .. } | Formula::And(_) | Formula::Or(_) | Formula::Not(_) => return None,
    };
    let (margin, test) = match *q.body {
        Formula::Atom { margin, test } => (margin, test),
        Formula::Always(_) | Formula::Eventually(_) | Formula::And(_) | Formula::Or(_) | Formula::Not(_) => {
            return None;
        }
    };
    let [a, b] = [at[q.lo as usize] as f64, at[q.hi as usize] as f64];
    let n = if a == b { 0 } else { n.max(1) };
    let mut leaves = Vec::with_capacity(held.len() * (n as usize + 1));
    for i in 0..=n {
        let mut p = held.to_vec();
        p[q.leaf as usize] = if i == n { b as f32 } else { (a + (b - a) * i as f64 / n.max(1) as f64) as f32 };
        leaves.extend(p);
    }
    let lanes = n as usize + 1;
    let mut regs = vec![];
    tape::run(ssa, lanes, held.len(), &leaves, &mut regs)
        .expect("`check` refused a held vector shorter than the tape's slots");
    let (m, t) = (&regs[margin as usize * lanes..][..lanes], &regs[test as usize * lanes..][..lanes]);
    let best = (0..lanes).fold(0, |k, l| if sign * m[l] < sign * m[k] { l } else { k });
    let found = if sign > 0.0 { t.iter().all(|&v| v != 0.0) } else { t.iter().any(|&v| v != 0.0) };
    Some(Verdict {
        truth: if found { Truth::Holds } else { Truth::Fails },
        margin: m[best],
        bounds: [m[best] as f64; 2],
        witness: Box::new([(q.leaf, leaves[best * held.len() + q.leaf as usize])]),
        work: Work::default(),
        scope: Scope::Sampled { n: lanes as u32 },
    })
}

// ---- the search ---------------------------------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Debug)]
enum Tri {
    True,
    False,
    Either,
}
fn invert(t: Tri) -> Tri {
    match t {
        Tri::True => Tri::False,
        Tri::False => Tri::True,
        Tri::Either => Tri::Either,
    }
}
#[derive(Clone, Copy, PartialEq, Debug)]
enum Spent {
    Finished,
    Boxes,
    /// The boxes that could hold the extreme could not be split further (and the operator unbounded there, if one was).
    Width(Option<Unbounded>),
}
#[derive(Clone, Copy, PartialEq)]
enum Effort {
    Point,
    Box,
}
/// The budget ran out inside a nested search.
struct Stop;

/// A slot's value over a search box: an interval, or a line through `c` along outer slots (an inner witness that
/// moves with the outer search's leaves; `src/ranged.ts` `Track`).
#[derive(Clone, Debug)]
enum Arg {
    Box(I),
    Track { c: f64, s: Vec<(Slot, f64)> },
}
impl Arg {
    fn point(&self) -> bool {
        match self {
            Arg::Box(b) => b[0] == b[1],
            Arg::Track { .. } => false,
        }
    }
    fn mid(&self) -> f64 {
        match self {
            Arg::Box(b) => b[0] / 2.0 + b[1] / 2.0,
            Arg::Track { c, .. } => *c,
        }
    }
}
/// Where an enclosing search last found this one's extreme, and how each inner slot moves along outer slots.
#[derive(Clone, Default)]
struct Hint {
    at: Map<Slot, f64>,
    slopes: Map<Slot, Vec<(Slot, f64)>>,
}
#[derive(Clone)]
struct Found {
    lo: f64,
    hi: f64,
    truth: Tri,
    margin: f64,
    at: Vec<(Slot, f64)>,
    spent: Spent,
}
impl Found {
    fn unknown(spent: Spent) -> Found {
        Found { lo: f64::NEG_INFINITY, hi: f64::INFINITY, truth: Tri::Either, margin: f64::NAN, at: vec![], spent }
    }
}
/// A box of a search in min form (`always`'s sense).
#[derive(Clone)]
struct Cell {
    at: Vec<I>,
    lo: f64,
    hi: f64,
    sens: Vec<f64>,
    truth: Tri,
    margin: f64,
    why: Option<Unbounded>,
    inner: Map<Slot, f64>,
}

/// Pattern-search evaluations spent polishing a witness.
const POLISH: usize = 24;

struct Search<'a> {
    ssa: &'a Ssa,
    /// `ssa` over raw slots: each bound slot k reads x_k = R_k + sum_j Q_kj (R_j - M_j) (Q, M are extra slots, 0
    /// unless k is a `Track`). One program, so a track keeps its correlation (d/dy |t(y) - 1.7 y| along
    /// t = c + 1.7 (y - m) is sign(..) * 0, not sign * 1.7 - sign * 1.7).
    pre: Ssa,
    /// `pre` with d(margin)/d(R_j) appended (enclosed apart: a derivative may be unbounded where the value is not).
    ext: Ssa,
    /// Old register -> `pre` register.
    map: Vec<Id>,
    /// (margin, R_j) -> derivative register in `pre`.
    d: Map<(Id, Slot), Id>,
    /// Per margin, the `pre` registers it reads (a jump elsewhere, in the test, does not void its centred form).
    cone: Map<Id, Vec<bool>>,
    /// Extra slots: Q_kj and M_j.
    q: Map<(Slot, Slot), Slot>,
    m: Map<Slot, Slot>,
    /// Leaf slots of `pre`: the held slots and the extra ones.
    width: usize,
    /// Every register at the held values: the range ends.
    at: &'a [f32],
    limit: u32,
    boxes: u32,
    precision: f64,
}

impl<'a> Search<'a> {
    fn new(
        ssa: &'a Ssa,
        at: &'a [f32],
        held: usize,
        bound: BTreeSet<Slot>,
        atoms: Vec<Id>,
        limit: u32,
        precision: f64,
    ) -> Self {
        let (mut pre, mut q, mut m) = (Ssa::default(), Map::new(), Map::new());
        let mut next = held as Slot;
        let mut fresh = || (next, next += 1).0;
        for &j in &bound {
            m.insert(j, fresh());
            for &k in &bound {
                if k != j {
                    q.insert((k, j), fresh());
                }
            }
        }
        let x: Map<Slot, Id> = bound
            .iter()
            .map(|&k| {
                let mut v = pre.leaf(k);
                for &j in bound.iter().filter(|&&j| j != k) {
                    let (r, mj, qk) = (pre.leaf(j), pre.leaf(m[&j]), pre.leaf(q[&(k, j)]));
                    let dy = pre.op(Atom::Sub, [r, mj, 0]);
                    let t = pre.op(Atom::Mul, [qk, dy, 0]);
                    v = pre.op(Atom::Add, [v, t, 0]);
                }
                (k, v)
            })
            .collect();
        // Two rewrites that keep the twin's value and let the enclosure see what interval.rs's per-op rules cannot:
        // x * x as |x| * |x| (`mul` sees two intervals, not one register twice), and the operand of a sqrt that is
        // a sum of such squares held at >= 0 (the fold's f32 pad takes [0, w] to [-2^-148, w], and sqrt refused it:
        // every distance whose difference met 0 was OutsideDomain). `nonneg` is that structural fact per register.
        let (mut map, mut nonneg) = (Vec::with_capacity(ssa.ins.len()), Vec::with_capacity(ssa.ins.len()));
        for i in &ssa.ins {
            let (r, n) = match *i {
                Ins::Lit(v) => (pre.lit(v), v >= 0.0),
                Ins::Leaf(k) => (x.get(&k).copied().unwrap_or_else(|| pre.leaf(k)), false),
                Ins::Op(Atom::Mul, [p, r, _]) if p == r => {
                    let a = pre.op(Atom::Abs, [map[p as usize], 0, 0]);
                    (pre.op(Atom::Mul, [a, a, 0]), true)
                }
                Ins::Op(Atom::Sqrt, [p, _, _]) if nonneg[p as usize] => {
                    let z = pre.lit(0.0);
                    let c = pre.op(Atom::Max, [map[p as usize], z, 0]);
                    (pre.op(Atom::Sqrt, [c, 0, 0]), true)
                }
                Ins::Op(a, [p, r, t]) => {
                    let n = match a {
                        Atom::Add | Atom::Mul => nonneg[p as usize] && nonneg[r as usize],
                        Atom::Abs | Atom::Sqrt | Atom::Exp | Atom::Exp2 | Atom::Cosh => true,
                        Atom::Sub
                        | Atom::Div
                        | Atom::Neg
                        | Atom::Min
                        | Atom::Max
                        | Atom::Pow
                        | Atom::Atan2
                        | Atom::Rem
                        | Atom::Sign => false,
                        Atom::Floor
                        | Atom::Ceil
                        | Atom::Trunc
                        | Atom::RoundEven
                        | Atom::Log
                        | Atom::Log2
                        | Atom::Sin
                        | Atom::Cos => false,
                        Atom::Tan
                        | Atom::Asin
                        | Atom::Acos
                        | Atom::Atan
                        | Atom::Sinh
                        | Atom::Tanh
                        | Atom::Lt
                        | Atom::Gt
                        | Atom::Le => false,
                        Atom::Ge | Atom::Eq | Atom::Ne | Atom::And | Atom::Or | Atom::Not | Atom::Select => false,
                    };
                    (pre.op(a, [map[p as usize], map[r as usize], map[t as usize]]), n)
                }
            };
            map.push(r);
            nonneg.push(n);
        }
        let roots: Vec<Id> = atoms.iter().map(|&a| map[a as usize]).collect();
        let (mut ext, mut d) = (pre.clone(), Map::new());
        for &j in &bound {
            for (&a, g) in atoms.iter().zip(grad(&mut ext, &roots, j)) {
                d.insert((a, j), g);
            }
        }
        let cone = atoms.iter().map(|&a| (a, cone(&pre, map[a as usize]))).collect();
        Search { ssa, pre, ext, map, d, cone, q, m, width: next as usize, at, limit, boxes: 0, precision }
    }
    fn charge(&mut self) -> Result<(), Stop> {
        if self.boxes >= self.limit {
            return Err(Stop);
        }
        self.boxes += 1;
        Ok(())
    }

    /// Bounds on `f` over `env`. `top`: no enclosing search, so a budget stop keeps this search's bounds.
    fn bound(
        &mut self,
        f: &Formula,
        env: &[Arg],
        outer_point: bool,
        effort: Effort,
        hint: &Hint,
        top: bool,
    ) -> Result<Found, Stop> {
        let fold = |s: &mut Self, fs: &[Formula], sign: f64| -> Result<Found, Stop> {
            let mut out = Found {
                lo: sign * f64::INFINITY,
                hi: sign * f64::INFINITY,
                truth: if sign > 0.0 { Tri::True } else { Tri::False },
                margin: f64::NAN,
                at: vec![],
                spent: Spent::Finished,
            };
            for f in fs {
                let g = s.bound(f, env, outer_point, effort, hint, top)?;
                let pick = |a: f64, b: f64| if sign * a < sign * b { a } else { b };
                (out.lo, out.hi) = (pick(out.lo, g.lo), pick(out.hi, g.hi));
                out.margin =
                    if out.margin.is_nan() || sign * g.margin < sign * out.margin { g.margin } else { out.margin };
                // and: any false is false, all true is true; or is its dual.
                let (win, lose) = if sign > 0.0 { (Tri::False, Tri::True) } else { (Tri::True, Tri::False) };
                out.truth = if out.truth == win || g.truth == win {
                    win
                } else if out.truth == lose && g.truth == lose {
                    lose
                } else {
                    Tri::Either
                };
                out.at.extend(g.at);
                out.spent = if out.spent == Spent::Finished { g.spent } else { out.spent };
            }
            Ok(out)
        };
        match f {
            Formula::Atom { margin, test } => {
                self.charge()?;
                let c = self.atom(*margin, *test, env, &[]);
                Ok(Found { lo: c.lo, hi: c.hi, truth: c.truth, margin: c.margin, at: vec![], spent: Spent::Finished })
            }
            Formula::Always(q) | Formula::Eventually(q) => {
                let always = matches!(f, Formula::Always(_));
                // A run of one kind is one search over the joint box.
                let (mut leaves, mut body) = (vec![q], &*q.body);
                while let (Formula::Always(q), true) | (Formula::Eventually(q), false) = (body, always) {
                    leaves.push(q);
                    body = &q.body;
                }
                self.search(always, &leaves, body, env, outer_point, effort, hint, top)
            }
            Formula::And(fs) => fold(self, fs, 1.0),
            Formula::Or(fs) => fold(self, fs, -1.0),
            Formula::Not(f) => {
                let g = self.bound(f, env, outer_point, effort, hint, top)?;
                Ok(Found { lo: -g.hi, hi: -g.lo, truth: invert(g.truth), margin: -g.margin, ..g })
            }
        }
    }

    /// An atom over `env` (min form, unsigned): its bounds, truth, the twin's margin at a point, and per `leaves`
    /// slot its sensitivity (|D| times the width).
    fn atom(&self, margin: Id, test: Id, env: &[Arg], leaves: &[Slot]) -> Cell {
        let none = |why| Cell {
            at: vec![],
            lo: f64::NEG_INFINITY,
            hi: f64::INFINITY,
            sens: vec![],
            truth: Tri::Either,
            margin: f64::NAN,
            why,
            inner: Map::new(),
        };
        if env.iter().all(Arg::point) {
            let p: Vec<f32> = env.iter().map(|a| a.mid() as f32).collect();
            let mut regs = vec![];
            tape::run(self.ssa, 1, p.len(), &p, &mut regs)
                .expect("`check` refused a held vector shorter than the tape's slots");
            let (m, t) = (regs[margin as usize] as f64, regs[test as usize]);
            let truth = if m > 0.0 {
                Tri::True
            } else if m < 0.0 {
                Tri::False
            } else if t != 0.0 {
                Tri::True
            } else {
                Tri::False
            };
            return Cell { at: vec![], lo: m, hi: m, sens: vec![], truth, margin: m, why: None, inner: Map::new() };
        }
        let half = |k: Slot| match &env[k as usize] {
            Arg::Box(b) => (b[1] - b[0]) / 2.0,
            Arg::Track { .. } => 0.0,
        };
        let x = self.leaves(env, |a| match a {
            Arg::Box(b) => *b,
            Arg::Track { c, .. } => [*c; 2],
        });
        let nat = match interval::enclose(&self.pre, &x) {
            Ok(v) => v,
            // Report the caller's register: the first original instruction at or after the prelude's.
            Err(u) => {
                return none(Some(Unbounded {
                    at: self.map.iter().position(|&r| r >= u.at).expect("the prelude ends at the last mapped register")
                        as Id,
                    ..u
                }));
            }
        };
        let (pm, pt) = (self.map[margin as usize], self.map[test as usize]);
        let [mut lo, mut hi] = nat[pm as usize];
        let mut sens = vec![0.0; leaves.len()];
        if let Some((cen, d)) = self.centred(margin, env, &x, &nat) {
            (lo, hi) = (lo.max(cen[0]), hi.min(cen[1]));
            for (k, &s) in leaves.iter().enumerate() {
                sens[k] = d.get(&s).map_or(0.0, |i| i[0].abs().max(i[1].abs()) * 2.0 * half(s));
            }
        }
        let t = nat[pt as usize];
        let truth = if lo > 0.0 {
            Tri::True
        } else if hi < 0.0 {
            Tri::False
        } else if t[0] > 0.0 {
            Tri::True
        } else if t[1] == 0.0 {
            Tri::False
        } else {
            Tri::Either
        };
        Cell { at: vec![], lo, hi, sens, truth, margin: f64::NAN, why: None, inner: Map::new() }
    }

    /// The `pre` leaf vector: R_k from `r`, Q_kj the track slopes, M_j the box middles.
    fn leaves(&self, env: &[Arg], r: impl Fn(&Arg) -> I) -> Vec<I> {
        let mut x: Vec<I> = env.iter().map(r).collect();
        x.resize(self.width, [0.0; 2]);
        for (&j, &mj) in &self.m {
            x[mj as usize] = [env[j as usize].mid(); 2];
        }
        for (k, a) in env.iter().enumerate() {
            if let Arg::Track { s, .. } = a {
                for &(j, v) in s {
                    x[self.q[&(k as Slot, j)] as usize] = [v; 2];
                }
            }
        }
        x
    }

    /// The centred enclosure of the margin over `x` and its derivative per free (box) slot, where it is valid.
    fn centred(&self, margin: Id, env: &[Arg], x: &[I], nat: &[I]) -> Option<(I, Map<Slot, I>)> {
        if !continuous(&self.pre, nat, &self.cone[&margin]) {
            return None;
        }
        let pm = self.map[margin as usize];
        let mid = self.leaves(env, |a| [a.mid(); 2]);
        let p = interval::enclose(&self.pre, &mid).ok()?[pm as usize];
        let e = rounding(&self.pre, nat, pm)?;
        let ext = interval::enclose(&self.ext, x).ok()?;
        let mut total: Map<Slot, I> = Map::new();
        for (k, a) in env.iter().enumerate() {
            if let Arg::Box(b) = a
                && b[0] < b[1]
            {
                // Only bound slots are boxes wider than a point (held slots are [v, v]), and each has a derivative.
                total.insert(k as Slot, ext[self.d[&(margin, k as Slot)] as usize]);
            }
        }
        let (mut lo, mut hi) = (p[0] - e, p[1] + e);
        for (&j, d) in &total {
            let m = env[j as usize].mid();
            let w = [x[j as usize][0] - m, x[j as usize][1] - m];
            let p = [d[0] * w[0], d[0] * w[1], d[1] * w[0], d[1] * w[1]];
            lo += p.iter().copied().fold(f64::INFINITY, f64::min);
            hi += p.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        }
        // f64 rounding of the sum itself, far inside E's f32 scale but outward all the same.
        let r = (lo.abs().max(hi.abs())) * 1e-15;
        (lo.is_finite() && hi.is_finite()).then_some(([lo - r, hi + r], total))
    }

    #[allow(clippy::too_many_arguments)]
    fn search(
        &mut self,
        always: bool,
        quants: &[&Quant],
        body: &Formula,
        outer: &[Arg],
        outer_point: bool,
        effort: Effort,
        hint: &Hint,
        top: bool,
    ) -> Result<Found, Stop> {
        let sign = if always { 1.0 } else { -1.0 };
        let leaves: &[Slot] = &quants.iter().map(|q| q.leaf).collect::<Vec<_>>();
        let ranges: Vec<I> =
            quants.iter().map(|q| [self.at[q.lo as usize] as f64, self.at[q.hi as usize] as f64]).collect();
        let floor: Vec<f64> =
            ranges.iter().map(|r| ((r[1] - r[0]) * 1e-9).max(ulp(r[0].abs().max(r[1].abs())))).collect();
        let is_atom = matches!(body, Formula::Atom { .. });
        let middle = |at: &[I]| -> Vec<f64> { at.iter().map(|b| b[0] / 2.0 + b[1] / 2.0).collect() };
        // A point of this search's leaves over the outer box: a constant, or a line along the outer slots the hint
        // says it moves with, when that line stays inside the leaf's range.
        let point_of = |xs: &[f64], hint: &Hint| -> Vec<Arg> {
            xs.iter()
                .zip(leaves)
                .zip(&ranges)
                .map(|((&x, k), r)| {
                    let live: Vec<(Slot, f64)> = hint.slopes.get(k).map_or(vec![], |s| {
                        s.iter()
                            .filter(|(j, _)| !outer[*j as usize].point() && matches!(outer[*j as usize], Arg::Box(_)))
                            .copied()
                            .collect()
                    });
                    let reach: f64 = live
                        .iter()
                        .map(|&(j, s)| s.abs() * (x_of(&outer[j as usize])[1] - x_of(&outer[j as usize])[0]) / 2.0)
                        .sum();
                    if live.is_empty() || x - reach < r[0] || x + reach > r[1] {
                        Arg::Box([x; 2])
                    } else {
                        Arg::Track { c: x, s: live }
                    }
                })
                .collect()
        };
        let mut st = State { upper: f64::INFINITY, margin: f64::NAN, best: vec![], counterexample: false };

        let mut live: Vec<Cell> = vec![];
        let mut spent = Spent::Finished;
        let mut whole: Option<Cell> = None;
        let run = (|| -> Result<(), Stop> {
            let w = self.cell(
                sign,
                leaves,
                body,
                outer,
                outer_point,
                &ranges.iter().map(|&r| Arg::Box(r)).collect::<Vec<_>>(),
                ranges.iter().all(|r| r[0] == r[1]),
                hint,
                true,
            )?;
            whole = Some(w.clone());
            if leaves.iter().all(|k| hint.at.contains_key(k)) {
                let xs: Vec<f64> = leaves.iter().map(|k| hint.at[k]).collect();
                self.try_point(
                    &mut st,
                    sign,
                    leaves,
                    body,
                    outer,
                    outer_point,
                    &point_of(&xs, hint),
                    &xs,
                    hint,
                    is_atom,
                )?;
            }
            let from = Hint { at: w.inner.clone(), slopes: hint.slopes.clone() };
            let inner = self.try_point(
                &mut st,
                sign,
                leaves,
                body,
                outer,
                outer_point,
                &point_of(&middle(&ranges), hint),
                &middle(&ranges),
                &from,
                is_atom,
            )?;
            live = vec![Cell { inner, ..w }];
            // The corners: an extreme often sits at an end of the range, where no box's middle falls.
            if leaves.len() <= 4 {
                for bits in 0..1usize << leaves.len() {
                    let xs: Vec<f64> =
                        ranges.iter().enumerate().map(|(j, r)| if bits >> j & 1 == 1 { r[1] } else { r[0] }).collect();
                    let from = Hint { at: live[0].inner.clone(), slopes: hint.slopes.clone() };
                    self.try_point(
                        &mut st,
                        sign,
                        leaves,
                        body,
                        outer,
                        outer_point,
                        &point_of(&xs, hint),
                        &xs,
                        &from,
                        is_atom,
                    )?;
                }
            }
            // Polish: a pattern search downhill from the best point, so an enclosing search starts near the extreme.
            if leaves.iter().all(|k| st.best.iter().any(|(j, _)| j == k)) {
                let mut steps: Vec<f64> = ranges.iter().map(|r| (r[1] - r[0]) / 512.0).collect();
                let mut n = 0;
                while n < POLISH && steps.iter().zip(&floor).any(|(h, f)| h > f) {
                    for j in 0..ranges.len() {
                        let mut moved = false;
                        for dir in [1.0, -1.0] {
                            let mut here: Vec<f64> = leaves.iter().map(|k| get(&st.best, *k)).collect();
                            let x = (here[j] + dir * steps[j]).max(ranges[j][0]).min(ranges[j][1]);
                            if x == here[j] {
                                continue;
                            }
                            here[j] = x;
                            let was = st.upper;
                            let from = Hint { at: st.best.iter().copied().collect(), slopes: hint.slopes.clone() };
                            self.try_point(
                                &mut st,
                                sign,
                                leaves,
                                body,
                                outer,
                                outer_point,
                                &point_of(&here, hint),
                                &here,
                                &from,
                                is_atom,
                            )?;
                            n += 1;
                            if st.upper < was {
                                moved = true;
                                break;
                            }
                        }
                        steps[j] = if moved { steps[j] * 2.0 } else { steps[j] / 2.0 };
                    }
                }
            }
            let cap = match effort {
                Effort::Box => self.limit.min(self.boxes + 8),
                Effort::Point => self.limit,
            };
            loop {
                live.retain(|c| c.lo <= st.upper);
                if live.is_empty() {
                    break;
                }
                let k = (0..live.len()).fold(0, |k, i| if live[i].lo < live[k].lo { i } else { k });
                if st.upper - live[k].lo <= self.precision
                    && (truth(&st, whole.as_ref(), &live) != Tri::Either || effort == Effort::Box)
                {
                    break;
                }
                if effort == Effort::Box && self.boxes >= cap {
                    break;
                }
                let c = live[k].clone();
                // Split along the leaf the value is most sensitive to; without sensitivities, the widest relative to its range.
                let widths: Vec<f64> =
                    c.at.iter()
                        .enumerate()
                        .map(|(j, b)| {
                            if b[1] - b[0] > floor[j] {
                                (b[1] - b[0]) / (ranges[j][1] - ranges[j][0]).max(1e-300)
                            } else {
                                0.0
                            }
                        })
                        .collect();
                let by: Vec<f64> =
                    if c.sens.len() == widths.len() && c.sens.iter().zip(&widths).any(|(s, w)| *s > 0.0 && *w > 0.0) {
                        c.sens.iter().zip(&widths).map(|(s, w)| if *w > 0.0 { *s } else { -1.0 }).collect()
                    } else {
                        widths.clone()
                    };
                let j = (0..by.len()).fold(0, |j, i| if by[i] > by[j] { i } else { j });
                let [a, b] = c.at[j];
                let m = a / 2.0 + b / 2.0;
                // widths[j] is 0 when nothing can split (or NaN from an unbounded leaf): stop either way.
                if widths[j].is_nan() || widths[j] <= 0.0 || m <= a || m >= b {
                    spent = Spent::Width(c.why);
                    break;
                }
                let halves: Vec<Vec<I>> = [[a, m], [m, b]]
                    .iter()
                    .map(|h| c.at.iter().enumerate().map(|(i, x)| if i == j { *h } else { *x }).collect())
                    .collect();
                // The middles first: their inner witnesses start the halves' inner searches, and the two give the slope
                // with which the inner witness moves along the split leaf.
                let mut inners = vec![];
                for h in &halves {
                    let from = Hint { at: c.inner.clone(), slopes: hint.slopes.clone() };
                    inners.push(self.try_point(
                        &mut st,
                        sign,
                        leaves,
                        body,
                        outer,
                        outer_point,
                        &point_of(&middle(h), hint),
                        &middle(h),
                        &from,
                        is_atom,
                    )?);
                }
                let (x0, x1) = (middle(&halves[0])[j], middle(&halves[1])[j]);
                let mut slopes = hint.slopes.clone();
                for &inner in c.inner.keys() {
                    if let (Some(p), Some(q)) = (inners[0].get(&inner), inners[1].get(&inner))
                        && x1 != x0
                    {
                        let e = slopes.entry(inner).or_default();
                        e.retain(|(k, _)| *k != leaves[j]);
                        e.push((leaves[j], (q - p) / (x1 - x0)));
                    }
                }
                // Commit both children together: if the budget interrupts the split, the parent still covers them.
                let mut kids = vec![];
                for (h, inner) in halves.iter().zip(&inners) {
                    let from = Hint { at: inner.clone(), slopes: slopes.clone() };
                    let args: Vec<Arg> = h.iter().map(|&b| Arg::Box(b)).collect();
                    kids.push(Cell {
                        inner: inner.clone(),
                        ..self.cell(sign, leaves, body, outer, outer_point, &args, false, &from, true)?
                    });
                }
                live.splice(k..k + 1, kids);
            }
            Ok(())
        })();
        match run {
            Ok(()) => {}
            Err(Stop) if top => spent = Spent::Boxes,
            Err(Stop) => return Err(Stop),
        }
        let t = match spent {
            Spent::Boxes => Tri::Either,
            Spent::Finished | Spent::Width(_) => truth(&st, whole.as_ref(), &live),
        };
        let lower = whole.as_ref().map_or(f64::NEG_INFINITY, |_| live.iter().fold(st.upper, |lo, c| lo.min(c.lo)));
        let upper = st.upper.min(whole.as_ref().map_or(f64::INFINITY, |w| w.hi));
        Ok(if sign > 0.0 {
            Found { lo: lower, hi: upper, truth: t, margin: st.margin, at: st.best, spent }
        } else {
            Found { lo: -upper, hi: -lower, truth: invert(t), margin: st.margin, at: st.best, spent }
        })
    }

    /// The body's enclosure over a box of this search's leaves (min form).
    #[allow(clippy::too_many_arguments)]
    fn cell(
        &mut self,
        sign: f64,
        leaves: &[Slot],
        body: &Formula,
        outer: &[Arg],
        outer_point: bool,
        at: &[Arg],
        point: bool,
        from: &Hint,
        precise: bool,
    ) -> Result<Cell, Stop> {
        self.charge()?;
        let mut env = outer.to_vec();
        for (k, a) in leaves.iter().zip(at) {
            env[*k as usize] = a.clone();
        }
        let cell_at: Vec<I> = at.iter().map(x_of).collect();
        let c = match body {
            Formula::Atom { margin, test } => {
                let c = self.atom(*margin, *test, &env, leaves);
                let (lo, hi) = if sign > 0.0 { (c.lo, c.hi) } else { (-c.hi, -c.lo) };
                let truth = if sign > 0.0 { c.truth } else { invert(c.truth) };
                Cell { lo, hi, truth, inner: if c.why.is_some() { from.at.clone() } else { Map::new() }, ..c }
            }
            Formula::Always(_) | Formula::Eventually(_) | Formula::And(_) | Formula::Or(_) | Formula::Not(_) => {
                // Run to the precision only where every leaf is held to a point; over a box a few splits must do.
                let here = point && outer_point && precise;
                let f = self.bound(
                    body,
                    &env,
                    outer_point && point,
                    if here { Effort::Point } else { Effort::Box },
                    from,
                    false,
                )?;
                let (lo, hi) = if sign > 0.0 { (f.lo, f.hi) } else { (-f.hi, -f.lo) };
                let why = match f.spent {
                    Spent::Width(u) => u,
                    Spent::Finished | Spent::Boxes => None,
                };
                Cell {
                    at: vec![],
                    lo,
                    hi,
                    sens: vec![],
                    truth: if sign > 0.0 { f.truth } else { invert(f.truth) },
                    margin: f.margin,
                    why,
                    inner: f.at.into_iter().collect(),
                }
            }
        };
        Ok(Cell { at: cell_at, ..c })
    }

    /// Evaluates a point of this search's leaves (`args`, values `xs`); returns its inner witness.
    #[allow(clippy::too_many_arguments)]
    fn try_point(
        &mut self,
        st: &mut State,
        sign: f64,
        leaves: &[Slot],
        body: &Formula,
        outer: &[Arg],
        outer_point: bool,
        args: &[Arg],
        xs: &[f64],
        from: &Hint,
        is_atom: bool,
    ) -> Result<Map<Slot, f64>, Stop> {
        // An inner search runs to the precision only at a point that may lower `upper`: a few splits from `from` first.
        let mut p = self.cell(sign, leaves, body, outer, outer_point, args, true, from, is_atom)?;
        if !is_atom && p.lo < st.upper {
            let again = Hint { at: p.inner.clone(), slopes: from.slopes.clone() };
            p = self.cell(sign, leaves, body, outer, outer_point, args, true, &again, true)?;
        }
        if p.hi < st.upper || (p.hi == st.upper && p.truth == Tri::False && !st.counterexample) {
            st.upper = p.hi;
            st.margin = p.margin;
            st.best =
                leaves.iter().zip(xs).map(|(k, x)| (*k, *x)).chain(p.inner.iter().map(|(k, v)| (*k, *v))).collect();
        }
        if p.truth == Tri::False {
            st.counterexample = true;
        }
        Ok(p.inner)
    }
}

struct State {
    upper: f64,
    margin: f64,
    best: Vec<(Slot, f64)>,
    counterexample: bool,
}
fn truth(st: &State, whole: Option<&Cell>, live: &[Cell]) -> Tri {
    if st.counterexample
        || st.upper < 0.0
        || whole.is_some_and(|w| w.truth == Tri::False)
        || live.iter().any(|c| c.truth == Tri::False)
    {
        Tri::False
    } else if whole.is_some_and(|w| w.truth == Tri::True)
        || (!live.is_empty() && live.iter().all(|c| c.truth == Tri::True || c.lo > 0.0))
    {
        Tri::True
    } else {
        Tri::Either
    }
}
fn get(at: &[(Slot, f64)], k: Slot) -> f64 {
    at.iter().find(|(j, _)| *j == k).map_or(f64::NAN, |p| p.1)
}
fn x_of(a: &Arg) -> I {
    match a {
        Arg::Box(b) => *b,
        Arg::Track { c, .. } => [*c; 2],
    }
}
/// One f32 ulp at magnitude `x` (the least split worth making: the twin reads f32 leaves).
fn ulp(x: f64) -> f64 {
    let x = (x as f32).abs().max(f32::MIN_POSITIVE);
    (f32::from_bits(x.to_bits() + 1) - x) as f64
}

/// Every jump of the program is decided over the box (so the mean value form holds).
fn continuous(ssa: &Ssa, nat: &[I], cone: &[bool]) -> bool {
    let point = |i: Id| nat[i as usize][0] == nat[i as usize][1];
    let sure = |i: Id| {
        let c = nat[i as usize];
        c[0] > 0.0 || c[1] < 0.0 || (c[0] == 0.0 && c[1] == 0.0)
    };
    ssa.ins.iter().zip(cone).enumerate().all(|(r, (i, &live))| {
        !live
            || match *i {
                Ins::Lit(_) | Ins::Leaf(_) => true,
                Ins::Op(a, [x, y, _]) => match a {
                    Atom::Add
                    | Atom::Sub
                    | Atom::Mul
                    | Atom::Div
                    | Atom::Neg
                    | Atom::Min
                    | Atom::Max
                    | Atom::Pow
                    | Atom::Abs
                    | Atom::Sqrt
                    | Atom::Exp
                    | Atom::Exp2 => true,
                    Atom::Log
                    | Atom::Log2
                    | Atom::Sin
                    | Atom::Cos
                    | Atom::Tan
                    | Atom::Asin
                    | Atom::Acos
                    | Atom::Atan
                    | Atom::Sinh
                    | Atom::Cosh
                    | Atom::Tanh => true,
                    Atom::Sign | Atom::Floor | Atom::Ceil | Atom::Trunc | Atom::RoundEven => point(r as Id),
                    Atom::Lt | Atom::Gt | Atom::Le | Atom::Ge | Atom::Eq | Atom::Ne | Atom::Not => point(r as Id),
                    Atom::And | Atom::Or | Atom::Select => sure(x),
                    Atom::Rem => interval::div([nat[x as usize], nat[y as usize], [0.0; 2]])
                        .is_ok_and(|q| q[0].trunc() == q[1].trunc()),
                    Atom::Atan2 => {
                        let (yy, xx) = (nat[x as usize], nat[y as usize]);
                        !(yy[0] <= 0.0 && yy[1] >= 0.0 && xx[0] < 0.0)
                    }
                },
            }
    })
}

/// Each atom's program and its partial derivatives, built once from its own rules.
fn mini(a: Atom) -> &'static (Ssa, Vec<Id>) {
    static MINI: [OnceLock<(Ssa, Vec<Id>)>; 40] = [const { OnceLock::new() }; 40];
    MINI[a as usize].get_or_init(|| {
        let n = prim::atom(a).arity as u32;
        let mut s = Ssa::default();
        let mut x = [0; 3];
        for k in 0..n {
            x[k as usize] = s.leaf(k);
        }
        let r = s.op(a, x);
        let d = (0..n).map(|k| grad(&mut s, &[r], k)[0]).collect();
        (s, d)
    })
}
/// A bound on |twin - real| of register `at` over the box: the forward running error.
fn rounding(ssa: &Ssa, nat: &[I], at: Id) -> Option<f64> {
    let ins = &ssa.ins[..=at as usize];
    let mut e = vec![0.0; ins.len()];
    for (r, i) in ins.iter().enumerate() {
        e[r] = match *i {
            Ins::Lit(_) | Ins::Leaf(_) => 0.0,
            Ins::Op(a, x) => {
                let def = prim::atom(a);
                let v = nat[r];
                let pad = 2.0
                    * def.ulps as f64
                    * (v[0].abs().max(v[1].abs()) * f32::EPSILON as f64).max(1.401298464324817e-45);
                let (s, d) = mini(a);
                let args: Vec<I> = x[..def.arity as usize].iter().map(|&k| nat[k as usize]).collect();
                let need = x[..def.arity as usize].iter().any(|&k| e[k as usize] > 0.0);
                let carried = if need {
                    let p = interval::enclose(s, &args).ok()?;
                    d.iter()
                        .zip(&x)
                        .map(|(&di, &k)| p[di as usize][0].abs().max(p[di as usize][1].abs()) * e[k as usize])
                        .sum::<f64>()
                } else {
                    0.0
                };
                pad + carried
            }
        };
        if !e[r].is_finite() {
            return None;
        }
    }
    Some(e[at as usize])
}
