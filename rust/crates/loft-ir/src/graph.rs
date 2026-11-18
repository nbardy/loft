//! The graph arena a job decodes into (plan §1.3, wire §3.2): typed nodes in recording order, so every operand precedes
//! its user, and the declarations their ops name, one array per kind. Built only by `wire::decode`, which validates
//! every bound, reference, layout and type rule; consumers read the public fields and never re-check them.
use serde::{Deserialize, Serialize};

/// A value's type. Wire code = the order here (src/record/wire.ts `TYS`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Ty {
    F32,
    V2,
    V3,
    V4,
    Bool,
}

impl Ty {
    pub const ALL: [Ty; 5] = [Ty::F32, Ty::V2, Ty::V3, Ty::V4, Ty::Bool];
    /// Components of a number type; None for `Bool`.
    pub fn width(self) -> Option<u8> {
        match self {
            Ty::F32 => Some(1),
            Ty::V2 => Some(2),
            Ty::V3 => Some(3),
            Ty::V4 => Some(4),
            Ty::Bool => None,
        }
    }
    /// The literal entries a value of this type takes in `Graph::lits` (a bool is one, 0 or 1).
    pub fn lits(self) -> u8 {
        self.width().unwrap_or(1)
    }
    /// The recorder's spelling (`src/expr.ts` `Ty`), which the canonical print uses.
    pub fn name(self) -> &'static str {
        match self {
            Ty::F32 => "f32",
            Ty::V2 => "vec2",
            Ty::V3 => "vec3",
            Ty::V4 => "vec4",
            Ty::Bool => "bool",
        }
    }
    pub(crate) fn of_width(w: u8) -> Option<Ty> {
        Ty::ALL.get(usize::from(w).checked_sub(1)?).copied().filter(|t| *t != Ty::Bool)
    }
}

/// A swizzle selection: `len` components (1..=4), component k in bits 2k..2k+2 of `comps`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Sel {
    pub len: u8,
    pub comps: u8,
}

/// What a node computes: one variant per wire op (src/record/wire.ts `Op`; codes in `wire`). Ids name the decl arrays
/// of `Graph` (`fns`, `raws`, `loops`, `kernels`, `params`, `inputs`) or a symbol.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Op {
    /// The first of `ty.lits()` entries of `Graph::lits` (exact f64 bits as authored).
    Lit(u32),
    Time,
    Param(u32),
    Input(u32),
    /// A binder of a loop or a graph function, named by symbol `.0`; read only inside its owner.
    Bind(u32),
    /// A free variable named by symbol `.0` (u, v, p), bound by the job's consumer.
    Var(u32),
    /// A graph function called with the node's args; inlined during interning.
    Call(u32),
    /// vec4 lane `lane` of the row the f32 arg names, of a kernel's resource.
    Fetch {
        kernel: u32,
        lane: u16,
    },
    /// Result component `k` of a loop.
    Loop {
        decl: u32,
        k: u16,
    },
    /// d args[0] / d args[1]: a request, rewritten to its derivative graph during interning. The wave-1 recorder writes
    /// derivatives already expanded, so no job carries one yet.
    Grad,
    /// A raw WGSL function called with the node's args. GPU only.
    Raw(u32),
    Swizzle(Sel),
    /// A vector from 1..=4 parts whose widths sum to the node's (or one f32, splatted).
    Vec,
    Prim(Prim),
}

macro_rules! prims {
    ($($p:ident $name:literal $rule:ident),* $(,)?) => {
        /// The operators, in the order of `src/expr.ts` `Fn` (wire code 16 + index), the order of loft-core's `Prim`.
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
        pub enum Prim { $($p),* }
        impl Prim {
            pub const ALL: &[Prim] = &[$(Prim::$p),*];
            /// The recorder's name, as `src/expr.ts` spells it (`Fn`), which the canonical print uses.
            pub fn name(self) -> &'static str { match self { $(Prim::$p => $name),* } }
            fn rule(self) -> Rule { match self { $(Prim::$p => Rule::$rule),* } }
        }
    };
}
prims! {
    Add "add" Map2, Sub "sub" Map2, Mul "mul" Map2, Div "div" Map2, Neg "neg" Map1, Min "min" Map2, Max "max" Map2,
    Pow "pow" Map2, Mod "mod" Map2, Atan2 "atan2" Map2, Step "step" Map2,
    Abs "abs" Map1, Sign "sign" Map1, Floor "floor" Map1, Fract "fract" Map1, Sqrt "sqrt" Map1, Exp "exp" Map1,
    Log "log" Map1, Sin "sin" Map1, Cos "cos" Map1, Tan "tan" Map1, Saturate "saturate" Map1,
    Normalize "normalize" Map1,
    Mix "mix" Map3, Clamp "clamp" Map3, Smoothstep "smoothstep" Map3, Length "length" Norm,
    Distance "distance" Dot, Dot "dot" Dot, Cross "cross" Cross,
    Lt "lt" Cmp, Gt "gt" Cmp, Le "le" Cmp, Ge "ge" Cmp, Eq "eq" Cmp, Ne "ne" Cmp,
    And "and" Logic, Or "or" Logic, Not "not" Not, Select "select" Select,
    Ceil "ceil" Map1, Trunc "trunc" Map1, RoundEven "roundEven" Map1, Rem "rem" Map2, InverseSqrt "inverseSqrt" Map1,
    Exp2 "exp2" Map1, Log2 "log2" Map1, Asin "asin" Map1, Acos "acos" Map1, Atan "atan" Map1, Sinh "sinh" Map1,
    Cosh "cosh" Map1, Tanh "tanh" Map1,
}

enum Rule {
    Map1,
    Map2,
    Map3,
    Norm,
    Dot,
    Cross,
    Cmp,
    Logic,
    Not,
    Select,
}

/// Component-wise result of two number types: equal, or a scalar splats to the other's width.
fn wider(a: Ty, b: Ty) -> Result<Ty, (Ty, Ty)> {
    match (a.width(), b.width()) {
        (Some(_), Some(_)) if a == b => Ok(a),
        (Some(1), Some(_)) => Ok(b),
        (Some(_), Some(1)) => Ok(a),
        _ => Err((a, b)),
    }
}

fn number(a: Ty) -> Result<Ty, (Ty, Ty)> {
    a.width().map(|_| a).ok_or((Ty::F32, a))
}

fn exactly(want: Ty, got: Ty) -> Result<Ty, (Ty, Ty)> {
    if want == got { Ok(got) } else { Err((want, got)) }
}

impl Prim {
    /// The result type of this operator over `args`. Err is (wanted, given) for the first operand that fails. Operand
    /// counts are the wire's to check (`takes`); a count that slips through is refused as (F32, F32).
    pub fn result(self, args: &[Ty]) -> Result<Ty, (Ty, Ty)> {
        match (self.rule(), args) {
            (Rule::Map1, [a]) => number(*a),
            (Rule::Map2, [a, b]) => wider(number(*a)?, number(*b)?),
            (Rule::Map3, [a, b, c]) => wider(wider(number(*a)?, number(*b)?)?, number(*c)?),
            (Rule::Norm, [a]) => number(*a).map(|_| Ty::F32),
            (Rule::Dot, [a, b]) => exactly(number(*a)?, *b).map(|_| Ty::F32),
            (Rule::Cross, [a, b]) => exactly(Ty::V3, *a).and(exactly(Ty::V3, *b)),
            (Rule::Cmp, [a, b]) => exactly(Ty::F32, *a).and(exactly(Ty::F32, *b)).map(|_| Ty::Bool),
            (Rule::Logic, [a, b]) => exactly(Ty::Bool, *a).and(exactly(Ty::Bool, *b)),
            (Rule::Not, [a]) => exactly(Ty::Bool, *a),
            (Rule::Select, [c, a, b]) => exactly(Ty::Bool, *c).and(wider(number(*a)?, number(*b)?)),
            (
                Rule::Map1
                | Rule::Map2
                | Rule::Map3
                | Rule::Norm
                | Rule::Dot
                | Rule::Cross
                | Rule::Cmp
                | Rule::Logic
                | Rule::Not
                | Rule::Select,
                _,
            ) => Err((Ty::F32, Ty::F32)),
        }
    }
    /// The operand count this operator takes.
    pub fn arity(self) -> usize {
        match self.rule() {
            Rule::Map1 | Rule::Norm | Rule::Not => 1,
            Rule::Map2 | Rule::Dot | Rule::Cross | Rule::Cmp | Rule::Logic => 2,
            Rule::Map3 | Rule::Select => 3,
        }
    }
}

/// A source position, the file resolved from the job's symbols.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Site {
    pub file: Box<str>,
    pub line: u32,
    pub col: u32,
}

/// One node: its operation, its type, and its operands as `args[args..args + n]` (node ids, all below its own).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Node {
    pub op: Op,
    pub ty: Ty,
    pub args: u32,
    pub n: u32,
}

/// A graph function: one body over binder nodes (`Op::Bind`), each call's args bound to them in order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GraphFn {
    pub name: u32,
    pub ret: Ty,
    pub binders: Box<[u32]>,
    pub body: u32,
}

/// A raw WGSL function: its name and text (symbols), and the raw functions its text calls (`Graph::raws`, each
/// declared before it).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RawFn {
    pub name: u32,
    pub ret: Ty,
    pub text: u32,
    pub deps: Box<[u32]>,
}

/// A loop: `count` steps (f32) from `init`, each step reading the state binders and the f32 index binder and giving
/// `next`, stopped early when `stop` (bool, over the same binders) holds. `init`, `state` and `next` are parallel.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Loop {
    pub count: u32,
    pub init: Box<[u32]>,
    pub state: Box<[u32]>,
    pub index: u32,
    pub next: Box<[u32]>,
    pub stop: Option<u32>,
}

impl Loop {
    /// Every node the loop names: count, starts, binders, index, steps and stop.
    pub fn nodes(&self) -> impl Iterator<Item = u32> + '_ {
        std::iter::once(self.count)
            .chain(self.init.iter().copied())
            .chain(self.state.iter().copied())
            .chain(std::iter::once(self.index))
            .chain(self.next.iter().copied())
            .chain(self.stop)
    }
}

/// What a `Fetch` reads. In wave 1 a kernel names a resource the TS compiler fills; wave 2 gives kernels bodies.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kernel {
    /// A per-frame table: `entries` rows of `lanes` vec4s, filled for `role`.
    Table { name: u32, lanes: u32, entries: u32, role: Role },
    /// Supplied samples `rows[at..at + len]`, read as one lane.
    Rows { label: u32, at: u32, len: u32 },
}

/// Who fills a table (src/expr.ts `TableSource["role"]`). Wire code = the order here.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Role {
    Stations,
    Data,
    Static,
}

impl Role {
    pub const ALL: [Role; 3] = [Role::Stations, Role::Data, Role::Static];
    pub fn name(self) -> &'static str {
        match self {
            Role::Stations => "stations",
            Role::Data => "data",
            Role::Static => "static",
        }
    }
}

/// An authored f32 parameter: default, minimum and maximum are `lits` indices.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Param {
    pub name: u32,
    pub def: u32,
    pub min: u32,
    pub max: u32,
}

/// A host input of a number type; its default is `ty.lits()` entries of `lits` from `def`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Input {
    pub name: u32,
    pub ty: Ty,
    pub def: u32,
}

/// The program a job asks for (plan §3.1). Wire code = the order here.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Program {
    Scene,
    Solve,
    Evaluate,
    Check,
}

/// A decoded, validated job graph.
#[derive(Clone, Debug, PartialEq)]
pub struct Graph {
    pub nodes: Box<[Node]>,
    pub args: Box<[u32]>,
    /// f64 bits, exactly as authored.
    pub lits: Box<[u64]>,
    /// f32 bits of supplied samples (curve controls, author tables).
    pub rows: Box<[u32]>,
    pub blobs: Box<[u8]>,
    pub syms: Box<str>,
    /// Symbol k is `syms[sym_at[k]..sym_at[k + 1]]`.
    pub sym_at: Box<[u32]>,
    pub sites: Box<[Site]>,
    /// Per node: 0 = no site, k = `sites[k - 1]`.
    pub node_sites: Box<[u32]>,
    pub fns: Box<[GraphFn]>,
    pub raws: Box<[RawFn]>,
    pub loops: Box<[Loop]>,
    pub kernels: Box<[Kernel]>,
    pub params: Box<[Param]>,
    pub inputs: Box<[Input]>,
    /// The program's roots.
    pub roots: Box<[u32]>,
}

impl Graph {
    pub fn args_of(&self, node: &Node) -> &[u32] {
        &self.args[node.args as usize..(node.args + node.n) as usize]
    }
    pub fn sym(&self, i: u32) -> &str {
        &self.syms[self.sym_at[i as usize] as usize..self.sym_at[i as usize + 1] as usize]
    }
    pub fn site(&self, node: u32) -> Option<&Site> {
        self.node_sites[node as usize].checked_sub(1).map(|k| &self.sites[k as usize])
    }
}
