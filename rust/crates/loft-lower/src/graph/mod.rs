//! The compiler's expression arena, from src/expr.ts.

mod emit;
mod fold;
pub mod lit;
mod rewrite;
#[cfg(test)]
mod tests;
mod walk;
mod wire;

pub use emit::{Bind, Emission, wgsl_ty};
pub use loft_ir::graph::{Prim, Ty};
pub use walk::Leaves;
pub use wire::{Imported, KernelRef};

use crate::{Error, input::Input};
use std::cell::RefCell;
use std::collections::HashMap;
use std::ops::Index;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Expr(pub usize);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct LibId(pub usize);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct RowId(pub usize);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TableId(pub usize);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct LoopId(pub usize);

#[derive(Clone, Debug)]
pub struct InputSpec {
    pub name: String,
    pub ty: Ty,
    pub def: Box<[f64]>,
}

#[derive(Clone, Debug)]
pub struct ParamSpec {
    pub name: String,
    pub def: f64,
    pub min: f64,
    pub max: f64,
}

#[derive(Clone, Debug)]
pub struct Row {
    pub label: String,
    pub samples: Box<[f32]>,
}

#[derive(Clone, Debug)]
pub struct Table {
    pub name: String,
    pub lanes: u32,
    pub entries: u32,
    pub role: loft_ir::graph::Role,
}

/// A CPU twin takes one component slice per argument and returns the result's components.
/// Booleans have one component, 0 or 1.
pub type LibJs = fn(&[Vec<f64>]) -> Result<Vec<f64>, Error>;

#[derive(Clone, Debug)]
pub struct LibFn {
    pub name: String,
    pub ret: Ty,
    pub wgsl: String,
    pub deps: Box<[LibId]>,
    pub reads: Box<[loft_ir::graph::Read]>,
    pub js: Option<LibJs>,
    /// Raw compute accessors are interned by identity even if their text names coincide.
    pub raw_compute: Option<String>,
}

#[derive(Clone, Debug)]
pub struct Loop {
    pub count: Expr,
    pub init: Vec<Expr>,
    pub state: Vec<Expr>,
    pub index: Expr,
    pub next: Vec<Expr>,
    pub inner: Vec<Expr>,
    pub outer: Vec<Expr>,
    pub results: Vec<Expr>,
    pub stop: Option<Expr>,
    children: Vec<Expr>,
}

#[derive(Clone, Debug)]
pub enum Op {
    Const(Box<[f64]>),
    Var(String),
    Input(InputSpec),
    Param(ParamSpec),
    Row { row: RowId, x: Expr },
    Call { prim: Prim, args: Box<[Expr]> },
    Vec(Box<[Expr]>),
    Get { a: Expr, sel: String },
    Lib { fun: LibId, args: Box<[Expr]> },
    Fetch { table: TableId, index: Expr, lane: u32 },
    Iterate { lo: LoopId, k: usize },
}

#[derive(Clone, Debug)]
pub struct Node {
    pub ty: Ty,
    pub op: Op,
}

/// Plain arrays retain their provenance until the constructor has checked identities.
#[derive(Clone, Copy, Debug)]
pub enum In<'a> {
    Expr(Expr),
    Number(f64),
    Bool(bool),
    Array(&'a [f64]),
}
impl From<Expr> for In<'_> {
    fn from(e: Expr) -> Self {
        Self::Expr(e)
    }
}
impl From<f64> for In<'_> {
    fn from(x: f64) -> Self {
        Self::Number(x)
    }
}
impl From<bool> for In<'_> {
    fn from(x: bool) -> Self {
        Self::Bool(x)
    }
}
impl<'a> From<&'a [f64]> for In<'a> {
    fn from(x: &'a [f64]) -> Self {
        Self::Array(x)
    }
}
impl<'a, const N: usize> From<&'a [f64; N]> for In<'a> {
    fn from(x: &'a [f64; N]) -> Self {
        Self::Array(x)
    }
}

#[derive(PartialEq, Eq, Hash)]
enum Key {
    Const(Ty, Box<[u64]>),
    Var(Ty, String),
    Input(String, Ty, Box<[u64]>),
    Param(String, u64),
    Row(RowId, Expr),
    Fetch(TableId, u32, Expr),
    Iterate(LoopId, usize),
}

#[derive(Default)]
struct Trie {
    expr: Option<Expr>,
    next: HashMap<Expr, Trie>,
}

/// Child ids to node: one table per primitive, library function and vector type.
/// One or two children fit in a single numeric key, longer lists use a trie.
#[derive(Default)]
struct CallTable {
    flat: HashMap<i64, Expr>,
    deep: Trie,
}

fn interned_at(table: &mut CallTable, args: &[Expr], nodes: &mut Vec<Node>, make: impl FnOnce() -> Node) -> Expr {
    const PAIR: usize = 1 << 26;
    let key = match args {
        [a] => Some(-1 - a.0 as i64),
        [a, b] if a.0 < PAIR && b.0 < PAIR => Some((a.0 * PAIR + b.0) as i64),
        _ => None,
    };
    if let Some(key) = key {
        return *table.flat.entry(key).or_insert_with(|| fresh(nodes, make()));
    }
    let mut leaf = &mut table.deep;
    for &arg in args {
        leaf = leaf.next.entry(arg).or_default();
    }
    *leaf.expr.get_or_insert_with(|| fresh(nodes, make()))
}

fn fresh(nodes: &mut Vec<Node>, node: Node) -> Expr {
    let id = Expr(nodes.len());
    nodes.push(node);
    id
}

fn bits(x: f64) -> u64 {
    // Maps compare numbers with SameValueZero: keep the first value's sign in the node, but share its key with +0.
    if x == 0.0 {
        0
    } else if x.is_nan() {
        f64::NAN.to_bits()
    } else {
        x.to_bits()
    }
}

#[derive(Default)]
pub struct Arena {
    nodes: Vec<Node>,
    interned: HashMap<Key, Expr>,
    scalars: HashMap<(Ty, u64), Expr>,
    calls: HashMap<Prim, CallTable>,
    vecs: HashMap<Ty, CallTable>,
    lib_calls: HashMap<LibId, CallTable>,
    gets: HashMap<String, HashMap<Expr, Expr>>,
    lib_names: HashMap<String, LibId>,
    libs: Vec<LibFn>,
    rows: Vec<Row>,
    tables: Vec<Table>,
    loops: Vec<Loop>,
    next_loop: usize,
    stamps: RefCell<walk::Stamps>,
    folded: fold::Folded,
    identities: usize,
}

impl Index<Expr> for Arena {
    type Output = Node;
    fn index(&self, e: Expr) -> &Node {
        &self.nodes[e.0]
    }
}

fn scene(at: &str, given: &str, expected: &str, fix: &str) -> Error {
    Error::Scene(format!("{at}: given {given}; expected {expected}. Fix: {fix}"))
}

fn width_type(width: usize) -> Option<Ty> {
    [Ty::F32, Ty::V2, Ty::V3, Ty::V4].get(width.checked_sub(1)?).copied()
}

fn wider(prim: Prim, tys: impl IntoIterator<Item = Ty>) -> Result<Ty, Error> {
    let mut out = Ty::F32;
    for t in tys {
        let Some(n) = t.width() else {
            return Err(scene(
                prim.name(),
                &format!("a {}", t.name()),
                "numbers",
                "choose between numbers with select(cond, a, b)",
            ));
        };
        if t == Ty::F32 {
            continue;
        }
        if out != Ty::F32 && out != t {
            let m = out.lits();
            let (big, small) = if m > n { (out, n) } else { (t, m) };
            let sel = &"xyzw"[..small as usize];
            let zeroes = vec!["0"; (m.abs_diff(n)) as usize].join(", ");
            let big = big.name();
            return Err(scene(
                prim.name(),
                &format!("a {} and a {}", out.name(), t.name()),
                "operands of one size (or a plain number, which goes with any)",
                &format!("make both {big}s (e.g. {big}(v.{sel}, {zeroes})) or take .{sel} of the {big}"),
            ));
        }
        out = t;
    }
    Ok(out)
}

impl Arena {
    fn make(&mut self, key: Key, ty: Ty, op: impl FnOnce() -> Op) -> Expr {
        *self.interned.entry(key).or_insert_with(|| fresh(&mut self.nodes, Node { ty, op: op() }))
    }

    pub fn constant(&mut self, ty: Ty, v: &[f64]) -> Result<Expr, Error> {
        for &n in v {
            if !n.is_finite() {
                return Err(scene(
                    "a number in an expression",
                    &lit::to_string(n),
                    "a finite number",
                    "look for a division by zero, or a value read before it was set, on the line named at the end",
                ));
            }
        }
        let op = || Op::Const(v.into());
        Ok(if let [n] = v {
            *self.scalars.entry((ty, bits(*n))).or_insert_with(|| fresh(&mut self.nodes, Node { ty, op: op() }))
        } else {
            self.make(Key::Const(ty, v.iter().copied().map(bits).collect()), ty, op)
        })
    }

    /// Lift a plain value to an expression. Numbers become f32, arrays become vectors, booleans become bool.
    pub fn lift<'a>(&mut self, x: impl Into<In<'a>>) -> Result<Expr, Error> {
        match x.into() {
            In::Expr(e) => Ok(e),
            In::Number(x) => self.constant(Ty::F32, &[x]),
            In::Bool(x) => self.constant(Ty::Bool, &[f64::from(x)]),
            In::Array(x) if (2..=4).contains(&x.len()) => self.constant(width_type(x.len()).unwrap(), x),
            In::Array(x) => Err(Error::Internal(format!("cannot lift an array of length {}", x.len()))),
        }
    }

    pub fn variable(&mut self, name: &str, ty: Ty) -> Expr {
        self.make(Key::Var(ty, name.to_owned()), ty, || Op::Var(name.to_owned()))
    }

    /// An input's declaration belongs to its scene. Include the default in the key so a different scene can reuse
    /// the name with another default. Binding checks conflicting declarations within one scene.
    pub fn input(&mut self, name: &str, ty: Ty, def: &[f64]) -> Result<Expr, Error> {
        let n = ty.width().unwrap_or(0) as usize;
        if def.len() != n || n == 0 {
            let fix = (0..n).map(|k| lit::to_string(def.get(k).copied().unwrap_or(0.0))).collect::<Vec<_>>().join(", ");
            let plural = if def.len() == 1 { "" } else { "s" };
            return Err(scene(
                &format!("input \"{name}\""),
                &format!("a default of {} number{plural} for a {}", def.len(), ty.name()),
                &format!("{n} numbers"),
                &format!("input(\"{name}\", \"{}\", [{fix}])", ty.name()),
            ));
        }
        let key = Key::Input(name.to_owned(), ty, def.iter().copied().map(bits).collect());
        Ok(self.make(key, ty, || Op::Input(InputSpec { name: name.to_owned(), ty, def: def.into() })))
    }

    pub fn param(&mut self, name: &str, def: f64, min: Option<f64>, max: Option<f64>) -> Expr {
        let span = if def.is_nan() { def } else { def.abs().max(1e-3) };
        // The first declaration of a name and default supplies the range.
        self.make(Key::Param(name.to_owned(), bits(def)), Ty::F32, || {
            Op::Param(ParamSpec {
                name: name.to_owned(),
                def,
                min: min.unwrap_or(def - 2.0 * span),
                max: max.unwrap_or(def + 2.0 * span),
            })
        })
    }

    pub fn add_row(&mut self, row: Row) -> Result<RowId, Error> {
        if row.samples.len() != 64 * 4 {
            return Err(Error::Internal(format!(
                "Row \"{}\": expected 256 floats, got {}",
                row.label,
                row.samples.len()
            )));
        }
        let id = RowId(self.rows.len());
        self.rows.push(row);
        Ok(id)
    }

    pub fn row_at(&self, row: RowId) -> &Row {
        &self.rows[row.0]
    }

    pub fn row<'a>(&mut self, row: RowId, x: impl Into<In<'a>>) -> Result<Expr, Error> {
        let x = self.lift(x)?;
        Ok(self.make(Key::Row(row, x), Ty::V4, || Op::Row { row, x }))
    }

    pub fn add_table(&mut self, table: Table) -> TableId {
        let id = TableId(self.tables.len());
        self.tables.push(table);
        id
    }

    pub fn table(&self, table: TableId) -> &Table {
        &self.tables[table.0]
    }

    pub fn fetch<'a>(&mut self, table: TableId, index: impl Into<In<'a>>, lane: u32) -> Result<Expr, Error> {
        let t = self.table(table);
        if lane >= t.lanes {
            return Err(Error::Internal(format!(
                "fetch: table \"{}\" has {} lanes, asked for lane {lane}",
                t.name, t.lanes
            )));
        }
        let index = self.lift(index)?;
        Ok(self.make(Key::Fetch(table, lane, index), Ty::V4, || Op::Fetch { table, index, lane }))
    }

    pub fn add_lib(&mut self, fun: LibFn) -> LibId {
        let id = LibId(self.libs.len());
        self.libs.push(fun);
        id
    }

    pub fn lib_fn(&self, fun: LibId) -> &LibFn {
        &self.libs[fun.0]
    }

    pub fn lib<'a>(&mut self, fun: LibId, args: &[impl Copy + Into<In<'a>>]) -> Result<Expr, Error> {
        let f = self.lib_fn(fun);
        let ty = f.ret;
        if f.raw_compute.is_none() {
            if let Some(&other) = self.lib_names.get(&f.name) {
                if other != fun {
                    return Err(Error::Internal(format!("library function \"{}\" is defined twice", f.name)));
                }
            } else {
                self.lib_names.insert(f.name.clone(), fun);
            }
        }
        let args = self.lift_args(args)?;
        Ok(self.intern_lib(fun, ty, &args))
    }

    fn intern_lib(&mut self, fun: LibId, ty: Ty, args: &[Expr]) -> Expr {
        interned_at(self.lib_calls.entry(fun).or_default(), args, &mut self.nodes, || Node {
            ty,
            op: Op::Lib { fun, args: args.into() },
        })
    }

    fn lift_args<'a>(&mut self, args: &[impl Copy + Into<In<'a>>]) -> Result<Vec<Expr>, Error> {
        args.iter().map(|&a| self.lift(a)).collect()
    }

    pub fn vec_of<'a>(&mut self, ty: Ty, parts: &[impl Copy + Into<In<'a>>]) -> Result<Expr, Error> {
        let args = self.lift_args(parts)?;
        let total = args.iter().try_fold(0usize, |n, &e| Some(n + usize::from(self[e].ty.width()?)));
        let width = usize::from(ty.lits());
        if !matches!(ty, Ty::V2 | Ty::V3 | Ty::V4) || (total != Some(width) && total != Some(1)) {
            let components = total.map_or_else(|| "NaN".to_owned(), |n| n.to_string());
            let example = (0..width)
                .map(|k| if total.is_some_and(|n| k < n) { &"xyzw"[k..k + 1] } else { "0" })
                .collect::<Vec<_>>()
                .join(", ");
            return Err(scene(
                ty.name(),
                &format!("{components} components"),
                &format!("{width} (or one number for all of them)"),
                &format!("{}({example}): give exactly {width}", ty.name()),
            ));
        }
        Ok(self.intern_vec(ty, &args))
    }

    fn intern_vec(&mut self, ty: Ty, args: &[Expr]) -> Expr {
        interned_at(self.vecs.entry(ty).or_default(), args, &mut self.nodes, || Node { ty, op: Op::Vec(args.into()) })
    }

    pub fn get(&mut self, a: Expr, sel: &str) -> Result<Expr, Error> {
        let ty = self[a].ty;
        let Some(size @ 2..=4) = ty.width() else {
            return Err(Error::Internal(format!("cannot take .{sel} of {}", ty.name())));
        };
        let Some(out) = width_type(sel.len()) else {
            return Err(Error::Internal(format!(".{sel} is out of range for {}", ty.name())));
        };
        if !sel.bytes().all(|ch| b"xyzw"[..size as usize].contains(&ch)) {
            return Err(Error::Internal(format!(".{sel} is out of range for {}", ty.name())));
        }
        Ok(self.intern_get(out, a, sel))
    }

    fn intern_get(&mut self, ty: Ty, a: Expr, sel: &str) -> Expr {
        let by_sel = self.gets.entry(sel.to_owned()).or_default();
        *by_sel.entry(a).or_insert_with(|| fresh(&mut self.nodes, Node { ty, op: Op::Get { a, sel: sel.to_owned() } }))
    }

    fn call(&mut self, prim: Prim, ty: Ty, args: &[Expr]) -> Expr {
        interned_at(self.calls.entry(prim).or_default(), args, &mut self.nodes, || Node {
            ty,
            op: Op::Call { prim, args: args.into() },
        })
    }

    fn ty_of(&self, x: In<'_>) -> Result<Ty, Error> {
        match x {
            In::Expr(e) => Ok(self[e].ty),
            In::Number(_) => Ok(Ty::F32),
            In::Bool(_) => Ok(Ty::Bool),
            In::Array(a) => width_type(a.len())
                .filter(|_| a.len() >= 2)
                .ok_or_else(|| Error::Internal(format!("cannot lift an array of length {}", a.len()))),
        }
    }

    fn is_const(&self, x: In<'_>, k: f64) -> bool {
        match x {
            In::Expr(e) => matches!(&self[e].op, Op::Const(v) if v.iter().all(|&v| v == k)),
            In::Array(v) => v.iter().all(|&v| v == k),
            In::Number(v) => v == k,
            In::Bool(_) => false,
        }
    }

    fn is_zero(&self, x: In<'_>) -> bool {
        self.identities_on() && self.is_const(x, 0.0)
    }

    pub fn identities_on(&self) -> bool {
        self.identities > 0
    }

    /// Exact identities for finite operands are enabled for style bodies. Applied library-wide, removing these
    /// nodes changed vertex rounding (crest.ts, map/surface.ts, grid_detail.ts).
    pub fn with_identities<T>(&mut self, f: impl FnOnce(&mut Self) -> T) -> T {
        self.identities += 1;
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| f(self)));
        self.identities -= 1;
        result.unwrap_or_else(|e| std::panic::resume_unwind(e))
    }

    /// Add, mul, min and max with more than two operands construct the same graph as left-nested calls.
    pub fn prim<'a>(&mut self, prim: Prim, args: &[impl Copy + Into<In<'a>>]) -> Result<Expr, Error> {
        let assoc = matches!(prim, Prim::Add | Prim::Mul | Prim::Min | Prim::Max);
        let count = args.len();
        let arity = prim.arity();
        if (assoc && count < 2) || (!assoc && count != arity) {
            let plural = if count == 1 { "" } else { "s" };
            let name = prim.name();
            let (expected, fix) = if assoc {
                ("two or more".into(), format!("{name}(a, b)"))
            } else {
                (
                    format!("exactly {arity}"),
                    format!("nest the calls, e.g. {name}({name}(a, b), c), saying which pair goes first"),
                )
            };
            return Err(scene(name, &format!("{count} operand{plural}"), &expected, &fix));
        }
        if assoc || matches!(prim, Prim::Sub | Prim::Div | Prim::Pow | Prim::Mod | Prim::Atan2 | Prim::Step | Prim::Rem)
        {
            // The call form validates every operand before lifting any. A bad later argument must not leave
            // constants or intermediate nodes from an earlier pair in the arena.
            for (i, x) in args.iter().copied().map(Into::into).enumerate() {
                let given = match x {
                    In::Array(v) if !(2..=4).contains(&v.len()) => format!("an array of {} numbers", v.len()),
                    In::Bool(_) => "boolean".into(),
                    In::Expr(_) | In::Number(_) | In::Array(_) => continue,
                };
                return Err(scene(
                    &format!("{}, operand {}", prim.name(), i + 1),
                    &given,
                    "an expression, a number, or an array of 2 to 4 numbers",
                    "write each operand out; for a list, xs.reduce((a, b) => add(a, b)) (reduce passes the index and the array as well)",
                ));
            }
        }
        if assoc {
            let mut a = args[0].into();
            for &b in &args[1..] {
                a = self.binary(prim, a, b.into())?.into();
            }
            let In::Expr(e) = a else { unreachable!("at least two operands") };
            return Ok(e);
        }
        use Prim::*;
        let (ty, lifted) = match prim {
            Sub | Div | Pow | Mod | Atan2 | Step | Rem => return self.binary(prim, args[0].into(), args[1].into()),
            Neg | Abs | Sign | Floor | Fract | Sqrt | Exp | Log | Sin | Cos | Tan | Saturate | Normalize | Ceil
            | Trunc | RoundEven | InverseSqrt | Exp2 | Log2 | Asin | Acos | Atan | Sinh | Cosh | Tanh | Clamp => {
                let x = self.lift(args[0])?;
                (self[x].ty, self.lift_args(args)?)
            }
            Mix => {
                let (a, b, t) = (args[0].into(), args[1].into(), args[2].into());
                // The blend factor can be the only vector. Under identities, either endpoint can discard the other.
                let ty = wider(prim, [self.ty_of(a)?, self.ty_of(b)?, self.ty_of(t)?])?;
                if self.is_zero(t) && self.ty_of(a)? == ty {
                    return self.lift(a);
                }
                if self.identities_on() && self.is_const(t, 1.0) && self.ty_of(b)? == ty {
                    return self.lift(b);
                }
                (ty, self.lift_args(args)?)
            }
            Smoothstep => {
                let lifted = self.lift_args(args)?;
                (wider(prim, lifted.iter().map(|&e| self[e].ty))?, lifted)
            }
            Select => {
                let yes = self.lift(args[1])?;
                let no = self.lift(args[2])?;
                let ty = wider(prim, [self[yes].ty, self[no].ty])?;
                (ty, vec![self.lift(args[0])?, yes, no])
            }
            Length | Distance | Dot => (Ty::F32, self.lift_args(args)?),
            Cross => (Ty::V3, self.lift_args(args)?),
            Lt | Gt | Le | Ge | Eq | Ne | And | Or | Not => (Ty::Bool, self.lift_args(args)?),
            Add | Mul | Min | Max => unreachable!("associative operators dispatched above"),
        };
        Ok(self.call(prim, ty, &lifted))
    }

    fn binary(&mut self, prim: Prim, a: In<'_>, b: In<'_>) -> Result<Expr, Error> {
        let ty = |s: &Self| wider(prim, [s.ty_of(a)?, s.ty_of(b)?]);
        if prim == Prim::Add {
            if self.is_zero(b) && self.ty_of(a)? == ty(self)? {
                return self.lift(a);
            }
            if self.is_zero(a) && self.ty_of(b)? == ty(self)? {
                return self.lift(b);
            }
        }
        if prim == Prim::Mul {
            // This removes the cost of a channel a paint does not give. The result is zero of the product's type.
            if self.is_zero(a) || self.is_zero(b) {
                let ty = ty(self)?;
                return self.constant(ty, &[0.0; 4][..ty.lits() as usize]);
            }
            // Multiplying by a plain array of ones creates no node if the other operand has the product's type.
            // A number or a constant expression of ones is deliberately excluded to preserve existing shader text.
            let is_one = |x| matches!(x, In::Array(v) if v.iter().all(|&x| x == 1.0));
            if is_one(b) && self.ty_of(a)? == ty(self)? {
                return self.lift(a);
            }
            if is_one(a) && self.ty_of(b)? == ty(self)? {
                return self.lift(b);
            }
        }
        let a = self.lift(a)?;
        let b = self.lift(b)?;
        let ty = wider(prim, [self[a].ty, self[b].ty])?;
        Ok(self.call(prim, ty, &[a, b]))
    }

    pub fn loop_at(&self, lo: LoopId) -> &Loop {
        &self.loops[lo.0]
    }

    /// Repeat a step recorded over fresh state and index variables. The optional stop reads the next state.
    pub fn iterate<'a>(
        &mut self,
        count: impl Into<In<'a>>,
        init: &[Expr],
        body: impl FnOnce(&mut Self, &[Expr], Expr) -> Result<(Vec<Expr>, Option<Expr>), Error>,
    ) -> Result<Vec<Expr>, Error> {
        let (state, index) = self.loop_binders(init)?;
        let (next, stop) = body(self, &state, index)?;
        self.finish_loop(count.into(), init, state, index, next, stop)
    }

    fn loop_binders(&mut self, init: &[Expr]) -> Result<(Vec<Expr>, Expr), Error> {
        let id = self.next_loop;
        self.next_loop += 1;
        let mut state = Vec::with_capacity(init.len());
        for (j, &e) in init.iter().enumerate() {
            let ty = self[e].ty;
            if ty == Ty::Bool {
                return Err(Error::Internal("iterate: state components must be numbers".into()));
            }
            state.push(self.variable(&format!("_L{id}s{j}"), ty));
        }
        Ok((state, self.variable(&format!("_L{id}i"), Ty::F32)))
    }

    fn finish_loop(
        &mut self,
        count: In<'_>,
        init: &[Expr],
        state: Vec<Expr>,
        index: Expr,
        next: Vec<Expr>,
        stop: Option<Expr>,
    ) -> Result<Vec<Expr>, Error> {
        if next.len() != init.len() {
            return Err(Error::Internal(format!(
                "iterate: step returned {} components, the state has {}",
                next.len(),
                init.len()
            )));
        }
        for (j, (&next, &init)) in next.iter().zip(init).enumerate() {
            if self[next].ty != self[init].ty {
                return Err(Error::Internal(format!(
                    "iterate: state component {j} is {} but the step returned {}",
                    self[init].ty.name(),
                    self[next].ty.name()
                )));
            }
        }
        if let Some(e) = stop
            && self[e].ty != Ty::Bool
        {
            return Err(Error::Internal(format!("iterate: until must return a bool, got {}", self[e].ty.name())));
        }
        let body: Vec<_> = next.iter().copied().chain(stop).collect();
        let own: Vec<_> = [index].into_iter().chain(state.iter().copied()).collect();
        // Inner nodes depend on the loop's variables. Outer nodes are read by the body but computed before it.
        let (inner, outer) = self.split_on(&body, &own);
        let count = self.lift(count)?;
        let children = [count].into_iter().chain(init.iter().copied()).chain(outer.iter().copied()).collect();
        let lo = LoopId(self.loops.len());
        let results: Vec<_> = init
            .iter()
            .enumerate()
            .map(|(k, &e)| self.make(Key::Iterate(lo, k), self[e].ty, || Op::Iterate { lo, k }))
            .collect();
        self.loops.push(Loop {
            count,
            init: init.to_vec(),
            state,
            index,
            next,
            inner,
            outer,
            results: results.clone(),
            stop,
            children,
        });
        Ok(results)
    }

    /// The nodes a node reads, including a loop's count, starts and what its body reads from outside.
    pub fn children(&self, e: Expr) -> &[Expr] {
        match &self[e].op {
            Op::Const(_) | Op::Var(_) | Op::Input(_) | Op::Param(_) => &[],
            Op::Row { x, .. } | Op::Get { a: x, .. } | Op::Fetch { index: x, .. } => std::slice::from_ref(x),
            Op::Call { args, .. } | Op::Lib { args, .. } | Op::Vec(args) => args,
            Op::Iterate { lo, .. } => &self.loop_at(*lo).children,
        }
    }
}

/// The program roots emitted in one call with the probe binding, then out and libs lines.
pub fn probe(input: &Input) -> Result<String, Error> {
    let Imported { arena, roots, .. } = Arena::from_wire(input.graph())?;
    let mut e = arena.emit_wgsl(&roots, &ProbeBind, "e", &HashMap::new(), None)?;
    e.lines.push(format!("out {}", e.out.join(" ")));
    e.lines.push(format!(
        "libs {}",
        e.libs.iter().map(|&id| arena.lib_fn(id).name.as_str()).collect::<Vec<_>>().join(" ")
    ));
    e.lines.push(String::new());
    Ok(e.lines.join("\n"))
}

struct ProbeBind;

impl Bind for ProbeBind {
    fn variable(&self, name: &str, _ty: Ty) -> String {
        name.to_owned()
    }
    fn input(&self, spec: &InputSpec) -> String {
        format!("input_{}", spec.name)
    }
    fn param(&self, spec: &ParamSpec) -> String {
        format!("param_{}", spec.name)
    }
    fn row(&self, _id: RowId, row: &Row, x: &str) -> String {
        format!("row_{}({x})", row.label)
    }
    fn table(&self, _id: TableId, table: &Table, index: &str, lane: u32) -> String {
        format!("fetch_{}_{lane}({index})", table.name)
    }
}
