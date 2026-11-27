//! The source tree of `.loft` v1.0 (`docs/spec/LOFT_V1_SYNTAX.md`) as the parser builds it and the elaborator
//! completes it, and the checked types the passes share. Every expression has an `id`; the checker's types live in a
//! side table indexed by it. The parser may leave holes the elaborator fills (an unannotated parameter, a bracket
//! literal, a scan's element type); after elaboration the tree is fully typed and every pass reads it unchanged.

use loft_ir::Span;

#[derive(serde::Serialize, Clone, Debug)]
pub struct Ident {
    pub name: String,
    pub span: Span,
}

#[derive(serde::Serialize, Clone, Debug, Default)]
pub struct Module {
    /// Dimensions (length, time, mass, angle) on source types and literals. Their GPU representation is f32.
    pub dimensions: Vec<(Span, Dimension)>,
    pub items: Vec<Item>,
    pub public: Vec<Span>,
    /// Authored declaration name to body range, including its opening delimiter.
    #[serde(skip)]
    pub bodies: std::collections::HashMap<Span, Span>,
    /// Linked names for authored callbacks used by compiler-generated code.
    pub links: Vec<(String, String)>,
    pub file: Option<u16>,
    /// `diff(f, x)` sites, as written: the AD pass generates `f__d_x` (the parser already called it).
    pub diffs: Vec<(Ident, Ident)>,
}

pub type Dimension = [f64; 4];

#[derive(serde::Serialize, Clone, Debug)]
pub enum Item {
    Use(Import),
    Space(Vec<Ident>),
    Struct(Ident, Vec<RowField>),
    TypeAlias(Ident, TyExpr),
    Const(ConstDecl),
    Fn(FnDecl),
    Kind(KindDecl),
    Scan(Box<ScanDecl>),
    Schedule(ScheduleDecl),
    /// `build { ... }`: an anonymous build fn; the default engine runs every one in source order.
    Build(FnDecl),
    Law(LawDecl),
    Kernel(KernelDecl),
    /// `check name(h: K, ...) -> unit { ... }`: `ret` is the verdict's unit.
    Check(FnDecl),
    Camera(CameraDecl),
    Target(TargetDecl),
    World(Ident, Vec<Ident>),
    Bake(BakeDecl),
}

#[derive(serde::Serialize, Clone, Debug)]
pub struct Import {
    pub path: Vec<Ident>,
    pub binding: ImportBinding,
    pub span: Span,
}

#[derive(serde::Serialize, Clone, Debug)]
pub enum ImportBinding {
    Module(Option<Ident>),
    Selected(Vec<Ident>),
    Wildcard,
}

impl Item {
    pub fn declarations(&self) -> &[Ident] {
        let name = match self {
            Self::Use(_) | Self::Build(_) => return &[],
            Self::Space(names) => return names,
            Self::Struct(n, _) | Self::TypeAlias(n, _) | Self::World(n, _) => n,
            Self::Const(x) => &x.name,
            Self::Fn(x) | Self::Check(x) => &x.name,
            Self::Kind(x) => &x.name,
            Self::Scan(x) => &x.name,
            Self::Schedule(x) => &x.name,
            Self::Law(x) => &x.f.name,
            Self::Kernel(x) => &x.f.name,
            Self::Camera(x) => &x.name,
            Self::Target(x) => &x.name,
            Self::Bake(x) => &x.texel.name,
        };
        std::slice::from_ref(name)
    }
}

#[derive(serde::Serialize, Clone, Debug)]
pub struct TargetDecl {
    pub name: Ident,
    pub format: Ident,
    pub extent: TargetExtent,
    pub samples: u32,
    pub span: Span,
}

#[derive(serde::Serialize, Clone, Copy, Debug, PartialEq)]
pub enum TargetExtent {
    Fixed(u32, u32),
    View(f64),
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum TexDim {
    D2(u32, u32),
    D3(u32, u32, u32),
    Cube(u32),
}

#[derive(serde::Serialize, Clone, Debug)]
pub enum BakeClock {
    Load,
    Dirty(Vec<Ident>),
}

#[derive(serde::Serialize, Clone, Debug)]
pub struct BakeDecl {
    pub texel: FnDecl,
    pub dim: TexDim,
    pub format: Ident,
    pub clock: BakeClock,
    pub mips: bool,
}

/// Permissions checked at a kernel's typed boundary.
#[derive(serde::Serialize, Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum KernelCapability { Atomics, Workgroup, Subgroups, DynamicLoops }

#[derive(serde::Serialize, serde::Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub enum Determinism {
    #[default]
    Det,
    NonDet,
}

impl Determinism {
    pub fn is_det(&self) -> bool { *self == Self::Det }
}

#[derive(serde::Serialize, Clone, Debug)]
pub struct KernelDecl {
    pub f: FnDecl,
    pub tier: Determinism,
    pub allow: std::collections::BTreeSet<KernelCapability>,
    /// Checked atomic destinations, keyed by the table parameter at this entry.
    pub writes: std::collections::BTreeMap<String, Determinism>,
}

#[derive(serde::Serialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum UnprovedReason { AuthorOptOut }

#[derive(serde::Serialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum GeometryProof { Required, Unproved(UnprovedReason) }

/// `law name(h: K, dt, tbl: [T]) writes h.{a, b} { ... }`.
#[derive(serde::Serialize, Clone, Debug)]
pub struct LawDecl {
    pub f: FnDecl,
    pub tier: Determinism,
    pub writes: Vec<Ident>,
}

/// `camera Main { at: ..., radius: ..., height: ..., period: ..., fov: ... }`: build-stage values.
#[derive(serde::Serialize, Clone, Debug)]
pub struct CameraDecl {
    pub name: Ident,
    pub fields: Vec<(Ident, Expr)>,
}

/// `scan name(t: f32 in lo..hi, steps: N) @schedule(bake_lut) -> Table[T] { init: e; step(prev, t_i): e; }`:
/// a sequential integration baked at load into an N-entry table, read with `sample_c2(name, t)`.
#[derive(serde::Serialize, Clone, Debug)]
pub struct ScanDecl {
    pub name: Ident,
    pub var: Ident,
    pub var_ty: TyExpr,
    pub lo: Expr,
    pub hi: Expr,
    pub steps: Expr,
    /// `None` until elaboration (the type of `init`).
    pub elem: Option<TyExpr>,
    pub init: Expr,
    pub prev: Ident,
    pub at: Ident,
    pub step: Expr,
}

/// `const X: T = e;` (stage 1, plain WGSL) or `comptime const X: T = e;` (stage 0, overridable by the CLI).
#[derive(serde::Serialize, Clone, Debug)]
pub struct ConstDecl {
    pub range: Option<RowRange>,
    pub comptime: bool,
    pub name: Ident,
    pub ty: Option<TyExpr>,
    pub value: Expr,
}

/// `@name(args)` before a fn. Phase 0 knows one: `@lipschitz(param)` or `@lipschitz(max(a, b))`, a library axiom
/// the truth gate samples.
#[derive(serde::Serialize, Clone, Debug)]
pub struct Attr {
    pub name: Ident,
    pub args: Vec<Expr>,
}

#[derive(serde::Serialize, Clone, Debug)]
pub struct FnDecl {
    pub comptime: bool,
    pub attrs: Vec<Attr>,
    pub name: Ident,
    pub params: Vec<Param>,
    pub ret: Option<TyExpr>,
    pub body: Block,
    /// A `wgsl(...) -> T { ... }` island lifted into a fn: its raw body, which naga validates at emit.
    pub raw: Option<RawWgsl>,
}

#[derive(serde::Serialize, Clone, Debug)]
pub struct Param {
    pub name: Ident,
    // Keep the serialized form of existing functions (including geometry certificates) stable.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default: Option<Expr>,
    /// `None` for a query's `self`, and for an unannotated parameter until elaboration infers it.
    pub ty: Option<TyExpr>,
}

#[derive(serde::Serialize, Clone, Copy, Debug, PartialEq, Eq)]
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

#[derive(serde::Serialize, Clone, Debug)]
pub struct KindDecl {
    pub proof: GeometryProof,
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
#[derive(serde::Serialize, Clone, Debug)]
pub struct KindParam {
    pub name: Ident,
    pub ty: TyExpr,
    pub range: Option<(Expr, Expr)>,
    pub span: Span,
}

#[derive(serde::Serialize, Clone, Debug)]
pub struct RowField {
    pub name: Ident,
    pub ty: TyExpr,
    /// `in lo..hi` (inclusive) or `> lo` / `>= lo`: checked at every emit, and the interval a divisor reads.
    pub range: Option<RowRange>,
    /// `= value`: what an `emit` that leaves the column out stores.
    pub default: Option<Expr>,
}

#[derive(serde::Serialize, Clone, Debug)]
pub struct RowRange {
    pub lo: Expr,
    pub hi: Option<Expr>,
    /// `> lo`: the low end is excluded.
    pub strict: bool,
}

#[derive(serde::Serialize, Clone, Debug)]
pub struct ScheduleDecl {
    pub radiance: Option<(Ident, TyExpr)>,
    pub implicit: bool,
    pub name: Ident,
    /// A schedule with parameters is a schedule function: a call to it in another schedule inlines its steps.
    pub params: Vec<ScheduleParam>,
    pub passes: Vec<PassDecl>,
}

/// `pass label: verb(args) { body calls };`
#[derive(serde::Serialize, Clone, Debug)]
pub struct PassDecl {
    pub label: Ident,
    pub verb: Ident,
    /// Outermost schedule call, separate from the template's diagnostic and resource paths.
    #[serde(skip)]
    pub call_site: Option<Span>,
    pub args: Vec<Arg>,
    pub body: Vec<(Ident, Vec<Arg>)>,
    pub resources: Option<PassResources>,
    /// `on load` / `on dirty(...)`: the stage the step runs at. `None` runs it every frame.
    pub clock: Option<BakeClock>,
}

impl PassDecl {
    pub fn scheduled_from(&self) -> Span {
        self.call_site.unwrap_or(self.verb.span)
    }
}

/// A schedule function's parameter: `name`, `name = default`, or `..name` (every remaining positional argument).
#[derive(serde::Serialize, Clone, Debug)]
pub enum ScheduleParam {
    One { name: Ident, default: Option<Expr> },
    Rest(Ident),
}

#[derive(serde::Serialize, Clone, Debug)]
pub struct PassResources {
    pub reads: Vec<Ident>,
    pub writes: Vec<Ident>,
    pub body: Block,
}

#[derive(serde::Serialize, Clone, Debug)]
pub struct Block {
    pub stmts: Vec<Stmt>,
}

#[derive(serde::Serialize, Clone, Debug)]
pub enum Stmt {
    Let { mutable: bool, name: Ident, ty: Option<TyExpr>, value: Expr },
    Assign { target: Expr, op: Option<BinOp>, value: Expr },
    /// `map name = head |> stage(args) |> ...;` introduces the fresh space `'name`.
    Map { name: Ident, head: Expr, stages: Vec<(Ident, Vec<Expr>)> },
    Return(Option<Expr>, Span),
    If { cond: Expr, then: Block, otherwise: Option<Block> },
    Each { var: Ident, values: Vec<Expr>, body: Block, span: Span },
    For { comptime: bool, var: Ident, lo: Expr, hi: Expr, body: Block, span: Span },
    /// `emit K(f: e, ...)`, or `let b = emit K(...)` binding the new row's id.
    Emit { kind: Ident, fields: Vec<(Ident, Expr)>, span: Span, bind: Option<Ident> },
    ImportTable { kind: Ident, path: String, index: Expr, span: Span },
    Expr(Expr),
}

#[derive(serde::Serialize, Clone, Debug)]
pub struct Expr {
    pub id: u32,
    pub span: Span,
    pub kind: ExprKind,
}

#[derive(serde::Serialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum NumSuffix {
    None,
    /// A unit-bearing float, already scaled to base units. The original suffix is kept in `text`.
    Unit,
}

#[derive(serde::Serialize, Clone, Debug)]
pub struct ExprBlock {
    pub body: Block,
    pub value: Box<Expr>,
}

/// An integer dispatch shares the conditional node's first arm and fallback.
#[derive(serde::Serialize, Clone, Debug)]
pub struct MatchCases {
    pub first: (i64, Span),
    pub rest: Vec<((i64, Span), Expr)>,
}

impl MatchCases {
    pub fn arms<'a>(&'a self, first: &'a Expr) -> impl Iterator<Item = ((i64, Span), &'a Expr)> {
        std::iter::once((self.first, first)).chain(self.rest.iter().map(|(key, value)| (*key, value)))
    }
}

#[derive(serde::Serialize, Clone, Debug)]
pub enum ExprKind {
    /// Geometry lowering reads the scale at this site and uses four-tap normals.
    Unproved { step_scale: Box<Expr>, body: ExprBlock },
    /// Explicit derivative inside an opted-out block; lowering supplies finite differences.
    FiniteDiff { function: Ident, parameter: Ident, args: Vec<Arg> },
    If {
        condition: Box<Expr>,
        then: ExprBlock,
        otherwise: ExprBlock,
        #[serde(skip_serializing_if = "Option::is_none")]
        cases: Option<Box<MatchCases>>,
    },
    Num { value: f64, suffix: NumSuffix, text: String, float: bool },
    Bool(bool),
    String(String),
    Name(String),
    Member(Box<Expr>, Ident),
    Index(Box<Expr>, Box<Expr>),
    Call { callee: Ident, args: Vec<Arg> },
    /// `Pt[S](...)`, `vec3<f32>(...)`, `Xform[A -> B](...)`.
    Ctor { ty: TyExpr, args: Vec<Arg> },
    Binary(BinOp, Box<Expr>, Box<Expr>),
    Unary(UnOp, Box<Expr>),

}

#[derive(serde::Serialize, Clone, Debug)]
pub struct Arg {
    pub name: Option<Ident>,
    pub value: Expr,
}

#[derive(serde::Serialize, Clone, Copy, Debug, PartialEq, Eq)]
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

#[derive(serde::Serialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum UnOp {
    Neg,
    Not,
}

pub fn frame_field(name:&str)->&str {
    match name {"o"=>"origin","t"=>"tangent","n"=>"normal","b"=>"binormal",s=>s}
}

#[derive(Clone, Debug, serde::Serialize)]
pub struct RawWgsl {
    pub body: String,
    pub references: Vec<Ident>,
}

impl FnDecl {
    // `wgsl det` was removed (2026-11-23): raw WGSL is never certified deterministic.
    pub fn refuses_unverified_raw(&self)->bool{self.raw.is_some()}
}

/// Type syntax as written.
#[derive(serde::Serialize, Clone, Debug)]
pub enum TyExpr {
    /// `f32`, `u32`, `bool`, `vec3f`, a unit alias like `Meters`.
    Name(Ident),
    Array(Box<TyExpr>, u32, Span),
    Geo(Geo, SpaceRef),
    Xform(SpaceRef, SpaceRef),
    Field(SpaceRef),
    /// `frame@S`: origin and an orthonormal tangent, normal, binormal (a scan's element).
    Frame(SpaceRef),
    /// `[K]`: a read-only table of kind `K` (a law's or check's other rows).
    Table(Ident),
}

impl TyExpr {
    pub fn span(&self) -> Span {
        match self {
            TyExpr::Array(_, _, span) => *span,
            TyExpr::Name(i) | TyExpr::Table(i) => i.span,
            TyExpr::Geo(_, s) | TyExpr::Field(s) | TyExpr::Frame(s) => s.span,
            TyExpr::Xform(a, b) => a.span.to(b.span),
        }
    }
}

/// `World`, or `'s` (a space variable in a fn signature, or a map's fresh space).
#[derive(serde::Serialize, Clone, Debug)]
pub struct SpaceRef {
    pub name: String,
    pub var: bool,
    pub span: Span,
}

#[derive(serde::Serialize, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Geo {
    Attach,
    Target,
    Pt,
    Vec,
    Dir,
    Norm,
}

impl Geo {
    /// The surface spelling: `p3`, `v3`, `d3`, `n3`.
    pub fn short(self) -> &'static str {
        match self {
            Geo::Attach => "attach",
            Geo::Target => "target",
            Geo::Pt => "p3",
            Geo::Vec => "v3",
            Geo::Dir => "d3",
            Geo::Norm => "n3",
        }
    }
}

/// A checked space: declared, fresh (introduced by `map`, numbered), a signature variable, or the open source of a
/// placement built by `place` (fixed where it is stored or composed, so it meets any space).
#[derive(serde::Serialize, Clone, Debug, PartialEq, Eq, Hash)]
pub enum Space {
    Named(String),
    Fresh(String, u32),
    Var(String),
    Open,
}

impl std::fmt::Display for Space {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Space::Named(n) => write!(f, "{n}"),
            Space::Fresh(n, _) => write!(f, "{n}"),
            Space::Var(n) => write!(f, "{n}"),
            Space::Open => write!(f, "_"),
        }
    }
}

/// Checked types. Units are f32 aliases in Phase 0 (`Meters`, `Rad`, ...); suffixes convert (`deg` to radians).
#[derive(serde::Serialize, Clone, Debug, PartialEq)]
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
    Struct(String),
    Array(Box<Ty>, u32),
    /// Origin, tangent, normal, binormal.
    Frame(Space),
    /// An ordered sum of library lobes (`lobe` fns), bottom layer first.
    Bsdf,
    /// A row id of a kind (a column typed `K`, the value of `emit K(...)`).
    Id(String),
    /// A read-only table of a kind's rows (`[K]`), indexed by an id.
    Table(String),
    /// A check's result: held, missed or unknown.
    Verdict,
}

impl std::fmt::Display for Ty {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Ty::Void => write!(f, "()"),
            Ty::Bool => write!(f, "bool"),
            Ty::U32 => write!(f, "u"),
            Ty::I32 => write!(f, "i"),
            Ty::F32 => write!(f, "f"),
            Ty::AbstractInt => write!(f, "integer literal"),
            Ty::VecN(3) => write!(f, "rgb"),
            Ty::VecN(n) => write!(f, "v{n}"),
            Ty::Geo(g, s) => write!(f, "{}@{s}", g.short()),
            Ty::Xform(a, b) => write!(f, "xf@{a}>{b}"),
            Ty::Map(a, b) => write!(f, "map {a} -> {b}"),
            Ty::Array(t, n) => write!(f, "array[{t}, {n}]"),
            Ty::Struct(k) => write!(f, "{k}"),
            Ty::Row(k) => write!(f, "row of {k}"),
            Ty::Frame(s) => write!(f, "frame@{s}"),
            Ty::Bsdf => write!(f, "Bsdf"),
            Ty::Id(k) => write!(f, "{k}"),
            Ty::Table(k) => write!(f, "[{k}]"),
            Ty::Verdict => write!(f, "verdict"),
        }
    }
}

impl Ty {
    /// f32 words in a row table.
    pub fn words(&self) -> Option<u32> {
        match self {
            Ty::Bool | Ty::U32 | Ty::I32 | Ty::F32 | Ty::Id(_) => Some(1),
            Ty::VecN(n) => Some(u32::from(*n)),
            Ty::Geo(..) => Some(3),
            Ty::Xform(..) | Ty::Frame(_) => Some(12),
            Ty::Void | Ty::AbstractInt | Ty::Map(..) | Ty::Row(_) | Ty::Struct(_) | Ty::Array(..) | Ty::Bsdf | Ty::Table(_) | Ty::Verdict => None,
        }
    }
}
