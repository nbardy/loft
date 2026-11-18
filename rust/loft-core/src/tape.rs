//! Expressions on the CPU: the Rust twin of `compileJs` (src/expr.ts), giving the same f64 bits.
//!
//! The packed program is the nodes reachable from the roots, dependencies first (src/core/tape_pack.ts). Compiling
//! does what compileJs does while it writes JavaScript: every vector becomes one value per component; a component is
//! either known now (a constant, or an operation of known operands, computed now by the same function) or a register
//! filled at run time; and the exact identities x*0, x*1, x+0, x-0, x/1 are applied to known operands. A known number
//! that becomes an operand of a run-time operation is first written as compileJs writes a literal, so -0 becomes 0
//! (`jsLit` prints String(-0), "0"). Run time is one linear pass over the instructions.

use crate::jsmath as m;
use crate::wire::{Reader, WireError};

pub const CURVE_N: usize = 64;

/// The node kinds of the packed program (src/core/tape_pack.ts `OP`).
const OP_CONST: u32 = 0;
const OP_VAR: u32 = 1;
const OP_LEAF: u32 = 2;
const OP_TIME: u32 = 3;
const OP_ROW: u32 = 4;
const OP_CALL: u32 = 5;
const OP_VEC: u32 = 6;
const OP_GET: u32 = 7;

/// The functions of a call, by number (src/core/tape_pack.ts `FN`, the order of expr.ts `Fn`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u32)]
pub enum Func {
    Add, Sub, Mul, Div, Neg, Min, Max, Pow, Mod, Atan2, Step,
    Abs, Sign, Floor, Fract, Sqrt, Exp, Log, Sin, Cos, Tan, Saturate, Normalize,
    Mix, Clamp, Smoothstep, Length, Distance, Dot, Cross,
    Lt, Gt, Le, Ge, Eq, Ne, And, Or, Not, Select,
    Ceil, Trunc, RoundEven, Rem, InverseSqrt, Exp2, Log2, Asin, Acos, Atan, Sinh, Cosh, Tanh,
}
const FNS: [Func; 53] = {
    use Func::*;
    [Add, Sub, Mul, Div, Neg, Min, Max, Pow, Mod, Atan2, Step,
     Abs, Sign, Floor, Fract, Sqrt, Exp, Log, Sin, Cos, Tan, Saturate, Normalize,
     Mix, Clamp, Smoothstep, Length, Distance, Dot, Cross,
     Lt, Gt, Le, Ge, Eq, Ne, And, Or, Not, Select,
     Ceil, Trunc, RoundEven, Rem, InverseSqrt, Exp2, Log2, Asin, Acos, Atan, Sinh, Cosh, Tanh]
};

/// A JS scalar known at compile time.
#[derive(Clone, Copy, Debug)]
enum K {
    N(f64),
    B(bool),
}

/// One component while compiling: known, or a register.
#[derive(Clone, Copy, Debug)]
enum C {
    K(K),
    R(u32),
}

/// An operand at run time. Booleans are 0 and 1 in registers and literals.
#[derive(Clone, Copy, Debug)]
pub enum Opnd {
    R(u32),
    L(f64),
}

#[derive(Clone, Debug)]
pub enum Inst {
    Var { dst: u32, slot: u32 },
    Time { dst: u32 },
    Leaf { dst: u32, v: f64 },
    Op { f: Func, a: [u32; 3], dst: u32 },
    /// A curve row at an unknown x: four registers from `dst`.
    Row { row: u32, x: u32, dst: u32 },
    /// A curve row at a known x, one component: `samples[i] * w0 + samples[j] * w1` as compileJs writes it.
    RowAt { row: u32, i: f64, j: f64, w0: f64, w1: f64, dst: u32 },
}

/// A compiled program: instructions over registers, run for several evaluation points ("lanes") at once, each
/// register holding one value per lane (`regs[r * lanes + lane]`). Known values that are operands live in registers
/// filled before the first instruction, so every operation reads registers only.
pub struct Tape {
    pub insts: Vec<Inst>,
    pub regs: usize,
    rows: Vec<Vec<f64>>,
    /// Registers holding written literals, and their values.
    lits: Vec<(u32, f64)>,
    /// Per root, the registers of its components.
    roots: Vec<Vec<u32>>,
    /// Per root, whether it is a boolean (compileJs returns a JS boolean: never a finite number).
    pub root_bool: Vec<bool>,
    /// The variable slots the program reads, each once.
    pub var_slots: Vec<u32>,
}

/// Why a program cannot be compiled here. The packer refuses the node kinds the tape lacks before this.
#[derive(Debug)]
pub struct TapeError(pub String);

impl From<WireError> for TapeError {
    fn from(e: WireError) -> Self { TapeError(e.0) }
}

/// `jsLit`: how a known value is written into the generated code.
fn lit(k: K) -> f64 {
    match k {
        K::N(x) => if x == 0.0 { 0.0 } else { x },
        K::B(b) => if b { 1.0 } else { 0.0 },
    }
}

fn truthy(x: f64) -> bool { x != 0.0 && !x.is_nan() }

/// The value of a scalar op on known operands: the function compileJs both folds with and emits.
fn fold(f: Func, a: &[K]) -> K {
    let n = |i: usize| match a[i] { K::N(x) => x, K::B(b) => if b { 1.0 } else { 0.0 } };
    let b = |i: usize| match a[i] { K::B(b) => b, K::N(x) => truthy(x) };
    K::N(match f {
        Func::Lt => return K::B(n(0) < n(1)),
        Func::Gt => return K::B(n(0) > n(1)),
        Func::Le => return K::B(n(0) <= n(1)),
        Func::Ge => return K::B(n(0) >= n(1)),
        Func::Eq => return K::B(n(0) == n(1)),
        Func::Ne => return K::B(n(0) != n(1)),
        // `a && b`, `a || b` on JS values: the operand itself.
        Func::And => return if b(0) { a[1] } else { a[0] },
        Func::Or => return if b(0) { a[0] } else { a[1] },
        Func::Not => return K::B(!b(0)),
        Func::Select => return if b(0) { a[1] } else { a[2] },
        _ => scalar(f, n(0), if a.len() > 1 { n(1) } else { 0.0 }, if a.len() > 2 { n(2) } else { 0.0 }),
    })
}

/// The numeric ops, as compileJs's EACH table computes them.
#[inline]
fn scalar(f: Func, a: f64, b: f64, c: f64) -> f64 {
    match f {
        Func::Add => a + b,
        Func::Sub => a - b,
        Func::Mul => a * b,
        Func::Div => a / b,
        Func::Min => m::min(a, b),
        Func::Max => m::max(a, b),
        Func::Pow => m::pow(a, b),
        Func::Atan2 => a.atan2(b),
        Func::Mod => a - b * (a / b).floor(),
        Func::Step => if b >= a { 1.0 } else { 0.0 },
        Func::Neg => -a,
        Func::Abs => a.abs(),
        Func::Sign => m::sign(a),
        Func::Floor => a.floor(),
        Func::Sqrt => a.sqrt(),
        Func::Exp => a.exp(),
        Func::Log => a.ln(),
        Func::Sin => a.sin(),
        Func::Cos => a.cos(),
        Func::Tan => a.tan(),
        Func::Fract => a - a.floor(),
        Func::Saturate => m::min(1.0, m::max(0.0, a)),
        Func::Mix => a + (b - a) * c,
        Func::Clamp => m::min(c, m::max(b, a)),
        Func::Smoothstep => m::smooth(a, b, c),
        Func::Ceil => a.ceil(),
        Func::Trunc => a.trunc(),
        Func::RoundEven => m::round_half_even(a),
        Func::Rem => a % b,
        Func::InverseSqrt => 1.0 / a.sqrt(),
        Func::Exp2 => m::pow(2.0, a),
        Func::Log2 => a.log2(),
        Func::Asin => a.asin(),
        Func::Acos => a.acos(),
        Func::Atan => a.atan(),
        Func::Sinh => a.sinh(),
        Func::Cosh => a.cosh(),
        Func::Tanh => a.tanh(),
        Func::Lt => (a < b) as u8 as f64,
        Func::Gt => (a > b) as u8 as f64,
        Func::Le => (a <= b) as u8 as f64,
        Func::Ge => (a >= b) as u8 as f64,
        Func::Eq => (a == b) as u8 as f64,
        Func::Ne => (a != b) as u8 as f64,
        Func::And => if truthy(a) { b } else { a },
        Func::Or => if truthy(a) { a } else { b },
        Func::Not => (!truthy(a)) as u8 as f64,
        Func::Select => if truthy(a) { b } else { c },
        // Expanded while compiling, never an instruction.
        Func::Normalize | Func::Length | Func::Distance | Func::Dot | Func::Cross => f64::NAN,
    }
}

/// An instruction while compiling: operands may still be literals.
enum Pre {
    Var { dst: u32, slot: u32 },
    Time { dst: u32 },
    Leaf { dst: u32, v: f64 },
    Op { f: Func, a: [Opnd; 3], dst: u32 },
    Row { row: u32, x: Opnd, dst: u32 },
    RowAt { row: u32, i: f64, j: f64, w0: f64, w1: f64, dst: u32 },
}

struct Builder {
    insts: Vec<Pre>,
    regs: u32,
}

impl Builder {
    fn reg(&mut self) -> u32 { self.regs += 1; self.regs - 1 }
    fn opnd(c: C) -> Opnd {
        match c { C::K(k) => Opnd::L(lit(k)), C::R(r) => Opnd::R(r) }
    }
    /// compileJs `apply`: computed now when every operand is known, else a local.
    fn apply(&mut self, f: Func, xs: &[C]) -> C {
        if xs.iter().all(|c| matches!(c, C::K(_))) {
            let ks: Vec<K> = xs.iter().map(|c| match c { C::K(k) => *k, _ => unreachable!() }).collect();
            return C::K(fold(f, &ks));
        }
        let dst = self.reg();
        let mut a = [Opnd::L(0.0); 3];
        for (i, c) in xs.iter().enumerate() { a[i] = Self::opnd(*c); }
        self.insts.push(Pre::Op { f, a, dst });
        C::R(dst)
    }
    /// compileJs `identity`: x*0, x*1, x+0, x-0, x/1 on known operands (`===` against 0 and 1; -0 === 0).
    fn identity(f: Func, a: C, b: C) -> Option<C> {
        let is = |c: C, v: f64| matches!(c, C::K(K::N(x)) if x == v);
        match f {
            Func::Mul => if is(a, 0.0) || is(b, 0.0) { Some(C::K(K::N(0.0))) } else if is(a, 1.0) { Some(b) } else if is(b, 1.0) { Some(a) } else { None },
            Func::Add => if is(a, 0.0) { Some(b) } else if is(b, 0.0) { Some(a) } else { None },
            Func::Sub => if is(b, 0.0) { Some(a) } else { None },
            Func::Div => if is(b, 1.0) { Some(a) } else { None },
            _ => None,
        }
    }
    /// compileJs `each`: component by component, scalars broadcast.
    fn each(&mut self, f: Func, args: &[&[C]]) -> Vec<C> {
        let width = args.iter().map(|a| a.len()).max().unwrap_or(0);
        (0..width).map(|i| {
            let xs: Vec<C> = args.iter().map(|a| if a.len() == 1 { a[0] } else { a[i] }).collect();
            let id = if xs.len() == 2 { Self::identity(f, xs[0], xs[1]) } else { None };
            match id { Some(c) => c, None => self.apply(f, &xs) }
        }).collect()
    }
    fn sum(&mut self, cs: &[C]) -> C {
        let mut s = cs[0];
        for c in &cs[1..] { s = self.each(Func::Add, &[&[s], &[*c]])[0]; }
        s
    }
    fn dot(&mut self, a: &[C], b: &[C]) -> C {
        let p = self.each(Func::Mul, &[a, b]);
        self.sum(&p)
    }
}

impl Tape {
    /// Reads one packed program and compiles it. `rows` are the curve rows' samples, by the packer's row index.
    pub fn read(r: &mut Reader) -> Result<Tape, TapeError> {
        let mut rows: Vec<Vec<f64>> = Vec::new();
        let nrows = r.usize()?;
        for _ in 0..nrows { rows.push(r.f64s()?); }
        let n = r.usize()?;
        let mut b = Builder { insts: Vec::with_capacity(n * 2), regs: 0 };
        let mut comps: Vec<Vec<C>> = Vec::with_capacity(n);
        let mut bools: Vec<bool> = Vec::with_capacity(n);
        for _ in 0..n {
            let op = r.u32()?;
            let width = r.usize()?;
            let is_bool = r.u32()? == 1;
            let node: Vec<C> = match op {
                OP_CONST => {
                    let v = r.f64s()?;
                    if is_bool { vec![C::K(K::B(v[0] != 0.0))] } else { v.into_iter().map(|x| C::K(K::N(x))).collect() }
                }
                OP_VAR => {
                    let slot = r.u32()?;
                    // `vars[name]`: a vector variable would be read as name[i]; the packer passes scalars only.
                    let dst = b.reg();
                    b.insts.push(Pre::Var { dst, slot });
                    vec![C::R(dst)]
                }
                OP_LEAF => {
                    let v = r.f64s()?;
                    v.into_iter().map(|x| { let dst = b.reg(); b.insts.push(Pre::Leaf { dst, v: x }); C::R(dst) }).collect()
                }
                OP_TIME => { let dst = b.reg(); b.insts.push(Pre::Time { dst }); vec![C::R(dst)] }
                OP_ROW => {
                    let row = r.u32()?;
                    let x = comps[r.usize()?][0];
                    match x {
                        C::K(k) => {
                            let x = match k { K::N(x) => x, K::B(_) => return Err(TapeError("a boolean curve parameter".into())) };
                            let s = m::min(1.0, m::max(0.0, x)) * (CURVE_N - 1) as f64;
                            let i = m::min((CURVE_N - 2) as f64, s.floor());
                            let t = s - i;
                            (0..4).map(|c| {
                                let dst = b.reg();
                                b.insts.push(Pre::RowAt { row, i: i * 4.0 + c as f64, j: (i + 1.0) * 4.0 + c as f64, w0: lit(K::N(1.0 - t)), w1: lit(K::N(t)), dst });
                                C::R(dst)
                            }).collect()
                        }
                        C::R(_) => {
                            let dst = b.regs;
                            b.regs += 4;
                            b.insts.push(Pre::Row { row, x: Builder::opnd(x), dst });
                            (0..4).map(|c| C::R(dst + c)).collect()
                        }
                    }
                }
                OP_CALL => {
                    let code = r.usize()?;
                    let f = *FNS.get(code).ok_or_else(|| TapeError(format!("no function {code}")))?;
                    let nargs = r.usize()?;
                    let mut args: Vec<Vec<C>> = Vec::with_capacity(nargs);
                    for _ in 0..nargs { args.push(comps[r.usize()?].clone()); }
                    call(&mut b, f, &args)
                }
                OP_VEC => {
                    let nargs = r.usize()?;
                    let mut parts: Vec<C> = Vec::new();
                    for _ in 0..nargs { parts.extend_from_slice(&comps[r.usize()?]); }
                    if parts.len() == 1 { vec![parts[0]; width] } else { parts }
                }
                OP_GET => {
                    let a = r.usize()?;
                    let nsel = r.usize()?;
                    let mut out = Vec::with_capacity(nsel);
                    for _ in 0..nsel { out.push(comps[a][r.usize()?]); }
                    out
                }
                _ => return Err(TapeError(format!("no node kind {op}"))),
            };
            comps.push(node);
            bools.push(is_bool);
        }
        let nroots = r.usize()?;
        let mut root_ops: Vec<Vec<Opnd>> = Vec::with_capacity(nroots);
        let mut root_bool = Vec::with_capacity(nroots);
        for _ in 0..nroots {
            let k = r.usize()?;
            root_ops.push(comps[k].iter().map(|c| Builder::opnd(*c)).collect());
            root_bool.push(bools[k]);
        }
        // Literals into registers after the computed ones, one per distinct value (by bits).
        let mut lits: Vec<(u32, f64)> = Vec::new();
        let mut next = b.regs;
        let mut reg_of = |o: Opnd, lits: &mut Vec<(u32, f64)>| -> u32 {
            match o {
                Opnd::R(r) => r,
                Opnd::L(v) => match lits.iter().find(|(_, x)| x.to_bits() == v.to_bits()) {
                    Some((r, _)) => *r,
                    None => { lits.push((next, v)); next += 1; next - 1 }
                },
            }
        };
        let insts: Vec<Inst> = b.insts.into_iter().map(|p| match p {
            Pre::Var { dst, slot } => Inst::Var { dst, slot },
            Pre::Time { dst } => Inst::Time { dst },
            Pre::Leaf { dst, v } => Inst::Leaf { dst, v },
            Pre::Op { f, a, dst } => Inst::Op { f, a: [reg_of(a[0], &mut lits), reg_of(a[1], &mut lits), reg_of(a[2], &mut lits)], dst },
            Pre::Row { row, x, dst } => Inst::Row { row, x: reg_of(x, &mut lits), dst },
            Pre::RowAt { row, i, j, w0, w1, dst } => Inst::RowAt { row, i, j, w0, w1, dst },
        }).collect();
        let roots: Vec<Vec<u32>> = root_ops.into_iter().map(|cs| cs.into_iter().map(|o| reg_of(o, &mut lits)).collect()).collect();
        let mut var_slots: Vec<u32> = insts.iter().filter_map(|i| match i { Inst::Var { slot, .. } => Some(*slot), _ => None }).collect();
        var_slots.sort_unstable();
        var_slots.dedup();
        Ok(Tape { insts, regs: next as usize, rows, lits, roots, root_bool, var_slots })
    }

    pub fn root_count(&self) -> usize { self.roots.len() }
    /// Root k's component registers: lane l of component c is `regs[root(k)[c] * lanes + l]`.
    pub fn root(&self, k: usize) -> &[u32] { &self.roots[k] }

    /// Runs the program for `lanes` points: lane l reads variable slot s at `vars[l * slots + s]` and the clock at
    /// `times[l]`. Every lane computes exactly what one compileJs call computes.
    pub fn run(&self, lanes: usize, slots: usize, vars: &[f64], times: &[f64], regs: &mut Vec<f64>) {
        let need = self.regs * lanes;
        if regs.len() < need { regs.resize(need, 0.0); }
        let p = regs.as_mut_ptr();
        let n = lanes;
        // SAFETY: every register index is below self.regs (assigned while compiling), so r * n + l < need.
        unsafe {
            for &(r, v) in &self.lits { for l in 0..n { *p.add(r as usize * n + l) = v; } }
            for inst in &self.insts {
                match *inst {
                    // An unset variable reads as undefined in JS: NaN in arithmetic.
                    Inst::Var { dst, slot } => {
                        let d = dst as usize * n;
                        for l in 0..n { *p.add(d + l) = if (slot as usize) < slots { vars[l * slots + slot as usize] } else { f64::NAN }; }
                    }
                    Inst::Time { dst } => { let d = dst as usize * n; for l in 0..n { *p.add(d + l) = times[l]; } }
                    Inst::Leaf { dst, v } => { let d = dst as usize * n; for l in 0..n { *p.add(d + l) = v; } }
                    Inst::Op { f, a, dst } => op(f, p, a[0] as usize * n, a[1] as usize * n, a[2] as usize * n, dst as usize * n, n),
                    Inst::RowAt { row, i, j, w0, w1, dst } => {
                        let s = &self.rows[row as usize];
                        let v = at(s, i) * w0 + at(s, j) * w1;
                        let d = dst as usize * n;
                        for l in 0..n { *p.add(d + l) = v; }
                    }
                    Inst::Row { row, x, dst } => {
                        let s = &self.rows[row as usize];
                        for l in 0..n {
                            let x = *p.add(x as usize * n + l);
                            let sp = m::min(1.0, m::max(0.0, x)) * (CURVE_N - 1) as f64;
                            let i = m::min((CURVE_N - 2) as f64, sp.floor());
                            let t = sp - i;
                            for c in 0..4usize {
                                let a = at(s, c as f64 + i * 4.0);
                                let b2 = at(s, c as f64 + (i + 1.0) * 4.0);
                                *p.add((dst as usize + c) * n + l) = a * (1.0 - t) + b2 * t;
                            }
                        }
                    }
                }
            }
        }
    }
}

/// One operation over every lane. The destination is a fresh register, never an operand.
#[inline(always)]
unsafe fn op(f: Func, p: *mut f64, a: usize, b: usize, c: usize, d: usize, n: usize) {
    // Slices, not pointers: the destination is a fresh register, so it never overlaps an operand, and the loops
    // vectorise over lanes.
    let x = std::slice::from_raw_parts(p.add(a), n);
    let y = std::slice::from_raw_parts(p.add(b), n);
    let z = std::slice::from_raw_parts(p.add(c), n);
    let out = std::slice::from_raw_parts_mut(p.add(d), n);
    macro_rules! lanes {
        (|$x:ident| $e:expr) => { for (o, &$x) in out.iter_mut().zip(x) { *o = $e; } };
        (|$x:ident, $y:ident| $e:expr) => { for ((o, &$x), &$y) in out.iter_mut().zip(x).zip(y) { *o = $e; } };
        (|$x:ident, $y:ident, $z:ident| $e:expr) => { for (((o, &$x), &$y), &$z) in out.iter_mut().zip(x).zip(y).zip(z) { *o = $e; } };
    }
    match f {
        Func::Add => lanes!(|x, y| x + y),
        Func::Sub => lanes!(|x, y| x - y),
        Func::Mul => lanes!(|x, y| x * y),
        Func::Div => lanes!(|x, y| x / y),
        Func::Neg => lanes!(|x| -x),
        Func::Sqrt => lanes!(|x| x.sqrt()),
        Func::Abs => lanes!(|x| x.abs()),
        Func::Min => lanes!(|x, y| m::min(x, y)),
        Func::Max => lanes!(|x, y| m::max(x, y)),
        _ => lanes!(|x, y, z| scalar(f, x, y, z)),
    }
}

/// `arr[i]` in JS: undefined (NaN in arithmetic) unless i is a whole index in range.
#[inline]
fn at(s: &[f64], i: f64) -> f64 {
    if i >= 0.0 && i.fract() == 0.0 && (i as usize) < s.len() { s[i as usize] } else { f64::NAN }
}

/// compileJs `node` for a call.
fn call(b: &mut Builder, f: Func, a: &[Vec<C>]) -> Vec<C> {
    match f {
        Func::Normalize => {
            if a[0].len() == 1 { return b.each(Func::Sign, &[&a[0]]); }
            let d = b.dot(&a[0], &a[0]);
            let l = b.apply(Func::Sqrt, &[d]);
            b.each(Func::Div, &[&a[0], &[l]])
        }
        Func::Length => { let d = b.dot(&a[0], &a[0]); vec![b.apply(Func::Sqrt, &[d])] }
        Func::Distance => {
            let d = b.each(Func::Sub, &[&a[0], &a[1]]);
            let s = b.dot(&d, &d);
            vec![b.apply(Func::Sqrt, &[s])]
        }
        Func::Dot => vec![b.dot(&a[0], &a[1])],
        Func::Cross => {
            let (x, y) = (&a[0], &a[1]);
            let mut term = |i: usize, j: usize| {
                let p = b.each(Func::Mul, &[&[x[i]], &[y[j]]]);
                let q = b.each(Func::Mul, &[&[x[j]], &[y[i]]]);
                b.each(Func::Sub, &[&p, &q])[0]
            };
            let c0 = term(1, 2);
            let c1 = term(2, 0);
            let c2 = term(0, 1);
            vec![c0, c1, c2]
        }
        Func::Select => {
            let c = a[0][0];
            let width = a[1].len().max(a[2].len());
            let pick = |v: &Vec<C>, i: usize| if v.len() == 1 { v[0] } else { v[i] };
            (0..width).map(|i| match c {
                C::K(K::B(t)) => if t { pick(&a[1], i) } else { pick(&a[2], i) },
                _ => b.apply(Func::Select, &[c, pick(&a[1], i), pick(&a[2], i)]),
            }).collect()
        }
        _ => {
            let refs: Vec<&[C]> = a.iter().map(|v| v.as_slice()).collect();
            b.each(f, &refs)
        }
    }
}
