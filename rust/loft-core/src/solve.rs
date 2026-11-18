//! `solve`'s numeric kernel (src/geom/solve.ts): the augmented Lagrangian with Newton/Levenberg-Marquardt inner steps,
//! the conflict diagnosis and the exchange rounds, over holds compiled to tapes. A line-for-line twin of `Problem`,
//! `diagnose`, `peaks` and the rounds loop of `solve`: the same operations in the same order on f64, so the same
//! numbers come out (jsmath for JS's Math where Rust's differs). What stays in TypeScript: classifying the holds,
//! their symbolic derivatives, the preference's quadratic, the start, the cache, and building and verifying the
//! returned curves.

use crate::jsmath as m;
use crate::tape::Tape;
use crate::wire::{Reader, WireError, Writer};
use crate::fx::FxMap as HashMap;
use std::time::{Duration, Instant};

const AIM: f64 = 1e-8;
const TOLERANCE: f64 = 1e-5;
const MET: f64 = TOLERANCE / 10.0;
const NEAR: f64 = 1e-3;
fn inner_band(within: f64) -> f64 { within / 2.0 }

/// The cubic B-spline kernel's d-th derivative (solve.ts `kernel`).
fn kernel(x: f64, d: usize) -> f64 {
    let s = if x < 0.0 { -1.0 } else { 1.0 };
    let a = x.abs();
    if a >= 2.0 { return 0.0; }
    let odd = if d % 2 == 1 { s } else { 1.0 };
    if a < 1.0 {
        let v = match d {
            0 => (4.0 - 6.0 * a * a + 3.0 * a * a * a) / 6.0,
            1 => (-12.0 * a + 9.0 * a * a) / 6.0,
            2 => (-12.0 + 18.0 * a) / 6.0,
            _ => 3.0,
        };
        return odd * v;
    }
    let o = 2.0 - a;
    let v = match d {
        0 => o * o * o / 6.0,
        1 => -o * o / 2.0,
        2 => o,
        _ => -1.0,
    };
    odd * v
}

/// Why the kernel stopped without an answer (solve.ts `SolveFailure`, by line index).
pub enum Failure {
    Budget,
    /// `detail`: 0 non-finite margin, 1 non-finite derivative, 2 non-finite spline derivative.
    Unreadable { line: usize, at: f64, detail: u32 },
    Infeasible { lines: Vec<usize>, worst: f64 },
    Search { worst: f64, line: usize },
}

type R<T> = Result<T, Failure>;

struct Channel {
    offset: usize,
    n: usize,
    h: f64,
    /// `h ** d`, d = 0..3, as JS computes it.
    hp: [f64; 4],
    /// The first channel with the same n and h: channels of one shape share their weights at a time.
    shape: usize,
}

struct Line {
    equal: bool,
    within: f64,
    size: usize,
    reads: Vec<(usize, usize)>,
    pairs: Vec<(usize, usize)>,
    samples: Vec<f64>,
    dense: Vec<f64>,
    grad: Tape,
    value: Tape,
    /// The certificate bounds `diagnose` derives from the hold's shape, in its order: (jet node id, low, high).
    cert: Vec<(u32, f64, f64)>,
}

/// A row's gradient and Hessian on the numbers it touches: `idx` (in `Rows::idx` from `at`, `len` long), the gradient
/// at those (`Rows::vals` from `g`) and the dense len x len block (`Rows::vals` from `m`).
#[derive(Clone, Copy)]
struct Part { at: usize, len: usize, g: usize, m: usize }

/// Every row at x (Problem.rows), the derivatives packed into two pools instead of a vector per row.
#[derive(Default)]
struct Rows {
    eq: Vec<f64>, eq_part: Vec<Part>,
    ge: Vec<f64>, ge_part: Vec<Part>,
    idx: Vec<usize>,
    vals: Vec<f64>,
}

impl Rows {
    fn idx(&self, p: Part) -> &[usize] { &self.idx[p.at..p.at + p.len] }
    fn g(&self, p: Part) -> &[f64] { &self.vals[p.g..p.g + p.len] }
    fn m(&self, p: Part) -> &[f64] { &self.vals[p.m..p.m + p.len * p.len] }
}

struct Problem {
    n: usize,
    steps: u32,
    mult: HashMap<(usize, u64, usize), f64>,
    rho: f64,
    channels: Vec<Channel>,
    t0: f64,
    q: Vec<f64>,
    b: Vec<f64>,
    delta: f64,
    deadline: Option<Instant>,
    regs: Vec<f64>,
    /// cholesky_solve's L, kept between calls.
    chol: std::cell::RefCell<Vec<f64>>,
}

/// `${t}` as a Map key: the same for -0 and 0, as String(t) is.
fn tkey(t: f64) -> u64 { if t == 0.0 { 0 } else { t.to_bits() } }

impl Problem {
    fn check(&self) -> R<()> {
        match self.deadline { Some(d) if Instant::now() >= d => Err(Failure::Budget), _ => Ok(()) }
    }

    /// The four controls and their weights at t (Problem.place).
    fn place(&self, c: &Channel, t: f64) -> (usize, f64, f64) {
        let raw = (m::fround(t) - self.t0) / c.h;
        let tau = m::min(m::max(raw, 0.0), (c.n - 3) as f64);
        let first = m::max(0.0, m::min((c.n - 4) as f64, tau.floor()));
        (first as usize, raw, tau)
    }
    fn weight(first: usize, raw: f64, tau: f64, q: usize, d: usize) -> f64 {
        if d > 0 && raw != tau { return 0.0; }
        if d == 3 { return [-1.0, 3.0, -3.0, 1.0][q]; }
        kernel(tau - (first as f64 + q as f64 - 1.0), d)
    }

    /// The jets of every unknown at t, by slot 4k + d (Problem.env).
    /// The B-spline weights at t for channel k's shape: (first, w[d][q]) (Problem.place and its `weight`).
    fn weights(&self, k: usize, t: f64) -> (usize, [[f64; 4]; 4]) {
        let (first, raw, tau) = self.place(&self.channels[k], t);
        let mut w = [[0.0; 4]; 4];
        for (d, wd) in w.iter_mut().enumerate() { for (q, wq) in wd.iter_mut().enumerate() { *wq = Self::weight(first, raw, tau, q, d); } }
        (first, w)
    }

    /// The jets the tape reads at t (Problem.env, which computes all of them: the others are never read), into
    /// `vars` at slot 4k + d. Weights are computed once per channel shape.
    fn env(&self, x: &[f64], t: f64, slots: &[u32], vars: &mut [f64]) {
        let mut shapes: [(usize, usize, [[f64; 4]; 4]); 4] = [(usize::MAX, 0, [[0.0; 4]; 4]); 4];
        let mut known = 0;
        for &slot in slots {
            let (k, d) = (slot as usize / 4, slot as usize % 4);
            let c = &self.channels[k];
            let found = shapes[..known].iter().position(|s| s.0 == c.shape);
            let (first, w) = match found {
                Some(i) => (shapes[i].1, shapes[i].2),
                None => {
                    let (first, w) = self.weights(c.shape, t);
                    if known < shapes.len() { shapes[known] = (c.shape, first, w); known += 1; }
                    (first, w)
                }
            };
            let mut v = 0.0;
            for q in 0..4 { v += x[c.offset + first + q] * w[d][q]; }
            vars[slot as usize] = v / c.hp[d];
        }
    }

    /// One line's tape at its times (one lane each): fills `self.regs`. `vars` is scratch.
    fn eval(&mut self, line: &Line, grad: bool, x: &[f64], ts: &[f64], vars: &mut Vec<f64>, times: &mut Vec<f64>) -> R<()> {
        self.check()?;
        let width = 4 * self.channels.len();
        vars.clear();
        vars.resize(width * ts.len(), 0.0);
        times.clear();
        let tape = if grad { &line.grad } else { &line.value };
        for (l, &t) in ts.iter().enumerate() {
            self.env(x, t, &tape.var_slots, &mut vars[l * width..(l + 1) * width]);
            times.push(m::fround(t));
        }
        tape.run(ts.len(), width, vars, times, &mut self.regs);
        Ok(())
    }

    /// `checkedEvaluator`'s check of lane l: every root finite (a boolean root never is).
    fn checked(&self, li: usize, tape: &Tape, lanes: usize, l: usize, t: f64) -> R<()> {
        for i in 0..tape.root_count() {
            let fine = !tape.root_bool[i] && tape.root(i).iter().all(|&r| self.regs[r as usize * lanes + l].is_finite());
            if !fine { return Err(Failure::Unreadable { line: li, at: m::fround(t), detail: if i == 0 { 0 } else { 1 } }); }
        }
        Ok(())
    }

    fn rows(&mut self, x: &[f64], lines: &[Line], only: Option<usize>, grad: bool) -> R<Rows> {
        let mut r = Rows::default();
        let (mut vars, mut times) = (Vec::new(), Vec::new());
        let range: Vec<usize> = match only { Some(i) => vec![i], None => (0..lines.len()).collect() };
        for li in range {
            let line = &lines[li];
            let lanes = line.samples.len();
            if lanes == 0 { continue; }
            self.eval(line, grad, x, &line.samples, &mut vars, &mut times)?;
            let tape = if grad { &line.grad } else { &line.value };
            for (l, &t) in line.samples.iter().enumerate() {
                self.checked(li, tape, lanes, l, t)?;
                let regs = &self.regs;
                let margin = tape.root(0);
                let mlen = margin.len();
                let mut chans: Vec<usize> = vec![];
                let mut firsts: Vec<usize> = vec![];
                let mut slot: Vec<usize> = vec![];
                let mut w: Vec<[f64; 4]> = vec![];
                let mut idx: Vec<usize> = vec![];
                if grad {
                    for &(k, _) in &line.reads {
                        let p = match chans.iter().position(|&c| c == k) {
                            Some(p) => p,
                            None => { chans.push(k); firsts.push(self.place(&self.channels[k], t).0); chans.len() - 1 }
                        };
                        slot.push(p);
                    }
                    let mut by_shape: Vec<(usize, [[f64; 4]; 4])> = vec![];
                    for &(k, d) in &line.reads {
                        let ch = &self.channels[k];
                        let wd = match by_shape.iter().find(|s| s.0 == ch.shape) {
                            Some(s) => s.1,
                            None => { let w = self.weights(ch.shape, t).1; by_shape.push((ch.shape, w)); w }
                        };
                        let scale = ch.hp[d];
                        w.push([0, 1, 2, 3].map(|q| wd[d][q] / scale));
                    }
                    for (p, &k) in chans.iter().enumerate() {
                        for q in 0..4 { idx.push(self.channels[k].offset + firsts[p] + q); }
                    }
                }
                let ll = idx.len();
                let at = r.idx.len();
                if grad { r.idx.extend_from_slice(&idx); }
                for c in 0..line.size {
                    let v = regs[margin[if c < mlen { c } else { 0 }] as usize * lanes + l];
                    if line.equal { r.eq.push(v); } else { r.ge.push(v); }
                    if !grad { continue; }
                    let pick = |ri: usize| { let d = tape.root(ri); regs[d[if d.len() == 1 { 0 } else { c }] as usize * lanes + l] };
                    // Only the entries at idx: addRow reads no other (and the rest are 0, so finite).
                    let g0 = r.vals.len();
                    r.vals.resize(g0 + ll + ll * ll, 0.0);
                    let (g, mm) = r.vals[g0..].split_at_mut(ll);
                    for ri in 0..line.reads.len() {
                        let dv = pick(1 + ri);
                        if dv != 0.0 { for q in 0..4 { g[slot[ri] * 4 + q] += dv * w[ri][q]; } }
                    }
                    for (ri, &(p, q)) in line.pairs.iter().enumerate() {
                        if (ri & 255) == 0 { self.check()?; }
                        let dv = pick(1 + line.reads.len() + ri);
                        if dv == 0.0 { continue; }
                        let (wp, wq, pp, qs) = (w[p], w[q], slot[p] * 4, slot[q] * 4);
                        for i in 0..4 {
                            for j in 0..4 {
                                let v2 = dv * wp[i] * wq[j];
                                mm[(pp + i) * ll + qs + j] += v2;
                                if p != q { mm[(qs + j) * ll + pp + i] += v2; }
                            }
                        }
                    }
                    if !g.iter().all(|v| v.is_finite()) || !mm.iter().all(|v| v.is_finite()) {
                        return Err(Failure::Unreadable { line: li, at: t, detail: 2 });
                    }
                    let part = Part { at, len: ll, g: g0, m: g0 + ll };
                    if line.equal { r.eq_part.push(part); } else { r.ge_part.push(part); }
                }
            }
        }
        Ok(r)
    }

    #[inline(never)]
    fn objective(&self, x: &[f64]) -> R<f64> {
        let n = self.n;
        let mut f = 0.0;
        for i in 0..n {
            self.check()?;
            let mut s = 0.0;
            for j in 0..n { s += self.q[i * n + j] * x[j]; }
            f += 0.5 * x[i] * s - self.b[i] * x[i];
        }
        Ok(f)
    }

    fn worst(&mut self, x: &[f64], lines: &[Line], li: usize) -> R<f64> {
        let r = self.rows(x, lines, Some(li), false)?;
        let line = &lines[li];
        let mut w = 0.0;
        if line.equal { for v in &r.eq { w = m::max(w, v.abs() - line.within); } }
        for v in &r.ge { w = m::max(w, -v); }
        Ok(w)
    }

    fn cholesky_solve(&self, a: &[f64], n: usize, damp: f64, r: &mut [f64]) -> R<bool> {
        let mut l = std::mem::take(&mut *self.chol.borrow_mut());
        l.clear();
        l.resize(n * n, 0.0);
        let ok = self.cholesky_in(a, n, damp, r, &mut l);
        *self.chol.borrow_mut() = l;
        ok
    }

    #[inline(never)]
    fn cholesky_in(&self, a: &[f64], n: usize, damp: f64, r: &mut [f64], l: &mut [f64]) -> R<bool> {
        // Each L[i][j] is computed exactly as the JS twin computes it (the same subtractions in the same order); rows
        // are taken four at a time so their columns left of the block run as four independent chains. Pivots are
        // still met in row order.
        const B: usize = 4;
        let mut i0 = 0;
        while i0 < n {
            let rows = B.min(n - i0);
            self.check()?;
            for j in 0..i0 {
                let lj = &l[j * n..j * n + j];
                let mut s = [0.0f64; B];
                for (b, sb) in s.iter_mut().enumerate().take(rows) {
                    let i = i0 + b;
                    *sb = a[i * n + j] + 0.0;
                }
                for k in 0..j {
                    let x = lj[k];
                    for (b, sb) in s.iter_mut().enumerate().take(rows) { *sb -= l[(i0 + b) * n + k] * x; }
                }
                let d = l[j * n + j];
                for (b, sb) in s.iter().enumerate().take(rows) { l[(i0 + b) * n + j] = sb / d; }
            }
            for b in 0..rows {
                let i = i0 + b;
                for j in i0..=i {
                    let mut s = a[i * n + j] + if i == j { damp } else { 0.0 };
                    for k in 0..j { s -= l[i * n + k] * l[j * n + k]; }
                    if i == j {
                        if !(s > 0.0) { return Ok(false); }
                        l[i * n + i] = s.sqrt();
                    } else {
                        l[i * n + j] = s / l[j * n + j];
                    }
                }
            }
            i0 += rows;
        }
        for i in 0..n {
            self.check()?;
            let mut s = r[i];
            for k in 0..i { s -= l[i * n + k] * r[k]; }
            r[i] = s / l[i * n + i];
        }
        for i in (0..n).rev() {
            self.check()?;
            let mut s = r[i];
            for k in i + 1..n { s -= l[k * n + i] * r[k]; }
            r[i] = s / l[i * n + i];
        }
        Ok(true)
    }

    /// The augmented Lagrangian's terms at y: f, and with grad g and H (the closure `augmented` hands `descend`).
    #[allow(clippy::too_many_arguments)]
    #[inline(never)]
    fn terms(&mut self, y: &[f64], grad: bool, lines: &[Line], slack: &[f64], lam: &[f64], mu: &[f64], rr: f64) -> R<(f64, Vec<f64>, Vec<f64>)> {
        let n = self.n;
        let r = self.rows(y, lines, None, grad)?;
        let mut f = self.objective(y)?;
        let mut g = if grad { vec![0.0; n] } else { vec![] };
        let mut h = if grad { self.q.clone() } else { vec![] };
        if grad {
            for i in 0..n {
                self.check()?;
                let mut s = -self.b[i];
                for j in 0..n { s += self.q[i * n + j] * y[j]; }
                g[i] = s;
            }
        }
        for (k, &e) in r.eq.iter().enumerate() {
            let residual = e - slack[k];
            let z = lam[k] + rr * residual;
            f += lam[k] * residual + 0.5 * rr * residual * residual;
            if grad {
                let p = r.eq_part[k];
                self.add_row(&mut g, &mut h, r.g(p), r.idx(p), z, rr)?;
                self.add_curvature(&mut h, r.idx(p), r.m(p), z)?;
            }
        }
        for (k, &margin) in r.ge.iter().enumerate() {
            let c = margin - self.delta;
            let s = m::max(0.0, mu[k] - rr * c);
            f += (s * s - mu[k] * mu[k]) / (2.0 * rr);
            if grad && s > 0.0 {
                let p = r.ge_part[k];
                self.add_row(&mut g, &mut h, r.g(p), r.idx(p), -s, rr)?;
                self.add_curvature(&mut h, r.idx(p), r.m(p), -s)?;
            }
        }
        Ok((f, g, h))
    }

    #[allow(clippy::too_many_arguments)]
    /// addRow with the row's gradient given at `idx` only (`grad[a]` is entry `idx[a]`).
    fn add_row(&self, g: &mut [f64], h: &mut [f64], grad: &[f64], idx: &[usize], c: f64, w: f64) -> R<()> {
        let n = self.n;
        for (a, &i) in idx.iter().enumerate() {
            self.check()?;
            let gi = grad[a];
            if gi == 0.0 { continue; }
            g[i] += c * gi;
            for (b, &j) in idx.iter().enumerate() { h[i * n + j] += w * gi * grad[b]; }
        }
        Ok(())
    }

    fn add_curvature(&self, h: &mut [f64], idx: &[usize], km: &[f64], c: f64) -> R<()> {
        let n = self.n;
        let l = idx.len();
        for a in 0..l {
            self.check()?;
            for b in 0..l { h[idx[a] * n + idx[b]] += c * km[a * l + b]; }
        }
        Ok(())
    }

    /// Problem.descend with the augmented Lagrangian's terms.
    #[allow(clippy::too_many_arguments)]
    #[inline(never)]
    fn descend(&mut self, x: &mut [f64], lines: &[Line], slack: &[f64], lam: &[f64], mu: &[f64], rr: f64, budget: usize, tol: f64) -> R<()> {
        let n = self.n;
        let mut damp = 1e-9;
        for _ in 0..budget {
            let (f, g, h) = self.terms(x, true, lines, slack, lam, mu, rr)?;
            let mut moved = false;
            for _ in 0..40 {
                let mut d: Vec<f64> = g.iter().map(|v| -v).collect();
                if self.cholesky_solve(&h, n, damp, &mut d)? {
                    let (mut step, mut size) = (0.0f64, 0.0f64);
                    for i in 0..n { step = m::max(step, d[i].abs()); size = m::max(size, x[i].abs()); }
                    if step <= tol * (1.0 + size) { return Ok(()); }
                    let y: Vec<f64> = x.iter().enumerate().map(|(i, v)| v + d[i]).collect();
                    self.steps += 1;
                    let fy = self.terms(&y, false, lines, slack, lam, mu, rr)?.0;
                    if fy <= f + 1e-13 * f.abs() {
                        x.copy_from_slice(&y);
                        damp = m::max(damp / 8.0, 1e-12);
                        moved = true;
                        break;
                    }
                }
                damp = m::max(damp * 8.0, 1e-9);
                if damp > 1e14 { break; }
            }
            if !moved { return Ok(()); }
        }
        Ok(())
    }

    fn keys(lines: &[Line]) -> (Vec<(usize, u64, usize)>, Vec<(usize, u64, usize)>) {
        let (mut eq, mut ge) = (vec![], vec![]);
        for (i, line) in lines.iter().enumerate() {
            for &t in &line.samples {
                for c in 0..line.size { if line.equal { eq.push((i, tkey(t), c)) } else { ge.push((i, tkey(t), c)) } }
            }
        }
        (eq, ge)
    }

    /// Problem.augmented.
    #[inline(never)]
    fn augmented(&mut self, x: &mut [f64], lines: &[Line]) -> R<f64> {
        let (keq, kge) = Self::keys(lines);
        let mut bands = vec![];
        for line in lines {
            if line.equal { for _ in 0..line.size * line.samples.len() { bands.push(inner_band(line.within)); } }
        }
        let mut lam: Vec<f64> = keq.iter().map(|k| self.mult.get(k).copied().unwrap_or(0.0)).collect();
        let mut mu: Vec<f64> = kge.iter().map(|k| self.mult.get(k).copied().unwrap_or(0.0)).collect();
        let mut last = f64::INFINITY;
        let mut v = f64::INFINITY;
        for _outer in 0..60 {
            let rr = self.rho;
            let before = x.to_vec();
            let r0 = self.rows(x, lines, None, false)?;
            let slack: Vec<f64> = r0.eq.iter().enumerate().map(|(k, &e)| m::max(-bands[k], m::min(bands[k], e + lam[k] / rr))).collect();
            self.descend(x, lines, &slack, &lam, &mu, rr, 100, 1e-9)?;
            let r = self.rows(x, lines, None, false)?;
            v = 0.0;
            for (k, &e) in r.eq.iter().enumerate() {
                v = m::max(v, e.abs() - bands[k]);
                let u = lam[k] + rr * e;
                lam[k] = u - m::min(m::max(u, -rr * bands[k]), rr * bands[k]);
            }
            for (k, &margin) in r.ge.iter().enumerate() {
                let c = margin - self.delta;
                v = m::max(v, -c);
                mu[k] = m::max(0.0, mu[k] - rr * c);
            }
            let (mut moved, mut size) = (0.0f64, 0.0f64);
            for i in 0..self.n { moved = m::max(moved, (x[i] - before[i]).abs()); size = m::max(size, x[i].abs()); }
            if v <= AIM && moved <= 1e-6 * (1.0 + size) { break; }
            if v > 0.25 * last { self.rho = m::min(self.rho * 10.0, 1e12); }
            last = v;
        }
        for (i, k) in keq.iter().enumerate() { self.mult.insert(*k, lam[i]); }
        for (i, k) in kge.iter().enumerate() { self.mult.insert(*k, mu[i]); }
        Ok(v)
    }

    /// The first value of a line's margin at each time (`line.value(problem.env(x, t))[0]`), as components. All are
    /// computed at once; the first time that does not evaluate is the error, as in JS evaluating them in order.
    fn margins(&mut self, li: usize, line: &Line, x: &[f64], ts: &[f64]) -> R<Vec<Vec<f64>>> {
        let (mut vars, mut times) = (Vec::new(), Vec::new());
        self.eval(line, false, x, ts, &mut vars, &mut times)?;
        let lanes = ts.len();
        let mut out = Vec::with_capacity(lanes);
        for (l, &t) in ts.iter().enumerate() {
            self.checked(li, &line.value, lanes, l, t)?;
            out.push(line.value.root(0).iter().map(|&r| self.regs[r as usize * lanes + l]).collect());
        }
        Ok(out)
    }
}

/// solve.ts `diagnose`.
#[inline(never)]
fn diagnose(p: &mut Problem, lines: &[Line], x: &[f64]) -> R<()> {
    let mut bounds: HashMap<u32, Vec<(usize, f64, f64)>> = HashMap::default();
    let mut worst = f64::NEG_INFINITY;
    let mut which = 0usize;
    for (li, line) in lines.iter().enumerate() {
        p.check()?;
        let miss = p.worst(x, lines, li)?;
        p.check()?;
        if miss > worst { worst = miss; which = li; }
        if line.reads.is_empty() && miss > 0.0 { return Err(Failure::Infeasible { lines: vec![li], worst: miss }); }
        for &(jet, low, high) in &line.cert {
            let prior = bounds.entry(jet).or_default();
            for &(other, olow, ohigh) in prior.iter() {
                p.check()?;
                let o = &lines[other];
                if o.dense[0] > *line.dense.last().unwrap() || line.dense[0] > *o.dense.last().unwrap() { continue; }
                if low > ohigh || high < olow {
                    let ls = if other == li { vec![li] } else { vec![other, li] };
                    return Err(Failure::Infeasible { lines: ls, worst });
                }
            }
            prior.push((li, low, high));
        }
    }
    p.check()?;
    if worst > 0.0 { return Err(Failure::Search { worst, line: which }); }
    Ok(())
}

/// solve.ts `peaks`.
#[inline(never)]
fn peaks(f: &mut dyn FnMut(&[f64]) -> R<Vec<f64>>, grid: &[f64]) -> R<Vec<(f64, f64)>> {
    // grid.map(f) is pure: evaluated as one batch.
    let v = f(grid)?;
    let mut f = |t: f64| -> R<f64> { Ok(f(&[t])?[0]) };
    let g = (5f64.sqrt() - 1.0) / 2.0;
    let mut out = vec![];
    for i in 0..grid.len() {
        let prev = if i == 0 { f64::NEG_INFINITY } else { v[i - 1] };
        let next = if i + 1 < grid.len() { v[i + 1] } else { f64::NEG_INFINITY };
        if !(v[i] >= prev && v[i] >= next) { continue; }
        let mut a = grid[i.saturating_sub(1)];
        let mut b = grid[(i + 1).min(grid.len() - 1)];
        let mut best = (grid[i], v[i]);
        let mut c = b - g * (b - a);
        let mut d = a + g * (b - a);
        let mut fc = f(c)?;
        let mut fd = f(d)?;
        let mut k = 0;
        while k < 30 && b - a > 1e-12 {
            if fc >= fd { b = d; d = c; fd = fc; c = b - g * (b - a); fc = f(c)?; } else { a = c; c = d; fc = fd; d = a + g * (b - a); fd = f(d)?; }
            k += 1;
        }
        for pt in [(c, fc), (d, fd)] { if pt.1 > best.1 { best = pt; } }
        out.push(best);
    }
    Ok(out)
}

/// Math.max(...xs) over a non-empty list.
fn max_of(xs: impl Iterator<Item = f64>) -> f64 { xs.fold(f64::NEG_INFINITY, m::max) }

/// The rounds of `solve`: augmented Lagrangian, diagnosis, exchange. Returns rounds; x and samples are updated.
fn rounds(p: &mut Problem, lines: &mut [Line], x: &mut [f64], h: f64) -> R<u32> {
    let mut rounds = 0;
    loop {
        rounds += 1;
        let v = p.augmented(x, lines)?;
        if v > MET { diagnose(p, lines, x)?; }
        let mut added = 0;
        for li in 0..lines.len() {
            if lines[li].dense.len() == 1 { continue; }
            let (equal, within, delta) = (lines[li].equal, lines[li].within, p.delta);
            let bad: Vec<(f64, f64)> = {
                let line = &lines[li];
                let mut violation = |ts: &[f64]| -> R<Vec<f64>> {
                    Ok(p.margins(li, line, x, ts)?.into_iter().map(|mv| if equal { max_of(mv.iter().map(|v| v.abs())) - inner_band(within) } else { delta - mv[0] }).collect())
                };
                let floor = if equal { MET } else { -NEAR };
                peaks(&mut violation, &line.dense)?.into_iter().filter(|q| q.1 > floor).collect()
            };
            let line = &mut lines[li];
            let (lo, hi) = (line.dense[0], *line.dense.last().unwrap());
            let samples = &line.samples;
            let apart = |t: f64| samples.iter().all(|u| (u - t).abs() > h / 16.0);
            // [...new Set(flatMap)]: first occurrence wins (SameValueZero), then those not already sampled.
            let mut fresh: Vec<f64> = vec![];
            let push = |t: f64, fresh: &mut Vec<f64>| { if !fresh.iter().any(|&u| u == t || (u.is_nan() && t.is_nan())) { fresh.push(t); } };
            for &(t, v) in &bad {
                if v > MET {
                    p.check()?;
                    push(t, &mut fresh);
                    for q in [-4.0, -3.0, -2.0, -1.0, 1.0, 2.0, 3.0, 4.0] {
                        let u = m::min(m::max(t + (q * h) / 16.0, lo), hi);
                        p.check()?;
                        if apart(u) { push(u, &mut fresh); }
                    }
                } else {
                    p.check()?;
                    if apart(t) { push(t, &mut fresh); }
                }
            }
            fresh.retain(|t| !samples.contains(t));
            let n = fresh.len();
            line.samples.extend(fresh);
            // Array sort with (p, q) => p - q: stable, numeric.
            line.samples.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
            added += n;
        }
        if added == 0 { break; }
        if rounds >= 24 { break; }
    }
    Ok(rounds)
}

pub fn run(r: &mut Reader, w: &mut Writer) -> Result<bool, WireError> {
    let budget = r.f64()?;
    let deadline = if budget < 0.0 { None } else { Some(Instant::now() + Duration::from_secs_f64(budget / 1000.0)) };
    let t0 = r.f64()?;
    let delta = r.f64()?;
    let h = r.f64()?;
    let nch = r.usize()?;
    let mut channels = vec![];
    for k in 0..nch {
        let offset = r.usize()?;
        let n = r.usize()?;
        let h = r.f64()?;
        let shape = channels.iter().position(|c: &Channel| c.n == n && c.h.to_bits() == h.to_bits()).unwrap_or(k);
        channels.push(Channel { offset, n, h, hp: [0, 1, 2, 3].map(|d| m::pow(h, d as f64)), shape });
    }
    let q = r.f64s()?;
    let b = r.f64s()?;
    let mut x = r.f64s()?;
    let nlines = r.usize()?;
    let mut lines = vec![];
    for _ in 0..nlines {
        let equal = r.u32()? == 0;
        let within = r.f64()?;
        let size = r.usize()?;
        let nreads = r.usize()?;
        let mut reads = vec![];
        for _ in 0..nreads { let k = r.usize()?; let d = r.usize()?; reads.push((k, d)); }
        let npairs = r.usize()?;
        let mut pairs = vec![];
        for _ in 0..npairs { let a = r.usize()?; let b = r.usize()?; pairs.push((a, b)); }
        let samples = r.f64s()?;
        let dense = r.f64s()?;
        let grad = Tape::read(r).map_err(|e| WireError(e.0))?;
        let value = Tape::read(r).map_err(|e| WireError(e.0))?;
        let ncert = r.usize()?;
        let mut cert = vec![];
        for _ in 0..ncert { let jet = r.u32()?; let lo = r.f64()?; let hi = r.f64()?; cert.push((jet, lo, hi)); }
        lines.push(Line { equal, within, size, reads, pairs, samples, dense, grad, value, cert });
    }
    r.done()?;
    let n = x.len();
    let mut p = Problem { n, steps: 0, mult: HashMap::default(), rho: 10.0, channels, t0, q, b, delta, deadline, regs: vec![], chol: Default::default() };
    match rounds(&mut p, &mut lines, &mut x, h) {
        Ok(k) => {
            w.f64s(&x);
            w.u32(p.steps);
            w.u32(k);
            for l in &lines { w.f64s(&l.samples); }
            Ok(true)
        }
        Err(f) => {
            match f {
                Failure::Budget => w.u32(0),
                Failure::Unreadable { line, at, detail } => { w.u32(1); w.u32(line as u32); w.f64(at); w.u32(detail); }
                Failure::Infeasible { lines, worst } => { w.u32(2); w.u32s(&lines.iter().map(|&l| l as u32).collect::<Vec<_>>()); w.f64(worst); }
                Failure::Search { worst, line } => { w.u32(3); w.f64(worst); w.u32(line as u32); }
            }
            w.u32(p.steps);
            Ok(false)
        }
    }
}

/// The verification of the returned curves (the `reports` of solve.ts): each hold's margin, now a function of time
/// alone, at the f32 time of every point of its fine grid, refined by `peaks`; the worst (t, v) per hold, or where it
/// could not be evaluated. Every hold is measured; solve.ts reads them in order, as it measured them.
pub fn verify(r: &mut Reader, w: &mut Writer) -> Result<bool, WireError> {
    let budget = r.f64()?;
    let deadline = if budget < 0.0 { None } else { Some(Instant::now() + Duration::from_secs_f64(budget / 1000.0)) };
    let n = r.usize()?;
    let mut holds = Vec::with_capacity(n);
    for _ in 0..n {
        let equal = r.u32()? == 0;
        let dense = r.f64s()?;
        let tape = Tape::read(r).map_err(|e| WireError(e.0))?;
        holds.push((equal, dense, tape));
    }
    r.done()?;
    let mut regs: Vec<f64> = vec![];
    let check = || match deadline { Some(d) if Instant::now() >= d => Err(Failure::Budget), _ => Ok(()) };
    for (equal, dense, tape) in &holds {
        // A non-finite margin at time `at`, or the violation at each time (frame(Math.fround(t))).
        let mut f = |ts: &[f64]| -> R<Vec<f64>> {
            check()?;
            let times: Vec<f64> = ts.iter().map(|&t| m::fround(t)).collect();
            tape.run(ts.len(), 0, &[], &times, &mut regs);
            let lanes = ts.len();
            let mut out = Vec::with_capacity(lanes);
            for l in 0..lanes {
                let fine = (0..tape.root_count()).all(|i| !tape.root_bool[i] && tape.root(i).iter().all(|&q| regs[q as usize * lanes + l].is_finite()));
                if !fine { return Err(Failure::Unreadable { line: 0, at: times[l], detail: 0 }); }
                let mv = tape.root(0).iter().map(|&q| regs[q as usize * lanes + l]);
                out.push(if *equal { max_of(mv.map(f64::abs)) } else { -tape_first(tape, &regs, lanes, l) });
            }
            Ok(out)
        };
        match peaks(&mut f, dense) {
            Ok(ps) => {
                let mut best = (dense[0], f64::NEG_INFINITY);
                for p in ps { if p.1 > best.1 { best = p; } }
                w.u32(0); w.f64(best.0); w.f64(best.1);
            }
            Err(Failure::Unreadable { at, .. }) => { w.u32(1); w.f64(at); }
            Err(Failure::Budget) => { w.b.truncate(1); w.u32(0); return Ok(false); }
            Err(_) => unreachable!("verification fails only by budget or an unreadable margin"),
        }
    }
    Ok(true)
}

fn tape_first(tape: &Tape, regs: &[f64], lanes: usize, l: usize) -> f64 { regs[tape.root(0)[0] as usize * lanes + l] }
