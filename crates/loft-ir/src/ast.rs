//! The source tree of the WGSL superset (draft §1), and the checked types the passes share.
//! Every expression has an `id`; the checker's types live in a side table indexed by it.

use crate::Span;

#[derive(Clone, Debug)]
pub struct Ident {
    pub name: String,
    pub span: Span,
}

#[derive(Debug, Default)]
pub struct Module {
    pub items: Vec<Item>,
    /// `diff(f, x)` sites, as written: the AD pass generates `f__d_x` (the parser already called it).
    pub diffs: Vec<(Ident, Ident)>,
}

#[derive(Debug)]
pub enum Item {
    Space(Vec<Ident>),
    Unit(Ident),
    Const(ConstDecl),
    Fn(FnDecl),
    Kind(KindDecl),
    Scan(ScanDecl),
    Schedule(ScheduleDecl),
}

/// `scan name(t: f32 in lo..hi, steps: N) @schedule(bake_lut) -> Table[T] { init: e; step(prev, t_i): e; }`:
/// a sequential integration baked at load into an N-entry table, read with `sample_c2(name, t)`.
#[derive(Debug)]
pub struct ScanDecl {
    pub name: Ident,
    pub var: Ident,
    pub var_ty: TyExpr,
    pub lo: Expr,
    pub hi: Expr,
    pub steps: Expr,
    pub elem: TyExpr,
    pub init: Expr,
    pub prev: Ident,
    pub at: Ident,
    pub step: Expr,
}

/// `const X: T = e;` (stage 1, plain WGSL) or `comptime const X: T = e;` (stage 0, overridable by the CLI).
#[derive(Debug)]
pub struct ConstDecl {
    pub comptime: bool,
    pub name: Ident,
    pub ty: Option<TyExpr>,
    pub value: Expr,
}

/// `@name(args)` before a fn. Phase 0 knows one: `@lipschitz(param)` or `@lipschitz(max(a, b))`, a library axiom
/// the truth gate samples.
#[derive(Debug)]
pub struct Attr {
    pub name: Ident,
    pub args: Vec<Expr>,
}

#[derive(Debug)]
pub struct FnDecl {
    pub comptime: bool,
    pub attrs: Vec<Attr>,
    pub name: Ident,
    pub params: Vec<Param>,
    pub ret: Option<TyExpr>,
    pub body: Block,
}

#[derive(Debug)]
pub struct Param {
    pub name: Ident,
    /// `None` only for a query's `self`.
    pub ty: Option<TyExpr>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QueryKind {
    /// Closed-form ray hit: `(self, ro: Pt[S], rd: Dir[S]) -> f32`, negative for a miss.
    Visible,
    /// Distance field: `(self, p: Pt[S]) -> Field[T]`, sphere traced with the derived L.
    Trace,
    /// Surface response: `(self, p: Pt[S], n: Norm[S]) -> Bsdf`, plus `uv: vec2<f32>` on a surface kind.
    Material,
    /// Parametric surface over the unit square, rasterised from `@grid(nu, nv)`: `(self, u: f32, v: f32) -> Pt[S]`.
    Surface,
}

#[derive(Debug)]
pub struct KindDecl {
    pub name: Ident,
    pub params: Vec<KindParam>,
    pub row: Vec<RowField>,
    pub queries: Vec<(QueryKind, FnDecl)>,
    pub fns: Vec<FnDecl>,
}

impl KindDecl {
    pub fn query(&self, q: QueryKind) -> Option<&FnDecl> {
        self.queries.iter().find(|(w, _)| *w == q).map(|(_, f)| f)
    }
}

/// `comptime name: T` or `comptime name: u32 in lo..hi`.
#[derive(Debug)]
pub struct KindParam {
    pub name: Ident,
    pub ty: TyExpr,
    pub range: Option<(Expr, Expr)>,
    pub span: Span,
}

#[derive(Debug)]
pub struct RowField {
    pub name: Ident,
    pub ty: TyExpr,
}

#[derive(Debug)]
pub struct ScheduleDecl {
    pub name: Ident,
    pub passes: Vec<PassDecl>,
}

/// `pass label: verb(args) { body calls };`
#[derive(Debug)]
pub struct PassDecl {
    pub label: Ident,
    pub verb: Ident,
    pub args: Vec<Arg>,
    pub body: Vec<(Ident, Vec<Arg>)>,
}

#[derive(Debug)]
pub struct Block {
    pub stmts: Vec<Stmt>,
}

#[derive(Debug)]
pub enum Stmt {
    Let { mutable: bool, name: Ident, ty: Option<TyExpr>, value: Expr },
    Assign { target: Expr, op: Option<BinOp>, value: Expr },
    /// `map name = head |> stage(args) |> ...;` introduces the fresh space `'name`.
    Map { name: Ident, head: Expr, stages: Vec<(Ident, Vec<Expr>)> },
    Return(Option<Expr>, Span),
    If { cond: Expr, then: Block, otherwise: Option<Block> },
    For { comptime: bool, var: Ident, lo: Expr, hi: Expr, body: Block, span: Span },
    Emit { kind: Ident, fields: Vec<(Ident, Expr)>, span: Span },
    Expr(Expr),
}

#[derive(Debug)]
pub struct Expr {
    pub id: u32,
    pub span: Span,
    pub kind: ExprKind,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NumSuffix {
    None,
    F,
    U,
    I,
    /// Meters: `m`.
    M,
    Rad,
    /// Degrees, stored as radians.
    Deg,
    Sec,
    Kg,
}

#[derive(Debug)]
pub enum ExprKind {
    Num { value: f64, suffix: NumSuffix, text: String, float: bool },
    Bool(bool),
    Name(String),
    Member(Box<Expr>, Ident),
    Index(Box<Expr>, Box<Expr>),
    Call { callee: Ident, args: Vec<Arg> },
    /// `Pt[S](...)`, `vec3<f32>(...)`, `Xform[A -> B](...)`.
    Ctor { ty: TyExpr, args: Vec<Arg> },
    Binary(BinOp, Box<Expr>, Box<Expr>),
    Unary(UnOp, Box<Expr>),
}

#[derive(Debug)]
pub struct Arg {
    pub name: Option<Ident>,
    pub value: Expr,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    Rem,
    Lt,
    Le,
    Gt,
    Ge,
    Eq,
    Ne,
    And,
    Or,
    BitAnd,
    BitOr,
    BitXor,
    Shl,
    Shr,
}

impl BinOp {
    pub fn wgsl(self) -> &'static str {
        match self {
            BinOp::Add => "+",
            BinOp::Sub => "-",
            BinOp::Mul => "*",
            BinOp::Div => "/",
            BinOp::Rem => "%",
            BinOp::Lt => "<",
            BinOp::Le => "<=",
            BinOp::Gt => ">",
            BinOp::Ge => ">=",
            BinOp::Eq => "==",
            BinOp::Ne => "!=",
            BinOp::And => "&&",
            BinOp::Or => "||",
            BinOp::BitAnd => "&",
            BinOp::BitOr => "|",
            BinOp::BitXor => "^",
            BinOp::Shl => "<<",
            BinOp::Shr => ">>",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UnOp {
    Neg,
    Not,
}

/// Type syntax as written.
#[derive(Clone, Debug)]
pub enum TyExpr {
    /// `f32`, `u32`, `bool`, `vec3f`, a unit alias like `Meters`.
    Name(Ident),
    /// `vec3<f32>`.
    Generic(Ident, Vec<TyExpr>),
    Geo(Geo, SpaceRef),
    Xform(SpaceRef, SpaceRef),
    Field(SpaceRef),
    /// `Frame[S]`: origin and an orthonormal tangent, normal, binormal (a scan's element).
    Frame(SpaceRef),
}

impl TyExpr {
    pub fn span(&self) -> Span {
        match self {
            TyExpr::Name(i) | TyExpr::Generic(i, _) => i.span,
            TyExpr::Geo(_, s) | TyExpr::Field(s) | TyExpr::Frame(s) => s.span,
            TyExpr::Xform(a, b) => a.span.to(b.span),
        }
    }
}

/// `World`, or `'s` (a space variable in a fn signature, or a map's fresh space).
#[derive(Clone, Debug)]
pub struct SpaceRef {
    pub name: String,
    pub var: bool,
    pub span: Span,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Geo {
    Pt,
    Vec,
    Dir,
    Norm,
}

/// A checked space: declared, fresh (introduced by `map`, numbered), or a signature variable.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Space {
    Named(String),
    Fresh(String, u32),
    Var(String),
}

impl std::fmt::Display for Space {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Space::Named(n) => write!(f, "{n}"),
            Space::Fresh(n, _) | Space::Var(n) => write!(f, "'{n}"),
        }
    }
}

/// Checked types. Units are f32 aliases in Phase 0 (`Meters`, `Rad`, ...); suffixes convert (`deg` to radians).
#[derive(Clone, Debug, PartialEq)]
pub enum Ty {
    Void,
    Bool,
    U32,
    I32,
    F32,
    /// An integer literal without suffix: converts to u32, i32 or f32 where used.
    AbstractInt,
    /// `vecN<f32>`, N in 2..=4, space-less.
    VecN(u8),
    Geo(Geo, Space),
    Xform(Space, Space),
    /// A map's `to_parent` / `from_parent`: only applied with `*`.
    Map(Space, Space),
    /// A kind's row (`self`).
    Row(String),
    /// Origin, tangent, normal, binormal.
    Frame(Space),
    /// An ordered sum of library lobes (`@lobe` fns), bottom layer first.
    Bsdf,
}

impl std::fmt::Display for Ty {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Ty::Void => write!(f, "()"),
            Ty::Bool => write!(f, "bool"),
            Ty::U32 => write!(f, "u32"),
            Ty::I32 => write!(f, "i32"),
            Ty::F32 => write!(f, "f32"),
            Ty::AbstractInt => write!(f, "integer literal"),
            Ty::VecN(n) => write!(f, "vec{n}<f32>"),
            Ty::Geo(g, s) => write!(f, "{g:?}[{s}]"),
            Ty::Xform(a, b) => write!(f, "Xform[{a} -> {b}]"),
            Ty::Map(a, b) => write!(f, "Map[{a} -> {b}]"),
            Ty::Row(k) => write!(f, "row of {k}"),
            Ty::Frame(s) => write!(f, "Frame[{s}]"),
            Ty::Bsdf => write!(f, "Bsdf"),
        }
    }
}

impl Ty {
    /// f32 words in a row table.
    pub fn words(&self) -> Option<u32> {
        match self {
            Ty::Bool | Ty::U32 | Ty::I32 | Ty::F32 => Some(1),
            Ty::VecN(n) => Some(u32::from(*n)),
            Ty::Geo(..) => Some(3),
            Ty::Xform(..) | Ty::Frame(_) => Some(12),
            Ty::Void | Ty::AbstractInt | Ty::Map(..) | Ty::Row(_) | Ty::Bsdf => None,
        }
    }
}
