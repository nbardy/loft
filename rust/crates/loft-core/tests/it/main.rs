//! Lane 1.3 pass/fail checks (lean Rust plan §8.2): (a) tape vs `compileJs`, (b) interval soundness, (c) grad vs
//! central differences, (d) a digest of tape bits to compare across hosts. Heavy counts: run with `--release`.

mod emit;
mod kernels;

use loft_core::grad::grad;
use loft_core::interval::{I, enclose};
use loft_core::prim::{ATOMS, Atom, PRIMS, Prim, TyRule, atom};
use loft_core::tape::{Id, Ins, Ssa, run};
use serde_json::Value;

const ALL_ATOMS: [Atom; 40] = {
    use Atom::*;
    [
        Add, Sub, Mul, Div, Neg, Min, Max, Pow, Atan2, Rem, Abs, Sign, Floor, Ceil, Trunc, RoundEven, Sqrt, Exp, Exp2,
        Log, Log2, Sin, Cos, Tan, Asin, Acos, Atan, Sinh, Cosh, Tanh, Lt, Gt, Le, Ge, Eq, Ne, And, Or, Not, Select,
    ]
};
const ALL_PRIMS: [Prim; 53] = {
    use Prim::*;
    [
        Add,
        Sub,
        Mul,
        Div,
        Neg,
        Min,
        Max,
        Pow,
        Mod,
        Atan2,
        Step,
        Abs,
        Sign,
        Floor,
        Fract,
        Sqrt,
        Exp,
        Log,
        Sin,
        Cos,
        Tan,
        Saturate,
        Normalize,
        Mix,
        Clamp,
        Smoothstep,
        Length,
        Distance,
        Dot,
        Cross,
        Lt,
        Gt,
        Le,
        Ge,
        Eq,
        Ne,
        And,
        Or,
        Not,
        Select,
        Ceil,
        Trunc,
        RoundEven,
        Rem,
        InverseSqrt,
        Exp2,
        Log2,
        Asin,
        Acos,
        Atan,
        Sinh,
        Cosh,
        Tanh,
    ]
};

/// The exact operation in f64 (`libm`'s f64 functions, < 1 f64 ulp): the reference for ulps and differences.
fn exact(a: Atom, x: f64, y: f64, z: f64) -> f64 {
    use libm as m;
    let bit = |p: bool| if p { 1.0 } else { 0.0 };
    let t = |p: f64| p != 0.0 && !p.is_nan();
    match a {
        Atom::Add => x + y,
        Atom::Sub => x - y,
        Atom::Mul => x * y,
        Atom::Div => x / y,
        Atom::Neg => -x,
        Atom::Min => {
            if x.is_nan() || y.is_nan() {
                f64::NAN
            } else {
                x.min(y)
            }
        }
        Atom::Max => {
            if x.is_nan() || y.is_nan() {
                f64::NAN
            } else {
                x.max(y)
            }
        }
        Atom::Pow => m::pow(x, y),
        Atom::Atan2 => m::atan2(x, y),
        Atom::Rem => m::fmod(x, y),
        Atom::Abs => x.abs(),
        Atom::Sign => {
            if x > 0.0 {
                1.0
            } else if x < 0.0 {
                -1.0
            } else {
                x
            }
        }
        Atom::Floor => x.floor(),
        Atom::Ceil => x.ceil(),
        Atom::Trunc => x.trunc(),
        Atom::RoundEven => m::rint(x),
        Atom::Sqrt => x.sqrt(),
        Atom::Exp => m::exp(x),
        Atom::Exp2 => m::exp2(x),
        Atom::Log => m::log(x),
        Atom::Log2 => m::log2(x),
        Atom::Sin => m::sin(x),
        Atom::Cos => m::cos(x),
        Atom::Tan => m::tan(x),
        Atom::Asin => m::asin(x),
        Atom::Acos => m::acos(x),
        Atom::Atan => m::atan(x),
        Atom::Sinh => m::sinh(x),
        Atom::Cosh => m::cosh(x),
        Atom::Tanh => m::tanh(x),
        Atom::Lt => bit(x < y),
        Atom::Gt => bit(x > y),
        Atom::Le => bit(x <= y),
        Atom::Ge => bit(x >= y),
        Atom::Eq => bit(x == y),
        Atom::Ne => bit(x != y),
        Atom::And => {
            if t(x) {
                y
            } else {
                x
            }
        }
        Atom::Or => {
            if t(x) {
                x
            } else {
                y
            }
        }
        Atom::Not => bit(!t(x)),
        Atom::Select => {
            if t(x) {
                y
            } else {
                z
            }
        }
    }
}
fn run64(s: &Ssa, leaves: &[f64]) -> Vec<f64> {
    let mut v: Vec<f64> = Vec::with_capacity(s.ins.len());
    for i in &s.ins {
        let x = match *i {
            Ins::Lit(c) => c as f64,
            Ins::Leaf(k) => leaves[k as usize],
            Ins::Op(a, [p, q, r]) => exact(a, v[p as usize], v[q as usize], v[r as usize]),
        };
        v.push(x);
    }
    v
}
fn run32(s: &Ssa, leaves: &[f32]) -> Vec<f32> {
    let mut r = Vec::new();
    run(s, 1, leaves.len(), leaves, &mut r).unwrap();
    r
}

/// |got - want| in f32 ulps of want (0 when both are the same non-finite value).
fn ulps(got: f32, want: f64) -> f64 {
    if got.is_nan() && want.is_nan() || got as f64 == want {
        return 0.0;
    }
    let w = want as f32;
    let ulp =
        if w.is_finite() { (f32::from_bits(w.abs().to_bits() + 1) - w.abs()).max(f32::from_bits(1)) } else { f32::MAX };
    (got as f64 - want).abs() / ulp as f64
}

/// 10^e through `libm`, not the host's `pow`, so the random inputs (and the digest of (d)) are the same everywhere.
fn p10(e: f64) -> f64 {
    libm::pow(10.0, e)
}

struct Rng(u64);
impl Rng {
    fn u(&mut self) -> f64 {
        self.0 = self.0.wrapping_add(0x9e3779b97f4a7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d049bb133111eb);
        ((z ^ (z >> 31)) >> 11) as f64 / (1u64 << 53) as f64
    }
    fn n(&mut self, n: usize) -> usize {
        (self.u() * n as f64) as usize % n
    }
    fn range(&mut self, lo: f64, hi: f64) -> f64 {
        lo + (hi - lo) * self.u()
    }
}

// ---- (a) tape against compileJs -------------------------------------------------------------------------------------

struct Fixture {
    name: String,
    ssa: Ssa,
    root: Vec<Id>,
    slots: usize,
    points: Vec<Vec<f32>>,
    want: Vec<Vec<f64>>,
}

fn fixtures() -> Vec<Fixture> {
    let path = std::env::var("LR13_MATH")
        .unwrap_or_else(|_| concat!(env!("CARGO_MANIFEST_DIR"), "/../../../out/lr13/math.json").into());
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| {
        panic!("{path}: {e}. Write it first: bun rust/crates/loft-core/dump_math.ts > out/lr13/math.json")
    });
    let doc: Value = serde_json::from_str(&text).unwrap();
    let u = |v: &Value| v.as_u64().unwrap() as usize;
    doc["cases"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| {
            let mut s = Ssa::default();
            let mut comps: Vec<Vec<Id>> = Vec::new();
            for n in c["nodes"].as_array().unwrap() {
                let cs = match n["k"].as_str().unwrap() {
                    "lit" => n["v"].as_array().unwrap().iter().map(|x| s.lit(x.as_f64().unwrap() as f32)).collect(),
                    "leaf" => vec![s.leaf(u(&n["slot"]) as u32)],
                    "call" => {
                        let args: Vec<Vec<Id>> =
                            n["args"].as_array().unwrap().iter().map(|a| comps[u(a)].clone()).collect();
                        let refs: Vec<&[Id]> = args.iter().map(|a| a.as_slice()).collect();
                        s.apply(ALL_PRIMS[u(&n["fn"])], &refs)
                    }
                    "vec" => {
                        let parts: Vec<Id> =
                            n["args"].as_array().unwrap().iter().flat_map(|a| comps[u(a)].clone()).collect();
                        if parts.len() == 1 { vec![parts[0]; u(&n["w"])] } else { parts }
                    }
                    "get" => n["sel"].as_array().unwrap().iter().map(|k| comps[u(&n["a"])][u(k)]).collect(),
                    k => panic!("node kind {k}"),
                };
                comps.push(cs);
            }
            let f32s = |v: &Value| v.as_array().unwrap().iter().map(|x| x.as_f64().unwrap() as f32).collect::<Vec<_>>();
            let f64s = |v: &Value| v.as_array().unwrap().iter().map(|x| x.as_f64().unwrap()).collect::<Vec<_>>();
            Fixture {
                name: c["name"].as_str().unwrap().into(),
                root: comps[u(&c["root"])].clone(),
                slots: c["slots"].as_array().unwrap().len(),
                points: c["points"].as_array().unwrap().iter().map(f32s).collect(),
                want: c["want"].as_array().unwrap().iter().map(f64s).collect(),
                ssa: s,
            }
        })
        .collect()
}

/// Cases wider than 4 ulps, by name, with the measured bound. Each is cancellation: a sum or difference whose result
/// is far smaller than its terms, so a few ulps of the terms are many ulps of the result. Every case, these included,
/// is also held to 1e-6 of max(1, |want|) (measured worst 5.03e-7).
const WIDER: [(&str, f64); 6] = [
    ("op:dot", 152.0),
    ("op:cross", 47.0),
    ("op:mod", 8.0), // dot/cross: sums of products; mod: a - b floor(a / b)
    ("expr.test#0", 71.0),
    ("expr.test#2", 4.5),
    ("expr.test#3", 202.0), // sin(7u) g + cos(t); normalize(cross); mod
];

#[test]
fn a_tape_matches_compile_js() {
    let fx = fixtures();
    let mut fails = Vec::new();
    println!("case                     max ulps   max |err|/max(1,|want|)");
    for f in &fx {
        let (mut worst, mut rel) = (0.0f64, 0.0f64);
        let mut regs = Vec::new();
        for (p, want) in f.points.iter().zip(&f.want) {
            run(&f.ssa, 1, f.slots, p, &mut regs).unwrap();
            for (&r, &w) in f.root.iter().zip(want) {
                let g = regs[r as usize];
                worst = worst.max(ulps(g, w));
                rel = rel.max((g as f64 - w).abs() / w.abs().max(1.0));
            }
        }
        println!("{:<24} {:>9.2}   {:.2e}", f.name, worst, rel);
        let bound = WIDER.iter().find(|(n, _)| *n == f.name).map_or(4.0, |w| w.1);
        if worst > bound {
            fails.push(format!("{} {worst:.1} ulps > {bound}", f.name));
        }
        if rel > 1e-6 {
            fails.push(format!("{} {rel:.2e} relative > 1e-6", f.name));
        }
    }
    println!("{} cases", fx.len());
    assert!(fails.is_empty(), "{fails:#?}");
}

/// The premise of the interval fold: each atom's f32 result is within `ulps` of the exact one.
#[test]
fn a_atom_ulps_hold() {
    let mut r = Rng(7);
    let mut fails = Vec::new();
    for a in ALL_ATOMS {
        let d = atom(a);
        let mut worst = 0.0f64;
        for _ in 0..200_000 {
            let mut x = [0f32; 3];
            for v in &mut x {
                *v = (r.range(-1.0, 1.0) * p10(r.range(-3.0, 1.6))) as f32;
            }
            if matches!(a, Atom::Pow | Atom::Log | Atom::Log2 | Atom::Sqrt) {
                x[0] = x[0].abs()
            }
            if matches!(a, Atom::Asin | Atom::Acos) {
                x[0] = x[0].clamp(-1.0, 1.0)
            }
            let want = exact(a, x[0] as f64, x[1] as f64, x[2] as f64);
            let got = (d.f)(x[0], x[1], x[2]);
            if want.is_finite() && want.abs() < f32::MAX as f64 {
                worst = worst.max(ulps(got, want));
            }
        }
        println!("{a:?}: max {worst:.3} ulps (bound {})", d.ulps);
        if worst > d.ulps as f64 {
            fails.push(format!("{a:?} {worst:.3} > {}", d.ulps));
        }
    }
    assert!(fails.is_empty(), "{fails:#?}");
}

// ---- random programs for (b) and (c) ---------------------------------------------------------------------------------

/// A random program over `leaves` leaves through authored operators (expansions included), `n` operators deep.
fn random_program(r: &mut Rng, leaves: u32, n: usize) -> Ssa {
    let mut s = Ssa::default();
    let mut pool: Vec<Id> = (0..leaves).map(|k| s.leaf(k)).collect();
    for _ in 0..3 {
        let c = s.lit(r.range(-3.0, 3.0) as f32);
        pool.push(c);
    }
    for _ in 0..n {
        let p = ALL_PRIMS[r.n(ALL_PRIMS.len())];
        let pick = |r: &mut Rng, pool: &Vec<Id>| pool[pool.len() - 1 - r.n(pool.len().min(6))];
        // Vector builtins get vector operands of one width; the rest are componentwise, so scalars suffice.
        let width = match PRIMS[p as usize].ty {
            TyRule::Vec3 => 3,
            TyRule::Same | TyRule::Scalar => 1 + r.n(3),
            TyRule::Widest | TyRule::Compare | TyRule::Logic | TyRule::Pick => 1,
        };
        let args: Vec<Vec<Id>> =
            (0..PRIMS[p as usize].arity).map(|_| (0..width).map(|_| pick(r, &pool)).collect()).collect();
        let refs: Vec<&[Id]> = args.iter().map(|a| a.as_slice()).collect();
        let out = s.apply(p, &refs);
        pool.extend(out);
    }
    s
}

// ---- (b) interval soundness -------------------------------------------------------------------------------------------

fn boxes_check(
    s: &Ssa,
    leaves: usize,
    r: &mut Rng,
    boxes: usize,
    esc: &mut u64,
    unb: &mut u64,
    names: &mut Vec<String>,
) {
    let mut regs = Vec::new();
    for _ in 0..boxes {
        let bx: Vec<I> = (0..leaves)
            .map(|_| {
                let c = r.range(-1.0, 1.0) * p10(r.range(-2.0, 1.0));
                let w = p10(r.range(-6.0, 0.7));
                let (lo, hi) = ((c - w) as f32 as f64, (c + w) as f32 as f64);
                [lo, hi]
            })
            .collect();
        let Ok(iv) = enclose(s, &bx) else {
            *unb += 1;
            continue;
        };
        let mut pts = vec![0f32; 16 * leaves];
        for l in 0..16 {
            for k in 0..leaves {
                let [lo, hi] = bx[k];
                pts[l * leaves + k] = match l {
                    0 => lo as f32,
                    1 => hi as f32,
                    _ => (r.range(lo, hi) as f32).clamp(lo as f32, hi as f32),
                };
            }
        }
        run(s, 16, leaves, &pts, &mut regs).unwrap();
        for (k, x) in iv.iter().enumerate() {
            for l in 0..16 {
                let v = regs[k * 16 + l] as f64;
                if !(v >= x[0] && v <= x[1]) {
                    *esc += 1;
                    if names.len() < 10 {
                        names.push(format!("{:?} = {v} outside {x:?}", s.ins[k]));
                    }
                }
            }
        }
    }
}

#[test]
fn b_intervals_enclose_the_tape() {
    let mut r = Rng(11);
    let (mut esc, mut unb, mut names, mut total) = (0u64, 0u64, Vec::new(), 0usize);
    // Every atom alone, 12k boxes each (480k), then 26k random programs x 20 boxes (520k).
    for a in ALL_ATOMS {
        let mut s = Ssa::default();
        let x = [s.leaf(0), s.leaf(1), s.leaf(2)];
        s.op(a, x);
        boxes_check(&s, 3, &mut r, 12_000, &mut esc, &mut unb, &mut names);
        total += 12_000;
    }
    for _ in 0..26_000 {
        let s = random_program(&mut r, 3, 12);
        boxes_check(&s, 3, &mut r, 20, &mut esc, &mut unb, &mut names);
        total += 20;
    }
    println!("boxes {total}, points {}, unbounded boxes {unb}, escapes {esc}", total * 16);
    assert_eq!(esc, 0, "{names:#?}");
}

// ---- (c) derivatives against central differences ---------------------------------------------------------------------

#[test]
fn c_grad_matches_central_differences() {
    let mut r = Rng(13);
    let (mut n, mut skipped, mut fails) = (0usize, 0usize, Vec::new());
    let mut progs: Vec<Ssa> = (0..2000).map(|_| random_program(&mut r, 2, 10)).collect();
    // Every atom alone too.
    for a in ALL_ATOMS {
        let mut s = Ssa::default();
        let x = [s.leaf(0), s.leaf(1), s.leaf(2)];
        s.op(a, x);
        progs.push(s);
    }
    for s0 in &progs {
        let leaves = s0.slots().max(1);
        let root = (s0.ins.len() - 1) as Id;
        for wrt in 0..leaves as u32 {
            let mut s = s0.clone();
            let d = grad(&mut s, &[root], wrt)[0];
            for _ in 0..8 {
                let p: Vec<f32> =
                    (0..leaves).map(|_| (r.range(0.1, 2.0) * if r.u() < 0.3 { -1.0 } else { 1.0 }) as f32).collect();
                let g = run32(&s, &p)[d as usize] as f64;
                let at = |dx: f64| {
                    let mut q: Vec<f64> = p.iter().map(|&x| x as f64).collect();
                    q[wrt as usize] += dx;
                    run64(s0, &q)[root as usize]
                };
                let h = 1e-6 * (p[wrt as usize].abs() as f64).max(1.0);
                let (fp, fm, f0) = (at(h), at(-h), at(0.0));
                let (c, fwd, bwd) = ((fp - fm) / (2.0 * h), (fp - f0) / h, (f0 - fm) / h);
                // A kink or jump between the samples, or a value off the reals: no derivative to compare.
                let scale = c.abs().max(1.0);
                if !(g.is_finite() && c.is_finite()) || (fwd - bwd).abs() > 1e-2 * scale {
                    skipped += 1;
                    continue;
                }
                n += 1;
                if (g - c).abs() > 1e-3 * scale && fails.len() < 10 {
                    fails.push(format!("{:?} d/d{wrt} at {p:?}: grad {g} fd {c}", s0.ins.last().unwrap()));
                }
            }
        }
    }
    println!("compared {n}, skipped (kink, jump or non-finite) {skipped}, failures {}", fails.len());
    assert!(fails.is_empty(), "{fails:#?}");
}

// ---- (d) the same bits everywhere --------------------------------------------------------------------------------------

/// FNV-1a over every register of the fixture corpus and 500 random programs: compare the printed digest across hosts.
#[test]
fn d_tape_digest() {
    let mut h: u64 = 0xcbf29ce484222325;
    let mut eat = |x: f32| {
        for b in x.to_bits().to_le_bytes() {
            h = (h ^ b as u64).wrapping_mul(0x100000001b3);
        }
    };
    let mut regs = Vec::new();
    let mut r = Rng(17);
    for _ in 0..500 {
        let s = random_program(&mut r, 3, 16);
        let pts: Vec<f32> = (0..64 * 3).map(|_| (r.range(-1.0, 1.0) * p10(r.range(-3.0, 2.0))) as f32).collect();
        run(&s, 64, 3, &pts, &mut regs).unwrap();
        regs.iter().for_each(|&x| eat(x));
    }
    for f in fixtures() {
        for p in &f.points {
            run(&f.ssa, 1, f.slots, p, &mut regs).unwrap();
            regs.iter().for_each(|&x| eat(x));
        }
    }
    println!("tape digest {h:016x} ({} atoms in table)", ATOMS.len());
}
