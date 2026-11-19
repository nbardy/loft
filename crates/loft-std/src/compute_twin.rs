//! Evaluate the validated deterministic compute subset in Naga's evaluation order.
#[derive(Debug, PartialEq, Eq)]
pub enum Error { Unsupported, Capacity, MissingEntry }
type Result<T> = std::result::Result<T, Error>;
use naga::{
    Arena, BinaryOperator as B, Expression as E, Handle, Literal, MathFunction as M,
    ScalarKind as K, Statement as S, Type, TypeInner as T,
};
use std::collections::BTreeMap;

const UNSUPPORTED: Error = Error::Unsupported;
type Expr = Handle<E>;
type Buffers = BTreeMap<(u32, u32), Vec<u8>>;

#[derive(Clone)]
enum Value {
    F(f32),
    U(u32),
    I(i32),
    B(bool),
    List(Vec<Value>),
    Ptr {
        global: bool,
        root: usize,
        path: Vec<usize>,
    },
}
use Value::*;

impl Value {
    fn word(&self) -> Result<u32> {
        Ok(match *self {
            F(x) => x.to_bits(),
            U(x) => x,
            I(x) => x as u32,
            B(x) => x.into(),
            _ => return Err(UNSUPPORTED),
        })
    }
    fn index(&self) -> Result<usize> {
        Ok(match *self {
            U(x) => x as usize,
            I(x) if x >= 0 => x as usize,
            _ => return Err(Error::Capacity),
        })
    }
    fn cond(&self) -> Result<bool> {
        match *self {
            B(b) => Ok(b),
            _ => Err(UNSUPPORTED),
        }
    }
    fn at(mut self, i: usize) -> Result<Self> {
        match &mut self {
            Ptr { path, .. } => {
                path.push(i);
                Ok(self)
            }
            List(values) => values.get(i).cloned().ok_or(Error::Capacity),
            _ => Err(UNSUPPORTED),
        }
    }
}

fn scalar(kind: K, word: u32) -> Result<Value> {
    Ok(match kind {
        K::Float => F(f32::from_bits(word)),
        K::Uint => U(word),
        K::Sint => I(word as i32),
        K::Bool => B(word != 0),
        _ => return Err(UNSUPPORTED),
    })
}

fn cast(v: Value, kind: K, convert: bool) -> Result<Value> {
    if let List(v) = v {
        return v
            .into_iter()
            .map(|v| cast(v, kind, convert))
            .collect::<Result<_>>()
            .map(List);
    }
    if !convert {
        return scalar(kind, v.word()?);
    }
    Ok(match (kind, v) {
        (K::Float, U(x)) => F(x as f32),
        (K::Float, I(x)) => F(x as f32),
        (K::Uint, F(x)) => U(x as u32),
        (K::Sint, F(x)) => I(x as i32),
        (K::Bool, v) => B(v.word()? != 0),
        (k, v) => scalar(k, v.word()?)?,
    })
}

fn read(word: &[u8], offset: usize) -> Result<u32> {
    Ok(u32::from_le_bytes(
        word.get(offset..offset + 4)
            .ok_or(Error::Capacity)?
            .try_into()
            .unwrap(),
    ))
}

fn unpack(
    types: &naga::UniqueArena<Type>,
    ty: Handle<Type>,
    bytes: Option<&[u8]>,
) -> Result<Value> {
    let word = |offset| bytes.map_or(Ok(0), |b| read(b, offset));
    let sub = |offset: usize| {
        bytes
            .map(|b| b.get(offset..).ok_or(Error::Capacity))
            .transpose()
    };
    Ok(match &types[ty].inner {
        T::Scalar(s) | T::Atomic(s) if s.width == 4 || s.kind == K::Bool => {
            scalar(s.kind, word(0)?)?
        }
        T::Vector { size, scalar: s } if s.width == 4 || s.kind == K::Bool => List(
            (0..*size as usize)
                .map(|i| scalar(s.kind, word(4 * i)?))
                .collect::<Result<_>>()?,
        ),
        T::Matrix {
            columns,
            rows,
            scalar: s,
        } if s.width == 4 => {
            let stride = if *rows == naga::VectorSize::Bi { 8 } else { 16 };
            List(
                (0..*columns as usize)
                    .map(|i| {
                        (0..*rows as usize)
                            .map(|j| scalar(s.kind, word(i * stride + j * 4)?))
                            .collect::<Result<_>>()
                            .map(List)
                    })
                    .collect::<Result<_>>()?,
            )
        }
        T::Struct { members, .. } => List(
            members
                .iter()
                .map(|m| unpack(types, m.ty, sub(m.offset as usize)?))
                .collect::<Result<_>>()?,
        ),
        T::Array { base, size, stride } => {
            let stride = *stride as usize;
            let n = match size {
                naga::ArraySize::Constant(n) => n.get() as usize,
                naga::ArraySize::Dynamic => bytes.ok_or(UNSUPPORTED)?.len() / stride,
                _ => return Err(UNSUPPORTED),
            };
            List(
                (0..n)
                    .map(|i| unpack(types, *base, sub(i * stride)?))
                    .collect::<Result<_>>()?,
            )
        }
        _ => return Err(UNSUPPORTED),
    })
}

fn pack(
    types: &naga::UniqueArena<Type>,
    ty: Handle<Type>,
    v: &Value,
    bytes: &mut [u8],
) -> Result<()> {
    let mut store = |i: usize, v: &Value| -> Result<()> {
        bytes
            .get_mut(i..i + 4)
            .ok_or(Error::Capacity)?
            .copy_from_slice(&v.word()?.to_le_bytes());
        Ok(())
    };
    match (&types[ty].inner, v) {
        (T::Scalar(_) | T::Atomic(_), v) => store(0, v),
        (T::Vector { .. }, List(v)) => {
            for (i, v) in v.iter().enumerate() {
                store(i * 4, v)?;
            }
            Ok(())
        }
        (T::Matrix { rows, .. }, List(columns)) => {
            let stride = if *rows == naga::VectorSize::Bi { 8 } else { 16 };
            for (i, col) in columns.iter().enumerate() {
                let List(col) = col else {
                    return Err(UNSUPPORTED);
                };
                for (j, v) in col.iter().enumerate() {
                    store(i * stride + j * 4, v)?;
                }
            }
            Ok(())
        }
        (T::Struct { members, .. }, List(v)) => {
            for (m, v) in members.iter().zip(v) {
                pack(types, m.ty, v, &mut bytes[m.offset as usize..])?;
            }
            Ok(())
        }
        (T::Array { base, stride, .. }, List(v)) => {
            for (i, v) in v.iter().enumerate() {
                pack(types, *base, v, &mut bytes[i * *stride as usize..])?;
            }
            Ok(())
        }
        _ => Err(UNSUPPORTED),
    }
}

fn binary(op: B, a: Value, b: Value) -> Result<Value> {
    if op == B::Multiply
        && let (List(left), List(right)) = (&a, &b)
    {
        let matrix = |v: &[Value]| matches!(v.first(), Some(List(_)));
        match (matrix(left), matrix(right)) {
            (true, true) => {
                return right
                    .iter()
                    .cloned()
                    .map(|col| binary(op, a.clone(), col))
                    .collect::<Result<_>>()
                    .map(List);
            }
            (true, false) => {
                return left
                    .iter()
                    .cloned()
                    .zip(right.iter().cloned())
                    .map(|(col, w)| binary(op, col, w))
                    .reduce(|x, y| binary(B::Add, x?, y?))
                    .ok_or(UNSUPPORTED)?;
            }
            (false, true) => {
                return right
                    .iter()
                    .cloned()
                    .map(|col| math(M::Dot, vec![a.clone(), col]))
                    .collect::<Result<_>>()
                    .map(List);
            }
            (false, false) => {}
        }
    }
    if let (List(a), List(b)) = (&a, &b) {
        return a
            .iter()
            .cloned()
            .zip(b.iter().cloned())
            .map(|(x, y)| binary(op, x, y))
            .collect::<Result<_>>()
            .map(List);
    }
    if let List(a) = a {
        return a
            .into_iter()
            .map(|x| binary(op, x, b.clone()))
            .collect::<Result<_>>()
            .map(List);
    }
    if let List(b) = b {
        return b
            .into_iter()
            .map(|y| binary(op, a.clone(), y))
            .collect::<Result<_>>()
            .map(List);
    }
    macro_rules! compare {
        ($x:ident, $y:ident) => {
            match op {
                B::Equal => B($x == $y),
                B::NotEqual => B($x != $y),
                B::Less => B($x < $y),
                B::LessEqual => B($x <= $y),
                B::Greater => B($x > $y),
                B::GreaterEqual => B($x >= $y),
                _ => return Err(UNSUPPORTED),
            }
        };
    }
    macro_rules! integer {
        ($tag:ident, $x:ident, $y:ident) => {
            match op {
                B::Add => $tag($x.wrapping_add($y)),
                B::Subtract => $tag($x.wrapping_sub($y)),
                B::Multiply => $tag($x.wrapping_mul($y)),
                B::Divide => $tag($x.wrapping_div(if $y == 0 { 1 } else { $y })),
                B::Modulo => $tag($x.wrapping_rem(if $y == 0 { 1 } else { $y })),
                B::And => $tag($x & $y),
                B::InclusiveOr => $tag($x | $y),
                B::ExclusiveOr => $tag($x ^ $y),
                B::ShiftLeft => $tag($x.wrapping_shl($y as u32)),
                B::ShiftRight => $tag($x.wrapping_shr($y as u32)),
                _ => compare!($x, $y),
            }
        };
    }
    Ok(match (a, b) {
        (F(x), F(y)) => match op {
            B::Add => F(x + y),
            B::Subtract => F(x - y),
            B::Multiply => F(x * y),
            B::Divide => F(x / y),
            B::Modulo => F(x % y),
            _ => compare!(x, y),
        },
        (U(x), U(y)) => integer!(U, x, y),
        (I(x), I(y)) => integer!(I, x, y),
        (I(x), U(y)) if op == B::ShiftLeft => I(x.wrapping_shl(y)),
        (I(x), U(y)) if op == B::ShiftRight => I(x.wrapping_shr(y)),
        (B(x), B(y)) => match op {
            B::LogicalAnd | B::And => B(x & y),
            B::LogicalOr | B::InclusiveOr => B(x | y),
            B::ExclusiveOr => B(x ^ y),
            _ => compare!(x, y),
        },
        _ => return Err(UNSUPPORTED),
    })
}

fn unary(op: naga::UnaryOperator, a: Value) -> Result<Value> {
    use naga::UnaryOperator::*;
    Ok(match (op, a) {
        (op, List(a)) => List(a.into_iter().map(|a| unary(op, a)).collect::<Result<_>>()?),
        (Negate, F(x)) => F(-x),
        (Negate, I(x)) => I(x.wrapping_neg()),
        (LogicalNot, B(x)) => B(!x),
        (BitwiseNot, U(x)) => U(!x),
        (BitwiseNot, I(x)) => I(!x),
        _ => return Err(UNSUPPORTED),
    })
}

fn select(cond: Value, a: Value, b: Value) -> Result<Value> {
    if let (List(c), List(a), List(b)) = (&cond, &a, &b) {
        return c
            .iter()
            .cloned()
            .zip(a.iter().cloned())
            .zip(b.iter().cloned())
            .map(|((c, a), b)| select(c, a, b))
            .collect::<Result<_>>()
            .map(List);
    }
    Ok(if cond.cond()? { a } else { b })
}

fn math(fun: M, args: Vec<Value>) -> Result<Value> {
    let mut it = args.iter();
    match (fun, args.as_slice()) {
        (M::Dot, [List(a), List(b)]) => {
            return a
                .iter()
                .cloned()
                .zip(b.iter().cloned())
                .map(|(x, y)| binary(B::Multiply, x, y))
                .reduce(|x, y| binary(B::Add, x?, y?))
                .ok_or(UNSUPPORTED)?;
        }
        (M::Cross, [List(a), List(b)]) if a.len() == 3 && b.len() == 3 => {
            return (0..3)
                .map(|i| {
                    let j = (i + 1) % 3;
                    let k = (i + 2) % 3;
                    let p = binary(B::Multiply, a[j].clone(), b[k].clone())?;
                    let q = binary(B::Multiply, a[k].clone(), b[j].clone())?;
                    binary(B::Subtract, p, q)
                })
                .collect::<Result<_>>()
                .map(List);
        }
        (M::Length | M::Normalize, [List(a)]) => {
            let d = math(M::Dot, vec![List(a.clone()), List(a.clone())])?;
            let l = math(M::Sqrt, vec![d])?;
            return if fun == M::Length {
                Ok(l)
            } else {
                binary(B::Divide, List(a.clone()), l)
            };
        }
        (M::Distance, [a, b]) => {
            return math(M::Length, vec![binary(B::Subtract, a.clone(), b.clone())?]);
        }
        _ => {}
    }
    if let Some(n) = args
        .iter()
        .find_map(|a| if let List(v) = a { Some(v.len()) } else { None })
    {
        return (0..n)
            .map(|i| {
                math(
                    fun,
                    args.iter()
                        .map(|a| match a {
                            List(v) => v[i].clone(),
                            _ => a.clone(),
                        })
                        .collect(),
                )
            })
            .collect::<Result<_>>()
            .map(List);
    }
    let a = it.next();
    let b = it.next();
    let c = it.next();
    Ok(match (fun, a, b, c) {
        (M::Abs, Some(F(x)), _, _) => F(x.abs()),
        (M::Abs, Some(I(x)), _, _) => I(x.wrapping_abs()),
        (M::Sqrt, Some(F(x)), _, _) => F(x.sqrt()),
        (M::InverseSqrt, Some(F(x)), _, _) => F(1. / x.sqrt()),
        (M::Floor, Some(F(x)), _, _) => F(x.floor()),
        (M::Ceil, Some(F(x)), _, _) => F(x.ceil()),
        (M::Trunc, Some(F(x)), _, _) => F(x.trunc()),
        (M::Round, Some(F(x)), _, _) => F(x.round_ties_even()),
        (M::Fract, Some(F(x)), _, _) => F(x - x.floor()),
        (M::Sign, Some(F(x)), _, _) => F(if *x == 0. { 0. } else { x.signum() }),
        (M::Min, Some(F(x)), Some(F(y)), _) => F(x.min(*y)),
        (M::Max, Some(F(x)), Some(F(y)), _) => F(x.max(*y)),
        (M::Clamp, Some(F(x)), Some(F(y)), Some(F(z))) => F(x.max(*y).min(*z)),
        (M::Min, Some(U(x)), Some(U(y)), _) => U(*x.min(y)),
        (M::Max, Some(U(x)), Some(U(y)), _) => U(*x.max(y)),
        (M::Min, Some(I(x)), Some(I(y)), _) => I(*x.min(y)),
        (M::Max, Some(I(x)), Some(I(y)), _) => I(*x.max(y)),
        (M::Clamp, Some(I(x)), Some(I(y)), Some(I(z))) => I(*x.max(y).min(z)),
        (M::Clamp, Some(U(x)), Some(U(y)), Some(U(z))) => U(*x.max(y).min(z)),
        (M::Fma, Some(F(x)), Some(F(y)), Some(F(z))) => F(x.mul_add(*y, *z)),
        (M::Mix, Some(F(x)), Some(F(y)), Some(F(z))) => F(x * (1. - z) + y * z),
        (M::Step, Some(F(edge)), Some(F(x)), _) => F(u8::from(x >= edge) as f32),
        (M::SmoothStep, Some(F(lo)), Some(F(hi)), Some(F(x))) => {
            let t = ((x - lo) / (hi - lo)).clamp(0., 1.);
            F(t * t * (3. - 2. * t))
        }
        _ => return Err(UNSUPPORTED),
    })
}

struct Call<'a> {
    args: Vec<Value>,
    expr: &'a Arena<E>,
    values: Vec<Option<Value>>,
    locals: Vec<Value>,
}
enum Flow {
    Next,
    Break,
    Continue,
    Return(Option<Value>),
}
struct Eval<'a> {
    ir: &'a naga::Module,
    global: Vec<Option<Value>>,
    budget: u32,
}
impl Eval<'_> {
    fn reference<'a>(&'a self, c: &'a Call<'_>, p: &Value) -> Result<&'a Value> {
        let Ptr { global, root, path } = p else {
            return Err(UNSUPPORTED);
        };
        let mut v = if *global {
            self.global[*root].as_ref().ok_or(UNSUPPORTED)?
        } else {
            &c.locals[*root]
        };
        for i in path {
            let List(inner) = v else {
                return Err(UNSUPPORTED);
            };
            v = inner.get(*i).ok_or(Error::Capacity)?;
        }
        Ok(v)
    }

    fn store(&mut self, c: &mut Call<'_>, p: &Value, x: Value) -> Result<()> {
        let Ptr { global, root, path } = p else {
            return Err(UNSUPPORTED);
        };
        let mut v = if *global {
            self.global[*root].as_mut().ok_or(UNSUPPORTED)?
        } else {
            &mut c.locals[*root]
        };
        for i in path {
            let List(inner) = v else {
                return Err(UNSUPPORTED);
            };
            v = inner.get_mut(*i).ok_or(Error::Capacity)?;
        }
        *v = x;
        Ok(())
    }

    fn get(&self, c: &Call<'_>, h: Expr) -> Result<Value> {
        match &c.values[h.index()] {
            Some(v) => Ok(v.clone()),
            None => self.eval(Some(c), h),
        }
    }

    fn eval(&self, c: Option<&Call<'_>>, h: Expr) -> Result<Value> {
        let expr = c.map_or(&self.ir.global_expressions, |c| c.expr);
        let e = |h| c.map_or_else(|| self.eval(None, h), |c| self.get(c, h));
        Ok(match &expr[h] {
            E::Literal(Literal::F32(x)) => F(*x),
            E::Literal(Literal::U32(x)) => U(*x),
            E::Literal(Literal::I32(x)) => I(*x),
            E::Literal(Literal::Bool(x)) => B(*x),
            E::Constant(k) => self.eval(None, self.ir.constants[*k].init)?,
            E::ZeroValue(ty) => unpack(&self.ir.types, *ty, None)?,
            E::Compose { ty, components } => {
                let values = components
                    .iter()
                    .map(|h| e(*h))
                    .collect::<Result<Vec<_>>>()?;
                if matches!(self.ir.types[*ty].inner, T::Vector { .. }) {
                    List(
                        values
                            .into_iter()
                            .flat_map(|v| match v {
                                List(v) => v,
                                x => vec![x],
                            })
                            .collect(),
                    )
                } else {
                    List(values)
                }
            }
            E::Access { base, index } => e(*base)?.at(e(*index)?.index()?)?,
            E::AccessIndex { base, index } => e(*base)?.at(*index as usize)?,
            E::Splat { size, value } => List(vec![e(*value)?; *size as usize]),
            E::Swizzle {
                size,
                vector,
                pattern,
            } => {
                let List(v) = e(*vector)? else {
                    return Err(UNSUPPORTED);
                };
                List(
                    pattern[..*size as usize]
                        .iter()
                        .map(|i| v[*i as usize].clone())
                        .collect(),
                )
            }
            E::FunctionArgument(i) => c.ok_or(UNSUPPORTED)?.args[*i as usize].clone(),
            E::GlobalVariable(h) => Ptr {
                global: true,
                root: h.index(),
                path: vec![],
            },
            E::LocalVariable(h) => Ptr {
                global: false,
                root: h.index(),
                path: vec![],
            },
            E::Load { pointer } => self
                .reference(c.ok_or(UNSUPPORTED)?, &e(*pointer)?)?
                .clone(),
            E::Unary { op, expr } => unary(*op, e(*expr)?)?,
            E::Binary { op, left, right } => binary(*op, e(*left)?, e(*right)?)?,
            E::Select {
                condition,
                accept,
                reject,
            } => select(e(*condition)?, e(*accept)?, e(*reject)?)?,
            E::As {
                expr,
                kind,
                convert,
            } => cast(e(*expr)?, *kind, convert.is_some())?,
            E::Math {
                fun,
                arg,
                arg1,
                arg2,
                arg3,
            } => math(
                *fun,
                std::iter::once(*arg)
                    .chain([*arg1, *arg2, *arg3].into_iter().flatten())
                    .map(e)
                    .collect::<Result<_>>()?,
            )?,
            E::ArrayLength(p) => {
                let List(v) = self.reference(c.ok_or(UNSUPPORTED)?, &e(*p)?)? else {
                    return Err(UNSUPPORTED);
                };
                U(v.len() as u32)
            }
            _ => return Err(UNSUPPORTED),
        })
    }

    fn call(&mut self, f: &naga::Function, args: Vec<Value>) -> Result<Option<Value>> {
        let mut c = Call {
            args,
            expr: &f.expressions,
            values: vec![None; f.expressions.len()],
            locals: vec![],
        };
        for (_, local) in f.local_variables.iter() {
            c.locals.push(match local.init {
                Some(h) => self.get(&c, h)?,
                None => unpack(&self.ir.types, local.ty, None)?,
            });
        }
        match self.block(&mut c, &f.body)? {
            Flow::Return(v) => Ok(v),
            Flow::Next => Ok(None),
            _ => Err(UNSUPPORTED),
        }
    }

    fn block(&mut self, c: &mut Call<'_>, block: &naga::Block) -> Result<Flow> {
        for s in block {
            self.budget = self.budget.checked_sub(1).ok_or(Error::Capacity)?;
            let e = |h| self.get(c, h);
            let flow = match s {
                S::Emit(range) => {
                    // Re-evaluate each emitted value on loop re-entry and keep its value across later stores.
                    for h in range.clone() {
                        c.values[h.index()] = Some(self.eval(Some(c), h)?);
                    }
                    Flow::Next
                }
                S::Block(b) => self.block(c, b)?,
                S::If {
                    condition,
                    accept,
                    reject,
                } => {
                    let b = if e(*condition)?.cond()? {
                        accept
                    } else {
                        reject
                    };
                    self.block(c, b)?
                }
                S::Loop {
                    body,
                    continuing,
                    break_if,
                } => loop {
                    self.budget = self.budget.checked_sub(1).ok_or(Error::Capacity)?;
                    match self.block(c, body)? {
                        Flow::Break => break Flow::Next,
                        f @ Flow::Return(_) => break f,
                        Flow::Continue | Flow::Next => {}
                    }
                    match self.block(c, continuing)? {
                        Flow::Next => {}
                        f => break f,
                    }
                    if let Some(h) = break_if
                        && self.get(c, *h)?.cond()?
                    {
                        break Flow::Next;
                    }
                },
                S::Break => Flow::Break,
                S::Continue => Flow::Continue,
                S::Return { value } => Flow::Return(value.map(e).transpose()?),
                S::Store { pointer, value } => {
                    let (p, v) = (e(*pointer)?, e(*value)?);
                    self.store(c, &p, v)?;
                    Flow::Next
                }
                S::Call {
                    function,
                    arguments,
                    result,
                } => {
                    let args = arguments.iter().map(|h| e(*h)).collect::<Result<_>>()?;
                    let value = self.call(&self.ir.functions[*function], args)?;
                    if let Some(h) = result {
                        c.values[h.index()] = value;
                    }
                    Flow::Next
                }
                S::Atomic {
                    pointer,
                    fun: naga::AtomicFunction::Add,
                    value,
                    result,
                    ..
                } => {
                    let (p, v) = (e(*pointer)?, e(*value)?);
                    let old = self.reference(c, &p)?.clone();
                    self.store(c, &p, binary(B::Add, old.clone(), v)?)?;
                    if let Some(h) = result {
                        c.values[h.index()] = Some(old);
                    }
                    Flow::Next
                }
                _ => return Err(UNSUPPORTED),
            };
            if !matches!(flow, Flow::Next) {
                return Ok(flow);
            }
        }
        Ok(Flow::Next)
    }
}

pub fn dispatch(
    ir: &naga::Module,
    entry: &str,
    rows: u32,
    buffers: &mut Buffers,
) -> Result<()> {
    let f = &ir
        .entry_points
        .iter()
        .find(|e| e.name == entry && e.stage == naga::ShaderStage::Compute)
        .ok_or(Error::MissingEntry)?
        .function;
    if !matches!(f.arguments.as_slice(), [a] if a.binding == Some(naga::Binding::BuiltIn(naga::BuiltIn::GlobalInvocationId)))
    {
        return Err(UNSUPPORTED);
    }
    let mut global = vec![None; ir.global_variables.len()];
    for (h, v) in ir.global_variables.iter() {
        if let Some(binding) = &v.binding
            && let Some(bytes) = buffers.get(&(binding.group, binding.binding))
        {
            global[h.index()] = Some(unpack(&ir.types, v.ty, Some(bytes))?);
        }
    }
    let mut eval = Eval {
        ir,
        global,
        budget: 0,
    };
    for i in 0..rows {
        eval.budget = 1_000_000;
        for (h, v) in ir
            .global_variables
            .iter()
            .filter(|(_, v)| v.space == naga::AddressSpace::Private)
        {
            eval.global[h.index()] = Some(match v.init {
                Some(e) => eval.eval(None, e)?,
                None => unpack(&ir.types, v.ty, None)?,
            });
        }
        eval.call(f, vec![List(vec![U(i), U(0), U(0)])])?;
    }
    for (h, v) in ir.global_variables.iter() {
        if let naga::AddressSpace::Storage { access } = v.space
            && access.intersects(naga::StorageAccess::STORE | naga::StorageAccess::ATOMIC)
            && let (Some(binding), Some(value)) = (&v.binding, &eval.global[h.index()])
        {
            pack(
                &ir.types,
                v.ty,
                value,
                buffers.get_mut(&(binding.group, binding.binding)).unwrap(),
            )?;
        }
    }
    Ok(())
}
