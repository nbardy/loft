//! Lane G2. The CPU reading of an expression. Like compileJs, the compiler
//! scalarises vectors and folds known operands before the program runs. Each
//! nonconstant operation is a local and each loop body is compiled once.

mod arena;
mod grad;
mod math;
mod ops;
mod twins;
mod wire;
pub use math::hypot;
pub use arena::compile_js;
pub use grad::{DerivativeRule, Differentiation, Footprint, ScreenRate};
pub use wire::probe;

use crate::error::Error;
use crate::graph::{LibJs, Prim, RowId, TableId, Ty};
use ops::Scalar;
use std::collections::HashMap;
use twins::Twin;

/// A CPU reading at the boundary of a compiled expression.
#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Number(f64),
    Vector(Vec<f64>),
    Bool(bool),
    Undefined,
}

/// How a live curve table addresses the authored rows. Each row holds 64 vec4s.
pub const CURVE_N: usize = 64;

#[derive(Clone, Copy, Default)]
pub enum RowSource<'a> {
    #[default]
    Own,
    Table(&'a dyn Fn(RowId) -> usize),
}

/// A frame's CPU bindings. Missing inputs and parameters take their defaults.
/// A table callback is called once per table, in first-use order.
#[derive(Default)]
pub struct Env<'a> {
    pub vars: HashMap<&'a str, Value>,
    pub inputs: HashMap<&'a str, Value>,
    pub params: HashMap<&'a str, f64>,
    pub curves: Option<&'a [f32]>,
    pub tables: Option<&'a dyn Fn(TableId) -> &'a [f32]>,
}

/// A component during compilation: either a constant or one local's slot.
#[derive(Clone, Copy)]
enum C {
    Known(Scalar),
    Local(usize),
}

impl C {
    const ZERO: Self = Self::Known(Scalar::Number(0.0));

    fn read(self, locals: &[Scalar]) -> Scalar {
        match self {
            Self::Known(Scalar::Number(x)) => Scalar::Number(math::source_number(x)),
            Self::Known(x @ (Scalar::Bool(_) | Scalar::Undefined)) => x,
            Self::Local(slot) => locals[slot],
        }
    }
}

/// A vector with one entry broadcasts, as in compileJs's at().
fn at(components: &[C], i: usize) -> C {
    components[if components.len() == 1 { 0 } else { i }]
}

/// Exact identities that make a sum of mostly-zero weights cost only its terms.
/// For a non-finite x, x * 0 gives 0 where the GPU would give NaN.
fn identity(fn_: Prim, a: C, b: C) -> Option<C> {
    let is = |c, x| matches!(c, C::Known(Scalar::Number(n)) if n == x);
    match fn_ {
        Prim::Mul if is(a, 0.0) || is(b, 0.0) => Some(C::ZERO),
        Prim::Mul if is(a, 1.0) => Some(b),
        Prim::Mul | Prim::Div if is(b, 1.0) => Some(a),
        Prim::Add if is(a, 0.0) => Some(b),
        Prim::Add | Prim::Sub if is(b, 0.0) => Some(a),
        Prim::Add
        | Prim::Sub
        | Prim::Mul
        | Prim::Div
        | Prim::Neg
        | Prim::Min
        | Prim::Max
        | Prim::Pow
        | Prim::Mod
        | Prim::Atan2
        | Prim::Step
        | Prim::Abs
        | Prim::Sign
        | Prim::Floor
        | Prim::Fract
        | Prim::Sqrt
        | Prim::Exp
        | Prim::Log
        | Prim::Sin
        | Prim::Cos
        | Prim::Tan
        | Prim::Saturate
        | Prim::Normalize
        | Prim::Mix
        | Prim::Clamp
        | Prim::Smoothstep
        | Prim::Length
        | Prim::Distance
        | Prim::Dot
        | Prim::Cross
        | Prim::Lt
        | Prim::Gt
        | Prim::Le
        | Prim::Ge
        | Prim::Eq
        | Prim::Ne
        | Prim::And
        | Prim::Or
        | Prim::Not
        | Prim::Select
        | Prim::Ceil
        | Prim::Trunc
        | Prim::RoundEven
        | Prim::Rem
        | Prim::InverseSqrt
        | Prim::Exp2
        | Prim::Log2
        | Prim::Asin
        | Prim::Acos
        | Prim::Atan
        | Prim::Sinh
        | Prim::Cosh
        | Prim::Tanh => None,
    }
}

enum Leaf<'a> {
    Var(&'a str),
    Input(&'a str, [f64; 4]),
    Param(&'a str, f64),
}

enum Curve<'a> {
    Own(&'a [f32]),
    Table(usize),
}

enum RowAt {
    Known { index: f64, t: f64 },
    Dynamic(C),
}

impl RowAt {
    fn weights(x: f64) -> (f64, f64) {
        let s = math::min(1.0, math::max(0.0, x)) * (CURVE_N - 1) as f64;
        let i = math::min((CURVE_N - 2) as f64, s.floor());
        (i, s - i)
    }

    fn new(x: C) -> Self {
        match x {
            C::Known(x) => {
                let (index, t) = Self::weights(x.number());
                Self::Known { index, t }
            }
            C::Local(_) => Self::Dynamic(x),
        }
    }
}

enum FetchAt {
    Known(f64),
    Dynamic { index: C, stride: f64, lane: f64 },
}

impl FetchAt {
    /// The GPU truncates toward zero. The generated JS uses ToInt32 for a dynamic index.
    fn new(index: C, lanes: u32, lane: u32) -> Self {
        match index {
            C::Known(i) => Self::Known((i.number().trunc() * f64::from(lanes) + f64::from(lane)) * 4.0),
            C::Local(_) => Self::Dynamic { index, stride: f64::from(lanes) * 4.0, lane: f64::from(lane) * 4.0 },
        }
    }

    fn offset(&self, locals: &[Scalar]) -> f64 {
        match *self {
            Self::Known(offset) => offset,
            Self::Dynamic { index, stride, lane } => {
                f64::from(math::to_uint32(index.read(locals).number()) as i32) * stride + lane
            }
        }
    }
}

enum Step<'a> {
    Each(usize, Prim, [C; 3]),
    Leaf(usize, usize, Leaf<'a>),
    Row(usize, Curve<'a>, RowAt),
    Fetch(usize, usize, FetchAt),
    Twin(usize, usize, Twin, Vec<Vec<C>>),
    Library(usize, Ty, LibJs, Vec<Vec<C>>),
    RawCompute(&'a str),
    Loop(Loop<'a>),
}

struct Loop<'a> {
    count: C,
    state: Vec<(usize, C)>,
    index: usize,
    body: Vec<Step<'a>>,
    next: Vec<C>,
    stop: Option<C>,
}

/// The program and its constants can be reused for different frames.
pub struct Compiled<'a> {
    steps: Vec<Step<'a>>,
    tables: Vec<(TableId, &'a str)>,
    need_curves: bool,
    slots: usize,
    roots: Vec<(Ty, Vec<C>)>,
}

impl Compiled<'_> {
    pub fn run(&self, env: &Env<'_>) -> Result<Vec<Value>, Error> {
        if self.need_curves && env.curves.is_none() {
            return Err(Error::Scene("compileJs: this function reads curves from a table; pass Env.curves".into()));
        }
        // Tables are looked up before anything else runs, exactly once per call.
        let mut tables = Vec::with_capacity(self.tables.len());
        if let Some((_, name)) = self.tables.first() {
            let table = env.tables.ok_or_else(|| {
                Error::Scene(format!("compileJs: this function reads the per-frame table \"{name}\"; pass Env.tables"))
            })?;
            tables.extend(self.tables.iter().map(|&(id, _)| table(id)));
        }
        let mut frame = Frame { env, tables, locals: vec![Scalar::Undefined; self.slots] };
        frame.run(&self.steps)?;
        Ok(self.roots.iter().map(|(ty, c)| frame.value(*ty, c)).collect())
    }
}

struct Frame<'a, 'env> {
    env: &'a Env<'env>,
    tables: Vec<&'env [f32]>,
    locals: Vec<Scalar>,
}

impl Frame<'_, '_> {
    fn value(&self, ty: Ty, c: &[C]) -> Value {
        match ty {
            Ty::F32 | Ty::U32 | Ty::Bool => match c[0].read(&self.locals) {
                Scalar::Number(x) => Value::Number(if x.is_nan() { f64::NAN } else { x }),
                Scalar::Bool(b) => Value::Bool(b),
                Scalar::Undefined => Value::Undefined,
            },
            Ty::V2 | Ty::V3 | Ty::V4 => Value::Vector(
                c.iter()
                    .map(|c| {
                        let x = c.read(&self.locals).number();
                        if x.is_nan() { f64::NAN } else { x }
                    })
                    .collect(),
            ),
            Ty::Kind(_) => unreachable!("compile_js accepts numeric and bool expressions"),
        }
    }

    fn run(&mut self, steps: &[Step<'_>]) -> Result<(), Error> {
        for instruction in steps {
            match instruction {
                Step::Each(dst, fn_, xs) => self.locals[*dst] = ops::each(*fn_, xs.map(|c| c.read(&self.locals))),
                Step::Leaf(dst, n, leaf) => self.leaf(*dst, *n, leaf),
                Step::Row(dst, source, index) => self.row(*dst, source, index),
                Step::Fetch(dst, table, at) => self.fetch(*dst, *table, at),
                Step::Twin(dst, n, twin, args) => self.twin(*dst, *n, *twin, args),
                Step::Library(dst, ty, js, args) => self.library(*dst, *ty, *js, args)?,
                Step::RawCompute(name) => {
                    return Err(Error::Scene(format!("the CPU twin cannot read GPU compute output \"{name}\"")));
                }
                Step::Loop(l) => self.loop_(l)?,
            }
        }
        Ok(())
    }

    fn leaf(&mut self, dst: usize, n: usize, leaf: &Leaf<'_>) {
        let value = match leaf {
            Leaf::Var(name) => self.env.vars.get(name),
            Leaf::Input(name, default) => {
                if let Some(v) = self.env.inputs.get(name) {
                    Some(v)
                } else {
                    for (out, x) in self.locals[dst..dst + n].iter_mut().zip(default) {
                        *out = Scalar::Number(*x);
                    }
                    return;
                }
            }
            Leaf::Param(name, default) => {
                self.locals[dst] = Scalar::Number(self.env.params.get(name).copied().unwrap_or(*default));
                return;
            }
        };
        let out = &mut self.locals[dst..dst + n];
        match value {
            Some(Value::Number(x)) => out[0] = Scalar::Number(*x),
            Some(Value::Bool(b)) => out[0] = Scalar::Bool(*b),
            Some(Value::Vector(v)) => {
                for (i, o) in out.iter_mut().enumerate() {
                    *o = v.get(i).map_or(Scalar::Undefined, |&x| Scalar::Number(x));
                }
            }
            Some(Value::Undefined) | None => out.fill(Scalar::Undefined),
        }
    }

    fn row(&mut self, dst: usize, source: &Curve<'_>, at: &RowAt) {
        let (i, t) = match at {
            RowAt::Known { index, t } => (*index, *t),
            RowAt::Dynamic(x) => RowAt::weights(x.read(&self.locals).number()),
        };
        let sample = |index: f64| {
            if index.is_nan() {
                return f64::NAN;
            }
            match source {
                Curve::Own(data) => data.get(index as usize).map_or(f64::NAN, |&x| f64::from(x)),
                Curve::Table(off) => self
                    .env
                    .curves
                    .expect("checked before running")
                    .get(off + index as usize)
                    .map_or(f64::NAN, |&x| f64::from(x)),
            }
        };
        for c in 0..4 {
            let a = sample(i * 4.0 + c as f64);
            let b = sample((i + 1.0) * 4.0 + c as f64);
            self.locals[dst + c] = Scalar::Number(a * (1.0 - t) + b * t);
        }
    }

    fn fetch(&mut self, dst: usize, table: usize, at: &FetchAt) {
        let offset = at.offset(&self.locals);
        for c in 0..4 {
            let i = offset + c as f64;
            self.locals[dst + c] = if i >= 0.0 {
                self.tables[table].get(i as usize).map_or(Scalar::Undefined, |&x| Scalar::Number(f64::from(x)))
            } else {
                Scalar::Undefined
            };
        }
    }

    fn twin(&mut self, dst: usize, n: usize, twin: Twin, args: &[Vec<C>]) {
        let args: Vec<_> = args
            .iter()
            .map(|v| {
                let mut a = [0.0; 4];
                for (x, c) in a.iter_mut().zip(v) {
                    *x = c.read(&self.locals).number();
                }
                a
            })
            .collect();
        for (out, x) in self.locals[dst..dst + n].iter_mut().zip(twin.run(&args)) {
            *out = Scalar::Number(x);
        }
    }

    fn library(&mut self, dst: usize, ty: Ty, js: LibJs, args: &[Vec<C>]) -> Result<(), Error> {
        let args = args.iter().map(|v| v.iter().map(|c| c.read(&self.locals).number()).collect()).collect::<Vec<_>>();
        let values = js(&args)?;
        let out = &mut self.locals[dst..dst + usize::from(ty.lits())];
        if values.len() != out.len() {
            return Err(Error::Internal(format!("compileJs: a {} library twin returned {} components", ty.name(), values.len())));
        }
        for (out, v) in out.iter_mut().zip(values) {
            *out = if ty == Ty::Bool { Scalar::Bool(Scalar::Number(v).truthy()) } else { Scalar::Number(v) };
        }
        Ok(())
    }

    fn loop_(&mut self, l: &Loop<'_>) -> Result<(), Error> {
        for &(dst, init) in &l.state {
            self.locals[dst] = init.read(&self.locals);
        }
        let n = l.count.read(&self.locals).number().trunc();
        let mut i = 0.0;
        let mut next = vec![Scalar::Undefined; l.next.len()];
        while i < n {
            self.locals[l.index] = Scalar::Number(i);
            self.run(&l.body)?;
            // Read every next value before assigning any; a step can swap state components.
            for (value, c) in next.iter_mut().zip(&l.next) {
                *value = c.read(&self.locals);
            }
            for (&(slot, _), &value) in l.state.iter().zip(&next) {
                self.locals[slot] = value;
            }
            if l.stop.is_some_and(|c| c.read(&self.locals).truthy()) {
                break;
            }
            i += 1.0;
        }
        Ok(())
    }
}

impl<'a> Compiled<'a> {
    fn local(&mut self, n: usize) -> usize {
        let slot = self.slots;
        self.slots += n;
        slot
    }

    fn emit(&mut self, n: usize, step: impl FnOnce(usize) -> Step<'a>) -> Vec<C> {
        let dst = self.local(n);
        self.steps.push(step(dst));
        (dst..dst + n).map(C::Local).collect()
    }

    fn apply(&mut self, fn_: Prim, xs: &[C]) -> C {
        let mut args = [C::ZERO; 3];
        args[..xs.len()].copy_from_slice(xs);
        // allKnown, then o.f: evaluate before known numbers are written as JS literals.
        if let [C::Known(a), C::Known(b), C::Known(c)] = args {
            return C::Known(ops::each(fn_, [a, b, c]));
        }
        let dst = self.local(1);
        self.steps.push(Step::Each(dst, fn_, args));
        C::Local(dst)
    }

    fn each(&mut self, fn_: Prim, args: &[&[C]]) -> Vec<C> {
        let n = args.iter().map(|a| a.len()).max().expect("a primitive has operands");
        (0..n)
            .map(|i| {
                let mut xs = [C::ZERO; 3];
                for (x, arg) in xs.iter_mut().zip(args) {
                    *x = at(arg, i);
                }
                if args.len() == 2
                    && let Some(value) = identity(fn_, xs[0], xs[1])
                {
                    return value;
                }
                self.apply(fn_, &xs)
            })
            .collect()
    }

    fn dot(&mut self, a: &[C], b: &[C]) -> C {
        self.each(Prim::Mul, &[a, b])
            .into_iter()
            .reduce(|s, c| identity(Prim::Add, s, c).unwrap_or_else(|| self.apply(Prim::Add, &[s, c])))
            .expect("numeric expressions have components")
    }

    fn call(&mut self, fn_: Prim, a: &[Vec<C>]) -> Vec<C> {
        use Prim::*;
        match fn_ {
            Normalize => {
                if a[0].len() == 1 {
                    return self.each(Sign, &[&a[0]]);
                }
                let sq = self.dot(&a[0], &a[0]);
                let l = self.apply(Sqrt, &[sq]);
                self.each(Div, &[&a[0], &[l]])
            }
            Length => {
                let sq = self.dot(&a[0], &a[0]);
                vec![self.apply(Sqrt, &[sq])]
            }
            Distance => {
                let d = self.each(Sub, &[&a[0], &a[1]]);
                let sq = self.dot(&d, &d);
                vec![self.apply(Sqrt, &[sq])]
            }
            Dot => vec![self.dot(&a[0], &a[1])],
            Cross => {
                let (x, y) = (&a[0], &a[1]);
                let mut term = |i, j| {
                    let ij = self.each(Mul, &[&[x[i]], &[y[j]]]);
                    let ji = self.each(Mul, &[&[x[j]], &[y[i]]]);
                    self.each(Sub, &[&ij, &ji])[0]
                };
                vec![term(1, 2), term(2, 0), term(0, 1)]
            }
            Select => {
                let c = a[0][0];
                (0..a[1].len().max(a[2].len()))
                    .map(|i| {
                        let (y, n) = (at(&a[1], i), at(&a[2], i));
                        match c {
                            C::Known(Scalar::Bool(b)) => {
                                if b {
                                    y
                                } else {
                                    n
                                }
                            }
                            C::Known(Scalar::Number(_) | Scalar::Undefined) | C::Local(_) => {
                                self.apply(Select, &[c, y, n])
                            }
                        }
                    })
                    .collect()
            }
            Add | Sub | Mul | Div | Neg | Min | Max | Pow | Mod | Atan2 | Step | Abs | Sign | Floor | Fract | Sqrt
            | Exp | Log | Sin | Cos | Tan | Saturate | Mix | Clamp | Smoothstep | Lt | Gt | Le | Ge | Eq | Ne | And
            | Or | Not | Ceil | Trunc | RoundEven | Rem | InverseSqrt | Exp2 | Log2 | Asin | Acos | Atan | Sinh
            | Cosh | Tanh => self.each(fn_, &a.iter().map(Vec::as_slice).collect::<Vec<_>>()),
        }
    }
}

/// Whether a library function has a portable CPU implementation.
pub fn has_cpu_twin(name: &str) -> bool {
    Twin::by_name(name).is_some()
}

/// A library reading rounded as f32 where expr.ts's foldNode folds a LibFn.
/// compileJs itself leaves a twin's outputs in f64.
pub fn fold_twin(name: &str, args: &[[f64; 4]]) -> Option<[f64; 4]> {
    Twin::by_name(name).map(|twin| twin.run(args).map(|x| f64::from(x as f32)))
}
