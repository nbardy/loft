//! `solve` (lean Rust plan §1.7, §3.4): unknown curves, each a uniform cubic B-spline of n numbers over the span,
//! chosen so the holds are met and the preference is least. The method is `src/geom/solve.ts`'s (its notes say why):
//! an augmented Lagrangian over the numbers, each subproblem by Newton steps with Levenberg-Marquardt damping; exact
//! first and second derivatives of every hold in the unknowns' jets (`grad`, twice), carried to the numbers by the
//! B-spline weights; interval holds held at samples, checked on a grid 64 times finer and exchanged where broken; a
//! conflict certified only between constant bounds on one jet over overlapping windows.
//!
//! New here. (1) The holds run in f64 through each atom's interval rule at a point box: the rule encloses the real
//! operation, so at a point it is that operation (DERIVATION; the fold's f32 pad is not applied). The f32 twin's noise,
//! 6e-8 relative, is above the 1e-8 the search aims for. A rule with no point answer there (a divisor at 0, rem,
//! atan2 on its cut) gives NaN, an error only where a root reads it, as in the JS twin. (2) The verdicts are `check`'s
//! proofs over the returned curve, which is `curve`: the shader's per-span polynomial of the f32 numbers, read at f32
//! times with the f32 frame defaults. solve2's fix (verify what is drawn, not the f64 search state) holds by
//! construction. A hold `check` cannot decide is `Undecided`; a proved break is an error. Nothing is passed unproved.
use crate::check::{Check, Formula, Quant, Slot, Strategy, Truth, Verdict, Work, check};
use crate::grad::grad;
use crate::prim::{self, Atom, Prim};
use crate::tape::{Id, Ins, Ssa};
use loft_ir::error::{Error, Meter, SceneFault};
use loft_ir::graph::Site;
use std::collections::HashMap;

pub struct Solve {
    /// The holds' margins, the starts and the near targets, over leaf slots.
    pub ssa: Ssa,
    /// Every leaf slot's f32 frame default (time and the jets are set while solving).
    pub held: Vec<f32>,
    pub time: Slot,
    /// Unknown k's d-th time derivative, at the time evaluated, is leaf slot `jets + 4k + d`.
    pub jets: Slot,
    pub over: [f64; 2],
    pub vary: Vec<Family>,
    pub holds: Vec<Hold>,
    pub prefer: Prefer,
    /// The interior target of every bound, in its margin's units (the TS default is 4e-6).
    pub delta: f64,
    /// Ceiling on hold evaluation in tape operations (instructions times lanes): deterministic, unlike wall time.
    pub tape_ops: u64,
    /// Each returned hold's proof: the `check` budget and precision.
    pub boxes: Work,
    pub precision: f64,
}
/// `n` numbers, the search starting from the B-spline nearest `from` (a register of time).
pub struct Family {
    pub n: usize,
    pub from: Id,
}
/// Weights of the integrals of squared jerk, squared acceleration, and squared distance from `near` (unknown, curve).
pub struct Prefer {
    pub jerk: f64,
    pub acceleration: f64,
    pub near_weight: f64,
    pub near: Vec<(usize, Id)>,
}
pub struct Hold {
    pub site: Site,
    pub kind: Kind,
    /// The margin's components over the jets: `equal`'s a - b, or a bound's amount over its limit.
    pub margin: Vec<Id>,
    /// The condition as written and its robustness margin: what `check` proves over `times` on the result.
    pub test: Id,
    pub robust: Id,
    pub times: [f64; 2],
}
pub enum Kind {
    /// Each component within ±within: a target, pinned to half the band while solving.
    Equal {
        within: f64,
    },
    Bound,
}
pub struct Solved {
    /// Each unknown's f32 numbers: `curve` of them is what is drawn.
    pub controls: Vec<Vec<f32>>,
    /// One per hold, in order: proved over its window on the returned curve, or `Undecided`.
    pub verdicts: Vec<Verdict>,
    pub rounds: u32,
    pub steps: u32,
}

const AIM: f64 = 1e-8;
/// Sampled residual above which the search diagnoses a conflict or samples a whole neighbourhood.
const MET: f64 = 1e-6;
/// A bound within this of its target between samples gets a sample at the dip.
const NEAR: f64 = 1e-3;
/// Gauss-Legendre, four points: exact for every squared spline derivative over a knot gap.
const GL: [(f64, f64); 4] = [
    (-0.8611363115940526, 0.3478548451374538),
    (-0.3399810435848563, 0.6521451548625461),
    (0.3399810435848563, 0.6521451548625461),
    (0.8611363115940526, 0.3478548451374538),
];

/// The returned curve at time register `t`: `src/curve.ts` `bspline`, a binary select over spans of one cubic each.
pub fn curve(s: &mut Ssa, t: Id, over: [f64; 2], c: &[f32]) -> Id {
    let n = c.len();
    let k = [over[0], (over[1] - over[0]) / (n - 3) as f64, 0.0, (n - 3) as f64].map(|v| s.lit(v as f32));
    let d = f(s, Prim::Sub, &[t, k[0]]);
    let d = f(s, Prim::Div, &[d, k[1]]);
    let tau = f(s, Prim::Clamp, &[d, k[2], k[3]]);
    piece(s, tau, c, 0, n - 4)
}
fn f(s: &mut Ssa, p: Prim, a: &[Id]) -> Id {
    s.apply(p, &a.iter().map(std::slice::from_ref).collect::<Vec<_>>())[0]
}
fn piece(s: &mut Ssa, tau: Id, c: &[f32], lo: usize, hi: usize) -> Id {
    if lo < hi {
        let mid = (lo + hi).div_ceil(2);
        let m = s.lit(mid as f32);
        let (when, a, b) = (f(s, Prim::Lt, &[tau, m]), piece(s, tau, c, lo, mid - 1), piece(s, tau, c, mid, hi));
        return f(s, Prim::Select, &[when, a, b]);
    }
    let [a, b, cc, d, six, one, first] =
        [c[lo], c[lo + 1], c[lo + 2], c[lo + 3], 6.0, 1.0, lo as f32].map(|v| s.lit(v));
    let (slope, ab, dc) = (f(s, Prim::Sub, &[cc, b]), f(s, Prim::Sub, &[b, a]), f(s, Prim::Sub, &[d, cc]));
    let (l, r) = (f(s, Prim::Sub, &[slope, ab]), f(s, Prim::Sub, &[dc, slope]));
    let (left, right, u) = (f(s, Prim::Div, &[l, six]), f(s, Prim::Div, &[r, six]), f(s, Prim::Sub, &[tau, first]));
    let v = f(s, Prim::Sub, &[one, u]);
    let cube = |s: &mut Ssa, x: Id| {
        let x2 = f(s, Prim::Mul, &[x, x]);
        f(s, Prim::Mul, &[x2, x])
    };
    let (v3, u3, m) = (cube(s, v), cube(s, u), f(s, Prim::Mix, &[b, cc, u]));
    let (lv, ru) = (f(s, Prim::Mul, &[left, v3]), f(s, Prim::Mul, &[right, u3]));
    let m = f(s, Prim::Add, &[m, lv]);
    f(s, Prim::Add, &[m, ru])
}

/// The cubic B-spline kernel's d-th derivative at x.
fn kernel(x: f64, d: usize) -> f64 {
    let (a, odd) = (x.abs(), if d % 2 == 1 && x < 0.0 { -1.0 } else { 1.0 });
    let o = 2.0 - a;
    odd * if a >= 2.0 {
        0.0
    } else if a < 1.0 {
        [(4.0 - 6.0 * a * a + 3.0 * a * a * a) / 6.0, (-12.0 * a + 9.0 * a * a) / 6.0, (-12.0 + 18.0 * a) / 6.0, 3.0][d]
    } else {
        [o * o * o / 6.0, -o * o / 2.0, o, -1.0][d]
    }
}

/// A program cut to `roots`: dead code dropped, common subexpressions merged, x * 1 and x + 0 folded (each `grad` pass
/// rebuilds the derivatives of what it shares with the others).
struct Prog {
    ssa: Ssa,
    roots: Vec<Id>,
}
fn compact(s: &Ssa, roots: &[Id]) -> Prog {
    let arity = |a: Atom| prim::atom(a).arity as usize;
    let mut live = vec![false; s.ins.len()];
    roots.iter().for_each(|&r| live[r as usize] = true);
    for i in (0..s.ins.len()).rev() {
        if let (true, Ins::Op(a, x)) = (live[i], s.ins[i]) {
            x[..arity(a)].iter().for_each(|&k| live[k as usize] = true);
        }
    }
    let (mut out, mut map, mut seen) = (Ssa::default(), vec![0; s.ins.len()], HashMap::new());
    for (i, &ins) in s.ins.iter().enumerate().filter(|(i, _)| live[*i]) {
        let ins = match ins {
            Ins::Op(a, x) => Ins::Op(a, std::array::from_fn(|j| if j < arity(a) { map[x[j] as usize] } else { 0 })),
            Ins::Lit(_) | Ins::Leaf(_) => ins,
        };
        let lit = |k: Id, v: f32| out.ins[k as usize] == Ins::Lit(v);
        map[i] = match ins {
            Ins::Op(Atom::Mul, [a, b, _]) if lit(a, 1.0) => b,
            Ins::Op(Atom::Mul, [a, b, _]) if lit(b, 1.0) => a,
            Ins::Op(Atom::Add, [a, b, _]) if lit(a, 0.0) => b,
            Ins::Op(Atom::Add, [a, b, _]) if lit(b, 0.0) => a,
            Ins::Op(..) | Ins::Lit(_) | Ins::Leaf(_) => {
                let key = match ins {
                    Ins::Lit(v) => (u8::MAX, v.to_bits(), 0, 0),
                    Ins::Leaf(k) => (u8::MAX - 1, k, 0, 0),
                    Ins::Op(a, x) => (a as u8, x[0], x[1], x[2]),
                };
                *seen.entry(key).or_insert_with(|| {
                    out.ins.push(ins);
                    (out.ins.len() - 1) as Id
                })
            }
        };
    }
    Prog { roots: roots.iter().map(|&r| map[r as usize]).collect(), ssa: out }
}

/// Runs `p` in f64 for `lanes` leaf vectors of width `w`; register r of lane l is `regs[r * lanes + l]`.
fn eval(p: &Ssa, lanes: usize, w: usize, leaves: &[f64], regs: &mut Vec<f64>) {
    regs.clear();
    regs.resize(p.ins.len() * lanes, 0.0);
    for (r, ins) in p.ins.iter().enumerate() {
        let (done, out) = regs.split_at_mut(r * lanes);
        for (l, o) in out[..lanes].iter_mut().enumerate() {
            *o = match *ins {
                Ins::Lit(v) => v as f64,
                Ins::Leaf(s) => leaves[l * w + s as usize],
                Ins::Op(a, x) => {
                    match (prim::atom(a).iv)(x.map(|k| [done.get(k as usize * lanes + l).copied().unwrap_or(0.0); 2])) {
                        Ok([lo, hi]) if lo == hi => lo,
                        Ok(_) | Err(_) => f64::NAN,
                    }
                }
            };
        }
    }
}

/// One hold compiled: the margin alone, and the margin with its gradient and Hessian in the jets it reads.
struct Line<'a> {
    hold: &'a Hold,
    size: usize,
    /// Pinned half-band of an `equal`.
    band: Option<f64>,
    reads: Vec<(usize, usize)>,
    pairs: Vec<(usize, usize)>,
    value: Prog,
    grad: Prog,
    samples: Vec<f64>,
    dense: Vec<f64>,
    /// Constant bounds on a jet the authored hold states: (slot, low, high).
    cert: Vec<(Slot, f64, f64)>,
}
/// One scalar row at x: its value, and its gradient and Hessian on the numbers it touches (`idx`).
struct Row {
    v: f64,
    band: Option<f64>,
    idx: Vec<usize>,
    g: Vec<f64>,
    m: Vec<f64>,
}
struct Problem<'a> {
    s: &'a Solve,
    /// Each unknown's first number in x, count and knot gap.
    chan: Vec<(usize, usize, f64)>,
    n: usize,
    q: Vec<f64>,
    b: Vec<f64>,
    /// The multipliers by (line, sample time, component), kept across exchange rounds, and the penalty weight.
    mult: HashMap<(usize, u64, usize), f64>,
    rho: f64,
    steps: u32,
    spent: u64,
    regs: Vec<f64>,
}

fn scene(site: &Site, fault: SceneFault) -> Error {
    Error::Scene { site: Some(site.clone()), fault }
}
fn unsolved(h: &Hold, fix: String) -> Error {
    scene(&h.site, SceneFault::Unsolved { fix: fix.into() })
}

impl Problem<'_> {
    /// The first of the four numbers of unknown k that reach t, and their weights for each jet (d-th derivative, in
    /// knot gaps: divide by h^d). Every sum and product below is in `solve.ts`'s order: at rho 1e12 the Newton path
    /// turns on the last bits (heron moved took 6 rounds against TS's 4 with sums reordered).
    fn place(&self, k: usize, t: f64) -> (usize, [[f64; 4]; 4]) {
        let (_, n, h) = self.chan[k];
        let raw = ((t as f32) as f64 - self.s.over[0]) / h;
        let tau = raw.max(0.0).min((n - 3) as f64);
        let first = (tau.floor() as usize).min(n - 4);
        // At a knot the piecewise cubic takes the right span (the last span at the upper end); outside, it is constant.
        let w = |d: usize, q: usize| match d {
            _ if d > 0 && raw != tau => 0.0,
            3 => [-1.0, 3.0, -3.0, 1.0][q],
            _ => kernel(tau - (first + q) as f64 + 1.0, d),
        };
        (first, std::array::from_fn(|d| std::array::from_fn(|q| w(d, q))))
    }

    /// Runs `p` at times `ts` with the jets of x; every root must be finite.
    fn run(&mut self, p: &Prog, h: &Hold, ts: &[f64], x: &[f64]) -> Result<(), Error> {
        let (w, s) = (self.s.held.len(), self.s);
        let mut leaves: Vec<f64> = Vec::with_capacity(ts.len() * w);
        for &t in ts {
            let at = leaves.len();
            leaves.extend(s.held.iter().map(|&v| v as f64));
            leaves[at + s.time as usize] = (t as f32) as f64;
            for k in 0..self.chan.len() {
                let (first, wt) = self.place(k, t);
                for (d, wd) in wt.iter().enumerate() {
                    let v: f64 = (0..4).map(|q| x[self.chan[k].0 + first + q] * wd[q]).sum();
                    leaves[at + s.jets as usize + 4 * k + d] = v / self.chan[k].2.powf(d as f64);
                }
            }
        }
        self.spent += (p.ssa.ins.len() * ts.len()) as u64;
        if self.spent > s.tape_ops {
            return Err(Error::Budget { meter: Meter::TapeOps, limit: s.tape_ops, spent: self.spent });
        }
        eval(&p.ssa, ts.len(), w, &leaves, &mut self.regs);
        let fine =
            p.roots.iter().all(|&r| self.regs[r as usize * ts.len()..][..ts.len()].iter().all(|v| v.is_finite()));
        if fine { Ok(()) } else { Err(scene(&h.site, SceneFault::Unreadable)) }
    }

    /// Every row of `lines` at x, with gradients and Hessians when asked (`solve.ts` `Problem.rows`).
    fn rows(&mut self, x: &[f64], lines: &[Line], grad: bool) -> Result<Vec<Row>, Error> {
        let mut out = vec![];
        for line in lines {
            let p = if grad { &line.grad } else { &line.value };
            self.run(p, line.hold, &line.samples, x)?;
            let lanes = line.samples.len();
            for (l, &t) in line.samples.iter().enumerate() {
                let at = |i: usize| self.regs[p.roots[i] as usize * lanes + l];
                let mut chans: Vec<usize> = vec![];
                line.reads.iter().for_each(|&(k, _)| {
                    if !chans.contains(&k) {
                        chans.push(k)
                    }
                });
                let places: Vec<_> = chans.iter().map(|&k| self.place(k, t)).collect();
                let idx: Vec<usize> = chans
                    .iter()
                    .zip(&places)
                    .flat_map(|(&k, w)| self.chan[k].0 + w.0..self.chan[k].0 + w.0 + 4)
                    .collect();
                // Each jet read: where its unknown's four numbers sit in `idx`, and their weights.
                let w: Vec<(usize, [f64; 4])> = line
                    .reads
                    .iter()
                    .map(|&(k, d)| {
                        let at = chans.iter().take_while(|&&c| c != k).count();
                        (4 * at, places[at].1[d].map(|v| v / self.chan[k].2.powf(d as f64)))
                    })
                    .collect();
                for c in 0..line.size {
                    let mut row = Row { v: at(c), band: line.band, idx: vec![], g: vec![], m: vec![] };
                    if grad {
                        let (ll, nr) = (idx.len(), line.reads.len());
                        row.g = vec![0.0; ll];
                        row.m = vec![0.0; ll * ll];
                        for (r, &(o, wr)) in w.iter().enumerate() {
                            let dv = at((1 + r) * line.size + c);
                            (0..4).for_each(|q| row.g[o + q] += dv * wr[q]);
                        }
                        for (r, &(i, j)) in line.pairs.iter().enumerate() {
                            let dv = at((1 + nr + r) * line.size + c);
                            let ((pi, wi), (pj, wj)) = (w[i], w[j]);
                            for (a, wa) in wi.iter().enumerate() {
                                for (b, wb) in wj.iter().enumerate() {
                                    row.m[(pi + a) * ll + pj + b] += dv * wa * wb;
                                    if i != j {
                                        row.m[(pj + b) * ll + pi + a] += dv * wa * wb;
                                    }
                                }
                            }
                        }
                        if !row.g.iter().chain(&row.m).all(|v| v.is_finite()) {
                            return Err(scene(&line.hold.site, SceneFault::Unreadable));
                        }
                        row.idx = idx.clone();
                    }
                    out.push(row);
                }
            }
        }
        Ok(out)
    }

    /// The augmented Lagrangian at y, with its gradient and Hessian when asked.
    fn terms(
        &mut self,
        y: &[f64],
        grad: bool,
        lines: &[Line],
        slack: &[f64],
        mult: &[f64],
    ) -> Result<(f64, Vec<f64>, Vec<f64>), Error> {
        let (n, rr) = (self.n, self.rho);
        let rows = self.rows(y, lines, grad)?;
        let qy: Vec<f64> = (0..n).map(|i| (0..n).map(|j| self.q[i * n + j] * y[j]).sum::<f64>()).collect();
        let mut f = (0..n).map(|i| 0.5 * y[i] * qy[i] - self.b[i] * y[i]).sum::<f64>();
        let (mut g, mut h): (Vec<f64>, _) = match grad {
            true => {
                ((0..n).map(|i| (0..n).fold(-self.b[i], |s, j| s + self.q[i * n + j] * y[j])).collect(), self.q.clone())
            }
            false => (vec![], vec![]),
        };
        let (eq, ge) = (
            rows.iter().enumerate().filter(|r| r.1.band.is_some()),
            rows.iter().enumerate().filter(|r| r.1.band.is_none()),
        );
        for (k, row) in eq.chain(ge) {
            let z = match row.band {
                Some(_) => {
                    let res = row.v - slack[k];
                    f += mult[k] * res + 0.5 * rr * res * res;
                    mult[k] + rr * res
                }
                None => {
                    let s = (mult[k] - rr * (row.v - self.s.delta)).max(0.0);
                    f += (s * s - mult[k] * mult[k]) / (2.0 * rr);
                    -s
                }
            };
            if !grad || z == 0.0 && row.band.is_none() {
                continue;
            }
            let l = row.idx.len();
            for (a, &i) in row.idx.iter().enumerate().filter(|a| row.g[a.0] != 0.0) {
                g[i] += z * row.g[a];
                row.idx.iter().enumerate().for_each(|(b, &j)| h[i * n + j] += rr * row.g[a] * row.g[b]);
            }
            for (a, &i) in row.idx.iter().enumerate() {
                row.idx.iter().enumerate().for_each(|(b, &j)| h[i * n + j] += z * row.m[a * l + b]);
            }
        }
        Ok((f, g, h))
    }

    /// Newton steps with Levenberg-Marquardt damping until a step moves no number by 1e-9 relative or none lowers f.
    fn descend(&mut self, x: &mut Vec<f64>, lines: &[Line], slack: &[f64], mult: &[f64]) -> Result<(), Error> {
        let mut damp = 1e-9;
        for _ in 0..100 {
            let (f, g, h) = self.terms(x, true, lines, slack, mult)?;
            let mut moved = false;
            for _ in 0..40 {
                let mut d: Vec<f64> = g.iter().map(|v| -v).collect();
                if cholesky(&h, self.n, damp, &mut d) {
                    let (step, size) = (norm(&d), norm(x));
                    if step <= 1e-9 * (1.0 + size) {
                        return Ok(());
                    }
                    let y: Vec<f64> = x.iter().zip(&d).map(|(a, b)| a + b).collect();
                    self.steps += 1;
                    // Near the least a step's gain is below the rounding of f: accept one that does not raise f past it.
                    if self.terms(&y, false, lines, slack, mult)?.0 <= f + 1e-13 * f.abs() {
                        (*x, damp, moved) = (y, (damp / 8.0).max(1e-12), true);
                        break;
                    }
                }
                damp = (damp * 8.0).max(1e-9);
                if damp > 1e14 {
                    break;
                }
            }
            if !moved {
                return Ok(());
            }
        }
        Ok(())
    }

    /// The augmented Lagrangian's outer loop: returns the worst sampled violation (at most AIM when it settled).
    fn augmented(&mut self, x: &mut Vec<f64>, lines: &[Line]) -> Result<f64, Error> {
        let keys: Vec<_> = lines
            .iter()
            .enumerate()
            .flat_map(|(i, l)| {
                l.samples
                    .iter()
                    .flat_map(move |&t| (0..l.size).map(move |c| (i, if t == 0.0 { 0 } else { t.to_bits() }, c)))
            })
            .collect();
        let mut mult: Vec<f64> = keys.iter().map(|k| self.mult.get(k).copied().unwrap_or(0.0)).collect();
        let (mut last, mut v) = (f64::INFINITY, f64::INFINITY);
        for _ in 0..60 {
            let (rr, before) = (self.rho, x.clone());
            let rows = self.rows(x, lines, false)?;
            let slack: Vec<f64> =
                rows.iter().zip(&mult).map(|(r, &m)| r.band.map_or(0.0, |b| (r.v + m / rr).clamp(-b, b))).collect();
            self.descend(x, lines, &slack, &mult)?;
            v = 0.0;
            for (r, m) in self.rows(x, lines, false)?.iter().zip(&mut mult) {
                (v, *m) = match r.band {
                    Some(b) => (v.max(r.v.abs() - b), *m + rr * r.v - (*m + rr * r.v).clamp(-rr * b, rr * b)),
                    None => (v.max(self.s.delta - r.v), (*m - rr * (r.v - self.s.delta)).max(0.0)),
                };
            }
            let moved = norm(&x.iter().zip(&before).map(|(a, b)| a - b).collect::<Vec<_>>());
            if v <= AIM && moved <= 1e-6 * (1.0 + norm(x)) {
                break;
            }
            if v > 0.25 * last {
                self.rho = (self.rho * 10.0).min(1e12);
            }
            last = v;
        }
        self.mult.extend(keys.into_iter().zip(mult));
        Ok(v)
    }

    /// A conflict between constant bounds on one jet over overlapping windows, else the worst hold: the search failed.
    fn diagnose(&mut self, x: &[f64], lines: &[Line]) -> Result<(), Error> {
        let (mut seen, mut worst, mut which) = (vec![], f64::NEG_INFINITY, 0);
        for (li, line) in lines.iter().enumerate() {
            let within = line.band.map_or(0.0, |b| 2.0 * b);
            let rows = self.rows(x, std::slice::from_ref(line), false)?;
            let miss = rows.iter().fold(0.0f64, |w, r| w.max(if r.band.is_some() { r.v.abs() - within } else { -r.v }));
            if miss > worst {
                (worst, which) = (miss, li);
            }
            let infeasible = |a: &Line| {
                scene(&a.hold.site, SceneFault::Infeasible { a: a.hold.site.clone(), b: line.hold.site.clone() })
            };
            if line.reads.is_empty() && miss > 0.0 {
                return Err(infeasible(line));
            }
            for &(jet, lo, hi) in &line.cert {
                for &(j, other, olo, ohi) in &seen {
                    let o: &Line = &lines[other];
                    let overlap =
                        o.dense[0] <= line.dense[line.dense.len() - 1] && line.dense[0] <= o.dense[o.dense.len() - 1];
                    if j == jet && overlap && (lo > ohi || hi < olo) {
                        return Err(infeasible(o));
                    }
                }
                seen.push((jet, li, lo, hi));
            }
        }
        match worst > 0.0 {
            true => Err(unsolved(
                lines[which].hold,
                format!(
                    "the local search stopped {worst:.3e} short of this hold: give a starting curve (spline(n, from)) on the side it should pass"
                ),
            )),
            false => Ok(()),
        }
    }
}

fn norm(v: &[f64]) -> f64 {
    v.iter().fold(0.0, |m, x| m.max(x.abs()))
}
/// Cholesky solve of (A + damp I) x = r in place of r; false when not positive definite.
fn cholesky(a: &[f64], n: usize, damp: f64, r: &mut [f64]) -> bool {
    let mut l = vec![0.0; n * n];
    for i in 0..n {
        for j in 0..=i {
            let s = (0..j).fold(a[i * n + j] + if i == j { damp } else { 0.0 }, |s, k| s - l[i * n + k] * l[j * n + k]);
            if i == j && s <= 0.0 || s.is_nan() {
                return false;
            }
            l[i * n + j] = if i == j { s.sqrt() } else { s / l[j * n + j] };
        }
    }
    for i in 0..n {
        r[i] = (0..i).fold(r[i], |s, k| s - l[i * n + k] * r[k]) / l[i * n + i];
    }
    for i in (0..n).rev() {
        r[i] = (i + 1..n).fold(r[i], |s, k| s - l[k * n + i] * r[k]) / l[i * n + i];
    }
    true
}

/// The local peaks of f over a grid (ends included), each refined by golden-section search between its neighbours.
type Measure<'a> = dyn FnMut(&[f64]) -> Result<Vec<f64>, Error> + 'a;
fn peaks(f: &mut Measure, grid: &[f64]) -> Result<Vec<(f64, f64)>, Error> {
    let (v, g, mut out) = (f(grid)?, (5f64.sqrt() - 1.0) / 2.0, vec![]);
    for i in 0..grid.len() {
        if !(i == 0 || v[i] >= v[i - 1]) || !(i + 1 == grid.len() || v[i] >= v[i + 1]) {
            continue;
        }
        let (mut a, mut b) = (grid[i.saturating_sub(1)], grid[(i + 1).min(grid.len() - 1)]);
        let (mut c, mut d) = (b - g * (b - a), a + g * (b - a));
        let (mut fc, mut fd) = (f(&[c])?[0], f(&[d])?[0]);
        for _ in 0..30 {
            if b - a <= 1e-12 {
                break;
            }
            if fc >= fd {
                (b, d, fd, c) = (d, c, fc, d - g * (d - a));
                fc = f(&[c])?[0];
            } else {
                (a, c, fc, d) = (c, d, fd, c + g * (b - c));
                fd = f(&[d])?[0];
            }
        }
        out.push(
            [(grid[i], v[i]), (c, fc), (d, fd)].into_iter().fold((grid[i], v[i]), |w, p| if p.1 > w.1 { p } else { w }),
        );
    }
    Ok(out)
}

fn grid(a: f64, b: f64, step: f64) -> Vec<f64> {
    if a == b {
        return vec![a];
    }
    let m = ((b - a) / step - 1e-9).ceil().max(1.0) as usize;
    (0..=m).map(|i| if i == m { b } else { a + (b - a) * i as f64 / m as f64 }).collect()
}

/// Solves `s`: the curves, and each hold proved over its window on them.
pub fn solve(s: &Solve) -> Result<Solved, Error> {
    let refuse = |what: &str| Error::Scene { site: None, fault: SceneFault::Unsupported { what: what.into() } };
    let [t0, t1] = s.over;
    let p = &s.prefer;
    if !(t0.is_finite() && t1.is_finite() && t1 > t0) || !(s.delta.is_finite() && s.delta >= 0.0) {
        return Err(refuse("solve over [t0, t1] with t1 > t0 and a finite delta >= 0"));
    }
    if s.vary.is_empty()
        || s.vary.iter().any(|v| !(4..=64).contains(&v.n))
        || s.holds.is_empty()
        || p.near.iter().any(|z| z.0 >= s.vary.len())
    {
        return Err(refuse("solve needs unknowns of 4 to 64 numbers each, and holds"));
    }
    let weights = [p.jerk, p.acceleration, p.near_weight];
    if weights.iter().any(|w| !(w.is_finite() && *w >= 0.0))
        || !(p.jerk > 0.0 || p.acceleration > 0.0 || p.near_weight > 0.0 && !p.near.is_empty())
    {
        return Err(refuse("a preference with finite weights, one of them above 0: prefer: { jerk: 1 }"));
    }
    let mut chan = vec![];
    for v in &s.vary {
        chan.push((chan.last().map_or(0, |&(o, n, _): &(usize, usize, f64)| o + n), v.n, (t1 - t0) / (v.n - 3) as f64));
    }
    let n = s.vary.iter().map(|v| v.n).sum::<usize>();
    let h = chan.iter().fold(f64::INFINITY, |m, c| m.min(c.2));
    let jet =
        |slot: Slot| (slot >= s.jets && slot < s.jets + 4 * s.vary.len() as Slot).then(|| (slot - s.jets) as usize);
    let mut lines = vec![];
    for hold in &s.holds {
        let [a, b] = hold.times;
        if !(a.is_finite() && b.is_finite() && a <= b && a >= t0 - 1e-12 && b <= t1 + 1e-12) {
            let what = format!("a hold from {a} to {b}, outside the span [{t0}, {t1}]: widen `over`, or move the hold");
            return Err(scene(&hold.site, SceneFault::Unsupported { what: what.into() }));
        }
        let value = compact(&s.ssa, &hold.margin);
        let mut reads: Vec<Slot> =
            value.ssa.ins.iter().filter_map(|i| if let Ins::Leaf(k) = *i { jet(k).map(|_| k) } else { None }).collect();
        reads.sort_unstable();
        let mut d = value.ssa.clone();
        let grads: Vec<Vec<Id>> = reads.iter().map(|&j| grad(&mut d, &value.roots, j)).collect();
        let (mut roots, mut pairs) = ([value.roots.clone(), grads.concat()].concat(), vec![]);
        for (i, gi) in grads.iter().enumerate() {
            for (j, &rj) in reads.iter().enumerate().skip(i) {
                pairs.push((i, j));
                roots.extend(grad(&mut d, gi, rj));
            }
        }
        let (band, ins) = (
            match hold.kind {
                Kind::Equal { within } => Some(within / 2.0),
                Kind::Bound => None,
            },
            &s.ssa.ins,
        );
        let within = band.map_or(0.0, |b| 2.0 * b);
        // A component jet - c or c - jet states jet >= c or jet <= c (an equal: within the band, widened for rounding).
        let cert = hold.margin.iter().filter_map(|&m| match ins[m as usize] {
            Ins::Op(Atom::Sub, [p, q, _]) => match (ins[p as usize], ins[q as usize]) {
                (Ins::Leaf(j), Ins::Lit(c)) if jet(j).is_some() => Some((j, c as f64, false)),
                (Ins::Lit(c), Ins::Leaf(j)) if jet(j).is_some() => Some((j, c as f64, true)),
                _ => None,
            },
            Ins::Op(..) | Ins::Lit(_) | Ins::Leaf(_) => None,
        });
        let r = |c: f64| within + 4.0 * f64::EPSILON * (c.abs() + within);
        let cert = cert
            .map(|(j, c, upper)| match (band, upper) {
                (Some(_), _) => (j, c - r(c), c + r(c)),
                (None, false) => (j, c, f64::INFINITY),
                (None, true) => (j, f64::NEG_INFINITY, c),
            })
            .collect();
        let reads: Vec<(usize, usize)> =
            reads.iter().map(|&k| ((k - s.jets) as usize / 4, (k - s.jets) as usize % 4)).collect();
        let size = hold.margin.len();
        let (samples, dense) = (grid(a, b, h), grid(a, b, h / 64.0));
        let gp = compact(&d, &roots);
        lines.push(Line { hold, size, band, reads, pairs, value, grad: gp, samples, dense, cert });
    }

    // The preference as an exact quadratic 0.5 x'Qx - b'x by Gauss-Legendre over each knot gap, and the start: each
    // unknown's B-spline nearest its `from` by least squares. Both read their curves of time in f64.
    let (mut q, mut bv, mut x) = (vec![0.0; n * n], vec![0.0; n], vec![0.0; n]);
    let aux = compact(
        &s.ssa,
        &[s.vary.iter().map(|v| v.from).collect::<Vec<_>>(), p.near.iter().map(|z| z.1).collect()].concat(),
    );
    let mut regs = vec![];
    for (k, &(o, nk, hk)) in chan.iter().enumerate() {
        let (mut m, mut r) = (vec![0.0; nk * nk], vec![0.0; nk]);
        let near = p.near.iter().position(|z| z.0 == k).map(|i| s.vary.len() + i);
        for sp in 0..nk - 3 {
            for (xi, wq) in GL {
                let (tau, w) = (sp as f64 + (1.0 + xi) / 2.0, wq * hk / 2.0);
                let mut leaves: Vec<f64> = s.held.iter().map(|&v| v as f64).collect();
                leaves[s.time as usize] = t0 + tau * hk;
                eval(&aux.ssa, 1, leaves.len(), &leaves, &mut regs);
                let at = |i: usize| regs[aux.roots[i] as usize];
                if !at(k).is_finite() || near.is_some_and(|i| !at(i).is_finite()) {
                    return Err(refuse("a starting or near curve that is not finite over the span"));
                }
                let b0: [f64; 4] = std::array::from_fn(|i| kernel(tau - (sp + i) as f64 + 1.0, 0));
                for (wt, d) in [(p.jerk, 3), (p.acceleration, 2)].into_iter().filter(|z| z.0 > 0.0) {
                    let bd: [f64; 4] =
                        std::array::from_fn(|i| kernel(tau - (sp + i) as f64 + 1.0, d) / hk.powf(d as f64));
                    (0..16)
                        .for_each(|ij| q[(o + sp + ij / 4) * n + o + sp + ij % 4] += wt * w * bd[ij / 4] * bd[ij % 4]);
                }
                for i in 0..4 {
                    if let Some(z) = near {
                        bv[o + sp + i] += p.near_weight * w * at(z) * b0[i];
                        (0..4).for_each(|j| q[(o + sp + i) * n + o + sp + j] += p.near_weight * w * b0[i] * b0[j]);
                    }
                    r[sp + i] += w * at(k) * b0[i];
                    (0..4).for_each(|j| m[(sp + i) * nk + sp + j] += w * b0[i] * b0[j]);
                }
            }
        }
        cholesky(&m, nk, 1e-12, &mut r);
        x[o..o + nk].copy_from_slice(&r);
    }
    // Scaled to order 1, plus a whisper of distance from 0 so the least is unique where the preference is flat.
    let scale = (0..n).fold(0.0f64, |m, i| m.max(q[i * n + i]));
    q.iter_mut().chain(&mut bv).for_each(|v| *v /= scale);
    (0..n).for_each(|i| q[i * n + i] += 1e-12);

    let mut pr = Problem { s, chan, n, q, b: bv, mult: HashMap::new(), rho: 10.0, steps: 0, spent: 0, regs: vec![] };
    let mut rounds = 0;
    loop {
        rounds += 1;
        if pr.augmented(&mut x, &lines)? > MET {
            pr.diagnose(&x, &lines)?;
        }
        // The exchange: each interval hold's broken (or nearly touched) stretches join its samples at their worst time,
        // a broken one with its neighbourhood every h/16.
        let mut added = 0;
        for line in &mut lines {
            if line.dense.len() == 1 {
                continue;
            }
            let (band, delta) = (line.band, s.delta);
            let mut violation = |ts: &[f64]| -> Result<Vec<f64>, Error> {
                pr.run(&line.value, line.hold, ts, &x)?;
                Ok((0..ts.len())
                    .map(|l| {
                        let m = |c: usize| pr.regs[line.value.roots[c] as usize * ts.len() + l];
                        match band {
                            Some(b) => (0..line.size).fold(f64::NEG_INFINITY, |w, c| w.max(m(c).abs())) - b,
                            None => delta - m(0),
                        }
                    })
                    .collect())
            };
            let floor = if band.is_some() { MET } else { -NEAR };
            let bad: Vec<(f64, f64)> =
                peaks(&mut violation, &line.dense)?.into_iter().filter(|p| p.1 > floor).collect();
            let (lo, hi, old) = (line.dense[0], line.dense[line.dense.len() - 1], &line.samples);
            let apart = |t: f64| old.iter().all(|u| (u - t).abs() > h / 16.0);
            let mut fresh: Vec<f64> = vec![];
            for (t, v) in bad {
                let near =
                    [-4.0, -3.0, -2.0, -1.0, 1.0, 2.0, 3.0, 4.0].map(|k: f64| (t + k * h / 16.0).max(lo).min(hi));
                let all = if v > MET {
                    [&[t][..], &near.into_iter().filter(|&u| apart(u)).collect::<Vec<_>>()].concat()
                } else {
                    [t].into_iter().filter(|&u| apart(u)).collect()
                };
                all.into_iter().for_each(|u| {
                    if !fresh.contains(&u) && !old.contains(&u) {
                        fresh.push(u)
                    }
                });
            }
            added += fresh.len();
            line.samples.extend(fresh);
            line.samples.sort_by(f64::total_cmp);
        }
        if added == 0 || rounds >= 24 {
            break;
        }
    }

    let controls: Vec<Vec<f32>> =
        pr.chan.iter().map(|&(o, nk, _)| x[o..o + nk].iter().map(|&v| v as f32).collect()).collect();
    Ok(Solved { verdicts: verdicts(s, &controls)?, controls, rounds, steps: pr.steps })
}

/// Each hold proved over its window on the returned curves: the holds' program with every jet leaf replaced by the
/// curve's time derivative (`grad` of `curve`), as the solved scene's holds read it.
fn verdicts(s: &Solve, controls: &[Vec<f32>]) -> Result<Vec<Verdict>, Error> {
    let mut v = Ssa::default();
    let t = v.leaf(s.time);
    let mut jets = vec![];
    for c in controls {
        let mut r = curve(&mut v, t, s.over, c);
        for d in 0..4 {
            jets.push(r);
            if d < 3 {
                r = grad(&mut v, &[r], s.time)[0];
            }
        }
    }
    let mut map = vec![];
    for &ins in &s.ssa.ins {
        map.push(match ins {
            Ins::Leaf(k) if k >= s.jets && ((k - s.jets) as usize) < jets.len() => jets[(k - s.jets) as usize],
            Ins::Leaf(k) => v.leaf(k),
            Ins::Lit(c) => v.lit(c),
            Ins::Op(a, x) => v.op(a, x.map(|k| map.get(k as usize).copied().unwrap_or(0))),
        });
    }
    let ends: Vec<[Id; 2]> = s.holds.iter().map(|h| h.times.map(|e| v.lit(e as f32))).collect();
    let mut out = vec![];
    for (h, [lo, hi]) in s.holds.iter().zip(ends) {
        // Each proof reads only its hold's cone: `check` encloses its whole program at every box.
        let p = compact(&v, &[map[h.robust as usize], map[h.test as usize], lo, hi]);
        let body = Box::new(Formula::Atom { margin: p.roots[0], test: p.roots[1] });
        let (how, q) = (
            Strategy::Prove { budget: s.boxes, precision: s.precision },
            Quant { leaf: s.time, lo: p.roots[2], hi: p.roots[3], body },
        );
        let verdict = check(&p.ssa, &s.held, &Check { site: Some(h.site.clone()), body: Formula::Always(q), how })?;
        if verdict.truth == Truth::Fails {
            let at = verdict.witness.first().map_or(f32::NAN, |w| w.1);
            return Err(unsolved(
                h,
                format!(
                    "the returned f32 curve breaks this hold (margin {:.3e} at t = {at}): loosen it, raise delta, or give the spline more numbers",
                    verdict.margin
                ),
            ));
        }
        out.push(verdict);
    }
    Ok(out)
}
